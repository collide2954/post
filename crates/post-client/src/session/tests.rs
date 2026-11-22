use std::num::NonZeroU32;

use post_proto::{
    ClientMessage, Code, ErrorCode, Invitation, Name, Role, ServerMessage, TextError, Timestamp,
};

use super::{Entry, Session};

fn name(text: &str) -> Name {
    Name::parse(text).expect(text)
}

fn time(second: u8) -> Timestamp {
    Timestamp::parse(&format!("2026-11-21T14:02:{second:02}.000Z")).expect("valid")
}

fn join(channel: &str, who: &str, second: u8) -> ServerMessage {
    ServerMessage::Join {
        channel: channel.to_owned(),
        name: name(who),
        time: time(second),
    }
}

fn leave(channel: &str, who: &str, second: u8) -> ServerMessage {
    ServerMessage::Leave {
        reference: None,
        channel: channel.to_owned(),
        name: name(who),
        time: time(second),
    }
}

fn message(channel: &str, from: &str, second: u8, text: &str) -> ServerMessage {
    ServerMessage::Message {
        reference: None,
        channel: channel.to_owned(),
        from: name(from),
        time: time(second),
        text: text.to_owned(),
    }
}

#[test]
fn starts_with_a_name_and_an_empty_timeline() {
    let session = Session::new(name("ana"));

    assert_eq!(session.name(), &name("ana"));
    assert_eq!(session.entries(), []);
    assert_eq!(session.problem(), None);
    assert_eq!(session.invitations(), None);
}

#[test]
fn keeps_the_newest_list_of_invitations_from_the_server() {
    let mut session = Session::new(name("ana"));
    let invitation = |text: &str, used| Invitation {
        code: Code::parse(text).expect(text),
        link: format!("https://chat.example.org/i/{text}"),
        created: time(1),
        end: None,
        uses: None,
        used,
    };

    session.apply(ServerMessage::Invitations {
        reference: Some(1),
        list: vec![invitation("7Hq2kdX9mPq4", 0)],
    });
    assert_eq!(
        session.invitations(),
        Some([invitation("7Hq2kdX9mPq4", 0)].as_slice())
    );

    let newer = vec![invitation("k3Wm8sTz2LpQ", 0), invitation("7Hq2kdX9mPq4", 1)];
    session.apply(ServerMessage::Invitations {
        reference: None,
        list: newer.clone(),
    });
    assert_eq!(session.invitations(), Some(newer.as_slice()));

    session.apply(ServerMessage::Invitations {
        reference: None,
        list: Vec::new(),
    });
    assert_eq!(session.invitations(), Some([].as_slice()));
    assert_eq!(session.entries(), []);
}

#[test]
fn makes_each_invitation_request_with_a_new_number() {
    let mut session = Session::new(name("ana"));
    let code = Code::parse("7Hq2kdX9mPq4").expect("valid");

    assert_eq!(
        session.invite(NonZeroU32::new(604_800), NonZeroU32::new(5)),
        ClientMessage::Invite {
            reference: Some(1),
            lifetime: NonZeroU32::new(604_800),
            uses: NonZeroU32::new(5),
        }
    );
    assert_eq!(
        session.ask_for_invitations(),
        ClientMessage::Invitations { reference: Some(2) }
    );
    assert_eq!(
        session.revoke(code.clone()),
        ClientMessage::Revoke {
            reference: Some(3),
            code,
        }
    );
}

#[test]
fn forgets_the_error_when_the_user_makes_a_new_request() {
    let mut session = Session::new(name("ana"));

    session.apply(ServerMessage::error(Some(1), ErrorCode::NotPermitted));
    assert_eq!(session.problem(), Some(ErrorCode::NotPermitted));

    let _request = session.ask_for_invitations();
    assert_eq!(session.problem(), None);
}

#[test]
fn forgets_the_error_when_the_user_dismisses_it() {
    let mut session = Session::new(name("ana"));
    session.apply(ServerMessage::error(Some(1), ErrorCode::TooSoon));

    session.dismiss_problem();

    assert_eq!(session.problem(), None);
    assert_eq!(
        session.compose("Hello"),
        Ok(ClientMessage::Send {
            reference: Some(1),
            channel: "lobby".to_owned(),
            text: "Hello".to_owned(),
        })
    );
}

fn rename(old: &str, new: &str, second: u8) -> ServerMessage {
    ServerMessage::Rename {
        reference: None,
        old: name(old),
        new: name(new),
        time: time(second),
    }
}

#[test]
fn shows_the_new_name_of_a_user_in_the_earlier_entries() {
    let mut session = Session::new(name("ana"));
    session.apply(join("lobby", "ben", 1));
    session.apply(message("lobby", "ben", 2, "Hi"));
    session.apply(message("lobby", "cy", 3, "Hello"));
    session.apply(leave("lobby", "ben", 4));

    session.apply(rename("ben", "benito", 5));
    session.apply(rename("benito", "beni", 6));

    assert_eq!(
        session.entries(),
        [
            Entry::Joined {
                time: time(1),
                name: name("beni"),
            },
            Entry::Message {
                time: time(2),
                from: name("beni"),
                text: "Hi".to_owned(),
            },
            Entry::Message {
                time: time(3),
                from: name("cy"),
                text: "Hello".to_owned(),
            },
            Entry::Left {
                time: time(4),
                name: name("beni"),
            },
            Entry::Renamed {
                time: time(5),
                old: name("ben"),
                new: name("benito"),
            },
            Entry::Renamed {
                time: time(6),
                old: name("benito"),
                new: name("beni"),
            },
        ]
    );
    assert_eq!(session.name(), &name("ana"));
}

