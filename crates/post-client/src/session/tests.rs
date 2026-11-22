use std::num::NonZeroU32;

use post_proto::{
    ChannelKind, ChannelName, ClientMessage, Code, ErrorCode, Invitation, ListedChannel, Name,
    Role, ServerMessage, TextError, Timestamp,
};

use super::{Channel, Entry, Session};

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
fn starts_with_a_name_and_no_problem() {
    let session = Session::new(name("ana"));

    assert_eq!(session.name(), &name("ana"));
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
        session.compose(&channel("general"), "Hello"),
        Ok(ClientMessage::Send {
            reference: Some(1),
            channel: "general".to_owned(),
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
    let mut session = session_with(&["general"]);
    session.apply(join("general", "ben", 1));
    session.apply(message("general", "ben", 2, "Hi"));
    session.apply(message("general", "cy", 3, "Hello"));
    session.apply(leave("general", "ben", 4));

    session.apply(rename("ben", "benito", 5));
    session.apply(rename("benito", "beni", 6));

    assert_eq!(
        entries(&session, "general"),
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
    let mut session = session_with(&["general"]);
    session.apply(message("general", "ana", 1, "Hello"));

    session.apply(ServerMessage::Rename {
        reference: Some(4),
        old: name("ana"),
        new: name("anna"),
        time: time(2),
    });

    assert_eq!(session.name(), &name("anna"));
    assert!(matches!(
        &entries(&session, "general")[0],
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
fn ignores_what_occurs_in_a_channel_that_the_user_is_not_in() {
    let mut session = session_with(&["general"]);

    session.apply(join("garden", "ben", 1));
    session.apply(message("garden", "ben", 2, "Hi"));
    session.apply(leave("garden", "ben", 3));
    session.apply(leave("garden", "ana", 4));

    assert_eq!(entries(&session, "general"), []);
    assert_eq!(session.channels().count(), 1);
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
    assert_eq!(session.problem(), None);
}

#[test]
fn makes_a_send_message_with_a_new_number_each_time() {
    let mut session = Session::new(name("ana"));
    let general = channel("general");

    assert_eq!(
        session.compose(&general, "Hello"),
        Ok(ClientMessage::Send {
            reference: Some(1),
            channel: "general".to_owned(),
            text: "Hello".to_owned(),
        })
    );
    assert_eq!(
        session.compose(&general, "  spaces stay  "),
        Ok(ClientMessage::Send {
            reference: Some(2),
            channel: "general".to_owned(),
            text: "  spaces stay  ".to_owned(),
        })
    );
}

#[test]
fn does_not_make_a_message_from_a_text_that_breaks_the_rules() {
    let mut session = Session::new(name("ana"));
    let general = channel("general");

    assert_eq!(session.compose(&general, "   "), Err(TextError::Empty));
    assert_eq!(
        session.compose(&general, &"x".repeat(4001)),
        Err(TextError::TooLong)
    );
    assert!(matches!(
        session.compose(&general, "Hello"),
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
}

#[test]
fn forgets_the_error_when_the_user_sends_a_new_text() {
    let mut session = Session::new(name("ana"));
    let general = channel("general");
    session.apply(ServerMessage::error(Some(1), ErrorCode::TextTooLong));

    assert_eq!(session.compose(&general, " "), Err(TextError::Empty));
    assert_eq!(session.problem(), Some(ErrorCode::TextTooLong));

    assert!(session.compose(&general, "Hello").is_ok());
    assert_eq!(session.problem(), None);
}

#[test]
fn starts_the_numbers_again_after_the_largest_number() {
    let mut session = Session::with_next_reference(name("ana"), u32::MAX);
    let general = channel("general");

    assert!(matches!(
        session.compose(&general, "one"),
        Ok(ClientMessage::Send {
            reference: Some(u32::MAX),
            ..
        })
    ));
    assert!(matches!(
        session.compose(&general, "two"),
        Ok(ClientMessage::Send {
            reference: Some(0),
            ..
        })
    ));
}

fn channel(text: &str) -> ChannelName {
    ChannelName::parse(text).expect(text)
}

fn state(title: &str, kind: ChannelKind, topic: &str, owner: &str) -> ServerMessage {
    ServerMessage::Channel {
        reference: None,
        channel: channel(title),
        kind,
        topic: topic.to_owned(),
        owner: name(owner),
        created: time(0),
    }
}

fn session_with(channels: &[&str]) -> Session {
    let mut session = Session::new(name("ana"));
    for title in channels {
        session.apply(state(title, ChannelKind::Public, "", "ana"));
    }
    session
}

fn entries<'a>(session: &'a Session, title: &str) -> &'a [Entry] {
    session
        .channel(&channel(title))
        .expect("a channel of the user")
        .entries()
}

#[test]
fn starts_with_no_channels() {
    let session = Session::new(name("ana"));

    assert_eq!(session.channels().count(), 0);
    assert!(session.channel(&channel("general")).is_none());
    assert!(!session.is_ready());
    assert_eq!(session.listed(), None);
}

#[test]
fn keeps_the_state_of_each_channel_of_the_user_in_the_sequence_of_the_names() {
    let mut session = Session::new(name("ana"));

    session.apply(state("general", ChannelKind::Public, "Plans", "ben"));
    session.apply(state("dev", ChannelKind::Private, "", "ana"));
    assert!(!session.is_ready());
    session.apply(ServerMessage::Ready {});

    assert!(session.is_ready());
    let names: Vec<&str> = session.channels().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["dev", "general"]);
    let general = session
        .channel(&channel("general"))
        .expect("a channel of the user");
    assert_eq!(general.kind(), ChannelKind::Public);
    assert_eq!(general.topic(), "Plans");
    assert_eq!(general.owner(), &name("ben"));
    assert_eq!(general.created(), time(0));
    assert_eq!(general.entries(), []);
    let dev = session.channel(&channel("dev"));
    assert_eq!(dev.map(Channel::kind), Some(ChannelKind::Private));
}

#[test]
fn records_what_occurs_in_each_channel_in_its_own_timeline() {
    let mut session = session_with(&["general", "dev"]);

    session.apply(join("general", "ben", 1));
    session.apply(message("general", "ben", 2, "Hi"));
    session.apply(message("dev", "ana", 3, "Notes"));
    session.apply(leave("general", "ben", 4));
    session.apply(message("garden", "cy", 5, "Hello"));

    assert_eq!(
        entries(&session, "general"),
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
                time: time(4),
                name: name("ben"),
            },
        ]
    );
    assert_eq!(
        entries(&session, "dev"),
        [Entry::Message {
            time: time(3),
            from: name("ana"),
            text: "Notes".to_owned(),
        }]
    );
}

#[test]
fn keeps_the_timeline_when_the_server_sends_the_state_of_a_channel_again() {
    let mut session = session_with(&["general"]);
    session.apply(message("general", "ana", 1, "Hello"));

    session.apply(state("general", ChannelKind::Private, "New topic", "ben"));

    let general = session
        .channel(&channel("general"))
        .expect("a channel of the user");
    assert_eq!(general.kind(), ChannelKind::Private);
    assert_eq!(general.topic(), "New topic");
    assert_eq!(general.owner(), &name("ben"));
    assert_eq!(general.entries().len(), 1);
    assert_eq!(session.channels().count(), 1);
}

#[test]
fn forgets_a_channel_when_the_user_leaves_it() {
    let mut session = session_with(&["general", "dev"]);
    session.apply(message("general", "ana", 1, "Hello"));

    session.apply(leave("general", "ana", 2));

    assert!(session.channel(&channel("general")).is_none());
    assert_eq!(session.channels().count(), 1);
    session.apply(message("general", "ben", 3, "Hi"));
    assert!(session.channel(&channel("general")).is_none());
    assert_eq!(entries(&session, "dev"), []);
}

#[test]
fn makes_each_channel_request_with_a_new_number() {
    let mut session = Session::new(name("ana"));
    let general = channel("general");

    assert_eq!(
        session.create_channel(&general, ChannelKind::Private),
        ClientMessage::ChannelCreate {
            reference: Some(1),
            channel: "general".to_owned(),
            kind: ChannelKind::Private,
        }
    );
    assert_eq!(
        session.join(&general),
        ClientMessage::Join {
            reference: Some(2),
            channel: "general".to_owned(),
        }
    );
    assert_eq!(
        session.leave(&general),
        ClientMessage::Leave {
            reference: Some(3),
            channel: "general".to_owned(),
        }
    );
    assert_eq!(
        session.ask_for_channels(),
        ClientMessage::Channels { reference: Some(4) }
    );
    assert_eq!(
        session.compose(&general, "Hello"),
        Ok(ClientMessage::Send {
            reference: Some(5),
            channel: "general".to_owned(),
            text: "Hello".to_owned(),
        })
    );
    assert_eq!(session.compose(&general, "  "), Err(TextError::Empty));
    assert_eq!(
        session.compose(&general, &"x".repeat(4001)),
        Err(TextError::TooLong)
    );
}

#[test]
fn forgets_the_error_when_the_user_makes_a_channel_request() {
    let general = channel("general");
    let requests: [fn(&mut Session, &ChannelName) -> ClientMessage; 4] = [
        |session, name| session.create_channel(name, ChannelKind::Public),
        |session, name| session.join(name),
        |session, name| session.leave(name),
        |session, _| session.ask_for_channels(),
    ];

    for request in requests {
        let mut session = Session::new(name("ana"));
        session.apply(ServerMessage::error(Some(1), ErrorCode::NameTaken));

        let _message = request(&mut session, &general);

        assert_eq!(session.problem(), None);
    }
}

fn listed(title: &str, members: u32) -> ListedChannel {
    ListedChannel {
        channel: channel(title),
        kind: ChannelKind::Public,
        topic: String::new(),
        members,
    }
}

fn part(reference: u32, list: Vec<ListedChannel>, more: bool) -> ServerMessage {
    ServerMessage::Channels {
        reference: Some(reference),
        list,
        more,
    }
}

#[test]
fn collects_the_parts_of_the_channel_list() {
    let mut session = Session::new(name("ana"));
    let _request = session.ask_for_channels();

    session.apply(part(1, vec![listed("dev", 1), listed("general", 2)], true));
    assert_eq!(session.listed(), None);
    session.apply(part(1, vec![listed("talk", 3)], false));

    assert_eq!(
        session.listed(),
        Some([listed("dev", 1), listed("general", 2), listed("talk", 3)].as_slice())
    );
}

#[test]
fn shows_the_earlier_channel_list_until_a_new_list_is_complete() {
    let mut session = Session::new(name("ana"));
    let _request = session.ask_for_channels();
    session.apply(part(1, vec![listed("dev", 1)], false));

    let _request = session.ask_for_channels();
    session.apply(part(2, vec![listed("general", 2)], true));
    assert_eq!(session.listed(), Some([listed("dev", 1)].as_slice()));
    session.apply(part(2, Vec::new(), false));

    assert_eq!(session.listed(), Some([listed("general", 2)].as_slice()));
}

#[test]
fn ignores_a_part_of_a_channel_list_that_is_not_the_answer_to_the_last_request() {
    let mut session = Session::new(name("ana"));

    session.apply(part(1, vec![listed("dev", 1)], false));
    assert_eq!(session.listed(), None);

    let _first = session.ask_for_channels();
    session.apply(part(1, vec![listed("dev", 1)], true));
    let _second = session.ask_for_channels();
    session.apply(part(1, vec![listed("old", 1)], false));
    assert_eq!(session.listed(), None);
    session.apply(ServerMessage::Channels {
        reference: None,
        list: vec![listed("no-number", 1)],
        more: false,
    });
    assert_eq!(session.listed(), None);

    session.apply(part(2, vec![listed("general", 2)], false));
    assert_eq!(session.listed(), Some([listed("general", 2)].as_slice()));
    session.apply(part(2, vec![listed("late", 1)], false));
    assert_eq!(session.listed(), Some([listed("general", 2)].as_slice()));
}

#[test]
fn shows_the_new_name_of_a_user_in_each_channel_that_shows_the_user() {
    let mut session = Session::new(name("ana"));
    session.apply(state("general", ChannelKind::Public, "", "ben"));
    session.apply(state("dev", ChannelKind::Public, "", "ana"));
    session.apply(state("quiet", ChannelKind::Public, "", "ana"));
    session.apply(message("dev", "ben", 1, "Hi"));

    session.apply(rename("ben", "benito", 2));

    let renamed = Entry::Renamed {
        time: time(2),
        old: name("ben"),
        new: name("benito"),
    };
    let general = session
        .channel(&channel("general"))
        .expect("a channel of the user");
    assert_eq!(general.owner(), &name("benito"));
    assert_eq!(general.entries(), std::slice::from_ref(&renamed));
    assert_eq!(
        entries(&session, "dev"),
        [
            Entry::Message {
                time: time(1),
                from: name("benito"),
                text: "Hi".to_owned(),
            },
            renamed,
        ]
    );
    assert_eq!(entries(&session, "quiet"), []);

    session.apply(rename("benito", "beni", 3));
    assert_eq!(entries(&session, "dev").len(), 3);
    assert_eq!(entries(&session, "quiet"), []);
}

#[test]
fn takes_the_new_name_of_its_own_account_in_each_channel() {
    let mut session = session_with(&["general"]);
    session.apply(join("general", "ana", 1));

    session.apply(rename("ana", "anna", 2));

    assert_eq!(session.name(), &name("anna"));
    let general = session
        .channel(&channel("general"))
        .expect("a channel of the user");
    assert_eq!(general.owner(), &name("anna"));
    assert_eq!(
        general.entries()[0],
        Entry::Joined {
            time: time(1),
            name: name("anna"),
        }
    );
    session.apply(leave("general", "ana", 3));
    assert_eq!(session.channels().count(), 1);
    session.apply(leave("general", "anna", 4));
    assert_eq!(session.channels().count(), 0);
}