#[test]
fn takes_the_new_name_of_its_own_account() {
    let mut session = Session::new(name("ana"));
    session.apply(message("lobby", "ana", 1, "Hello"));

    session.apply(ServerMessage::Rename {
        reference: Some(4),
        old: name("ana"),
        new: name("anna"),
        time: time(2),
    });

    assert_eq!(session.name(), &name("anna"));
    assert!(matches!(
        &session.entries()[0],
        Entry::Message { from, .. } if *from == name("anna")
    ));
}

#[test]
fn makes_a_rename_request_with_a_new_number() {
    let mut session = Session::new(name("ana"));
    session.apply(ServerMessage::error(Some(1), ErrorCode::TooSoon));

    let request = session.rename(&name("anna"));

    assert_eq!(
        request,
        ClientMessage::Rename {
            reference: Some(1),
            name: "anna".to_owned(),
        }
    );
    assert_eq!(session.problem(), None);
}

#[test]
fn records_what_occurs_in_the_lobby_in_sequence() {
    let mut session = Session::new(name("ana"));

    session.apply(join("lobby", "ben", 1));
    session.apply(message("lobby", "ben", 2, "Hi"));
    session.apply(leave("lobby", "ben", 3));

    assert_eq!(
        session.entries(),
        [
            Entry::Joined {
                time: time(1),
                name: name("ben"),
            },
            Entry::Message {
                time: time(2),
                from: name("ben"),
                text: "Hi".to_owned(),
            },
            Entry::Left {
                time: time(3),
                name: name("ben"),
            },
        ]
    );
}

#[test]
fn ignores_what_occurs_in_a_different_channel() {
    let mut session = Session::new(name("ana"));

    session.apply(join("garden", "ben", 1));
    session.apply(message("garden", "ben", 2, "Hi"));
    session.apply(leave("garden", "ben", 3));

    assert_eq!(session.entries(), []);
}

#[test]
fn ignores_a_second_welcome_message() {
    let mut session = Session::new(name("ana"));

    session.apply(ServerMessage::Welcome {
        reference: None,
        name: name("ben"),
        role: Role::Member,
    });

    assert_eq!(session.name(), &name("ana"));
    assert_eq!(session.entries(), []);
    assert_eq!(session.problem(), None);
}

#[test]
fn makes_a_send_message_with_a_new_number_each_time() {
    let mut session = Session::new(name("ana"));

    assert_eq!(
        session.compose("Hello"),
        Ok(ClientMessage::Send {
            reference: Some(1),
            channel: "lobby".to_owned(),
            text: "Hello".to_owned(),
        })
    );
    assert_eq!(
        session.compose("  spaces stay  "),
        Ok(ClientMessage::Send {
            reference: Some(2),
            channel: "lobby".to_owned(),
            text: "  spaces stay  ".to_owned(),
        })
    );
}

#[test]
fn does_not_make_a_message_from_a_text_that_breaks_the_rules() {
    let mut session = Session::new(name("ana"));

    assert_eq!(session.compose("   "), Err(TextError::Empty));
    assert_eq!(session.compose(&"x".repeat(4001)), Err(TextError::TooLong));
    assert!(matches!(
        session.compose("Hello"),
        Ok(ClientMessage::Send {
            reference: Some(1),
            ..
        })
    ));
}

#[test]
fn remembers_the_last_error_from_the_server() {
    let mut session = Session::new(name("ana"));

    session.apply(ServerMessage::error(Some(1), ErrorCode::TextTooLong));
    assert_eq!(session.problem(), Some(ErrorCode::TextTooLong));

    session.apply(ServerMessage::error(None, ErrorCode::BadMessage));
    assert_eq!(session.problem(), Some(ErrorCode::BadMessage));
    assert_eq!(session.entries(), []);
}

#[test]
fn forgets_the_error_when_the_user_sends_a_new_text() {
    let mut session = Session::new(name("ana"));
    session.apply(ServerMessage::error(Some(1), ErrorCode::TextTooLong));

    assert_eq!(session.compose(" "), Err(TextError::Empty));
    assert_eq!(session.problem(), Some(ErrorCode::TextTooLong));

    assert!(session.compose("Hello").is_ok());
    assert_eq!(session.problem(), None);
}

#[test]
fn starts_the_numbers_again_after_the_largest_number() {
    let mut session = Session::with_next_reference(name("ana"), u32::MAX);

    assert!(matches!(
        session.compose("one"),
        Ok(ClientMessage::Send {
            reference: Some(u32::MAX),
            ..
        })
    ));
    assert!(matches!(
        session.compose("two"),
        Ok(ClientMessage::Send {
            reference: Some(0),
            ..
        })
    ));
}
