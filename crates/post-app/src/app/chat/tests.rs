use std::num::NonZeroU32;

use post_client::{Destination, Entry, Fingerprint, KnownServer, Link, Outgoing, Trust};
use post_proto::{
    ClientMessage, Code, ErrorCode, Invitation, Key, Name, NameError, Role, ServerMessage,
    Timestamp,
};

use super::{Account, Chat, Invitations, Lifetime, Message, Open, Panel, Uses};
use crate::app::Effect;
use crate::app::servers::{Form, Target};
use crate::i18n::Text;

type Outbox = Box<dyn FnMut() -> Option<ClientMessage>>;

fn sent_message(outgoing: Outgoing) -> ClientMessage {
    match outgoing {
        Outgoing::Message(message) => message,
        Outgoing::Line(line) => panic!("a typed line and not a message: {line}"),
    }
}
fn name(text: &str) -> Name {
    Name::parse(text).expect(text)
}

fn code(text: &str) -> Code {
    Code::parse(text).expect(text)
}

fn time() -> Timestamp {
    Timestamp::parse("2026-11-21T14:02:00.000Z").expect("valid")
}

fn chat_of(role: Role, address: &str, capacity: usize) -> (Chat, Outbox) {
    let form = Form {
        address: address.to_owned(),
        name: "ana".to_owned(),
        problem: None,
    };
    let target = Target {
        attempt: 3,
        destination: Destination::parse(address).expect("valid"),
        name: Some(name("ana")),
        trust: Trust::default(),
    };
    let (link, mut messages) = Link::channel(name("ana"), role, Key::from_bytes([1; 32]), capacity);
    let chat = Chat::new(form, target, link);
    (
        chat,
        Box::new(move || messages.try_recv().ok().map(sent_message)),
    )
}

fn chat(capacity: usize) -> (Chat, Outbox) {
    chat_of(Role::Member, "localhost", capacity)
}

fn owner_with_invitations() -> (Chat, Outbox) {
    let (mut chat, mut outbox) = chat_of(Role::Owner, "localhost", 8);
    chat.update(Message::Open(Panel::Invitations));
    assert_eq!(
        outbox(),
        Some(ClientMessage::Invitations { reference: Some(1) })
    );
    (chat, outbox)
}

fn invitation(text: &str) -> Invitation {
    Invitation {
        code: code(text),
        link: format!("https://chat.example.org/i/{text}"),
        created: time(),
        end: None,
        uses: None,
        used: 0,
    }
}

fn list(invitations: Vec<Invitation>) -> ServerMessage {
    ServerMessage::Invitations {
        reference: None,
        list: invitations,
    }
}

fn renamed(old: &str, new: &str) -> ServerMessage {
    ServerMessage::Rename {
        reference: None,
        old: name(old),
        new: name(new),
        time: time(),
    }
}

#[test]
fn starts_with_the_name_that_the_server_gave() {
    let (chat, _outbox) = chat(4);

    assert_eq!(chat.session().name(), &name("ana"));
    assert_eq!(chat.draft(), "");
    assert_eq!(chat.notice(), None);
    assert_eq!(chat.target().attempt, 3);
    assert_eq!(chat.panel(), None);
    assert!(!chat.is_owner());
}

#[test]
fn describes_its_server_for_the_profile() {
    let (plain, _outbox) = chat(4);
    assert_eq!(
        plain.server(),
        KnownServer {
            address: "localhost:7678".to_owned(),
            key: Key::from_bytes([1; 32]),
            name: name("ana"),
            tls: false,
            certificate: None,
        }
    );

    let (link, _messages) = Link::channel(name("ana"), Role::Owner, Key::from_bytes([2; 32]), 4);
    let certificate = Fingerprint::from_bytes([9; 32]);
    let target = Target {
        attempt: 1,
        destination: Destination::parse("https://localhost:8443").expect("valid"),
        name: None,
        trust: Trust::default(),
    };
    let secure = Chat::new(
        Form::default(),
        target,
        link.with_certificate(Some(certificate)),
    );
    assert_eq!(
        secure.server(),
        KnownServer {
            address: "localhost:8443".to_owned(),
            key: Key::from_bytes([2; 32]),
            name: name("ana"),
            tls: true,
            certificate: Some("09".repeat(32)),
        }
    );
    assert!(secure.is_owner());
}

#[test]
fn records_what_the_server_sends() {
    let (mut chat, _outbox) = chat(4);

    let effect = chat.receive(ServerMessage::Join {
        channel: "lobby".to_owned(),
        name: name("ben"),
        time: time(),
    });

    assert_eq!(effect, Effect::None);
    assert_eq!(
        chat.session().entries(),
        [Entry::Joined {
            time: time(),
            name: name("ben"),
        }]
    );
}

#[test]
fn sends_the_draft_and_makes_the_field_empty() {
    let (mut chat, mut outbox) = chat(4);
    chat.update(Message::DraftChanged("Hello".to_owned()));
    assert_eq!(chat.draft(), "Hello");

    assert_eq!(chat.update(Message::Send), Effect::None);

    assert_eq!(
        outbox(),
        Some(ClientMessage::Send {
            reference: Some(1),
            channel: "lobby".to_owned(),
            text: "Hello".to_owned(),
        })
    );
    assert_eq!(outbox(), None);
    assert_eq!(chat.draft(), "");
    assert_eq!(chat.notice(), None);
}

#[test]
fn does_not_send_an_empty_draft() {
    let (mut chat, mut outbox) = chat(4);
    chat.update(Message::DraftChanged("   ".to_owned()));

    chat.update(Message::Send);

    assert_eq!(outbox(), None);
    assert_eq!(chat.draft(), "   ");
    assert_eq!(chat.notice(), None);
}

#[test]
fn tells_the_user_that_a_draft_is_too_long() {
    let (mut chat, mut outbox) = chat(4);
    let long = "x".repeat(4001);
    chat.update(Message::DraftChanged(long.clone()));

    chat.update(Message::Send);

    assert_eq!(outbox(), None);
    assert_eq!(chat.draft(), long);
    assert_eq!(chat.notice(), Some(Text::ErrorTextTooLong));

    chat.update(Message::DraftChanged("short".to_owned()));
    assert_eq!(chat.notice(), None);
}

#[test]
fn keeps_the_draft_when_the_connection_cannot_take_it() {
    let (mut chat, mut outbox) = chat(1);
    chat.update(Message::DraftChanged("one".to_owned()));
    chat.update(Message::Send);
    chat.update(Message::DraftChanged("two".to_owned()));

    chat.update(Message::Send);

    assert_eq!(chat.draft(), "two");
    assert_eq!(chat.notice(), Some(Text::ChatNotSent));
    assert!(outbox().is_some());
    assert_eq!(outbox(), None);
}

#[test]
fn shows_an_error_from_the_server_until_the_user_sends_again() {
    let (mut chat, _outbox) = chat(4);

    chat.receive(ServerMessage::error(Some(1), ErrorCode::NoSuchChannel));
    assert_eq!(chat.notice(), Some(Text::ErrorNoSuchChannel));

    chat.update(Message::DraftChanged("Hello".to_owned()));
    assert_eq!(chat.notice(), Some(Text::ErrorNoSuchChannel));
    chat.update(Message::Send);
    assert_eq!(chat.notice(), None);
}

#[test]
fn tells_the_user_that_the_profile_did_not_save_the_server() {
    let (mut chat, _outbox) = chat(4);

    chat.warn_that_the_server_is_not_saved();
    assert_eq!(chat.notice(), Some(Text::ChatServerNotSaved));

    chat.update(Message::DraftChanged("Hello".to_owned()));
    assert_eq!(chat.notice(), None);
}

#[test]
fn gives_the_form_back_when_the_user_leaves() {
    let (mut chat, _outbox) = chat(4);
    assert_eq!(chat.update(Message::Leave), Effect::None);

    let form = chat.into_form();

    assert_eq!(form.address, "localhost");
    assert_eq!(form.name, "ana");
}

#[test]
fn asks_the_server_for_the_invitations_when_the_panel_opens() {
    let (mut chat, mut outbox) = chat_of(Role::Owner, "localhost", 8);

    let effect = chat.update(Message::Open(Panel::Invitations));

    assert_eq!(effect, Effect::None);
    assert_eq!(
        chat.panel(),
        Some(&Open::Invitations(Invitations {
            lifetime: Lifetime::Week,
            uses: Uses::NoLimit,
            copied: None,
        }))
    );
    assert_eq!(
        outbox(),
        Some(ClientMessage::Invitations { reference: Some(1) })
    );
    assert_eq!(chat.session().invitations(), None);

    chat.receive(list(vec![invitation("7Hq2kdX9mPq4")]));
    assert_eq!(
        chat.session().invitations(),
        Some([invitation("7Hq2kdX9mPq4")].as_slice())
    );
}

#[test]
fn makes_an_invitation_with_the_limits_that_the_user_selects() {
    let (mut chat, mut outbox) = owner_with_invitations();

    chat.update(Message::Invite);
    chat.update(Message::LifetimeSelected(Lifetime::Day));
    chat.update(Message::UsesSelected(Uses::Five));
    chat.update(Message::Invite);
    chat.update(Message::LifetimeSelected(Lifetime::Forever));
    chat.update(Message::UsesSelected(Uses::One));
    chat.update(Message::Invite);

    let invite = |reference, lifetime, uses| ClientMessage::Invite {
        reference: Some(reference),
        lifetime: NonZeroU32::new(lifetime),
        uses: NonZeroU32::new(uses),
    };
    assert_eq!(outbox(), Some(invite(2, 604_800, 0)));
    assert_eq!(outbox(), Some(invite(3, 86_400, 5)));
    assert_eq!(outbox(), Some(invite(4, 0, 1)));
    assert_eq!(outbox(), None);
}

#[test]
fn each_limit_has_a_value_for_the_server() {
    let seconds: Vec<u32> = Lifetime::ALL
        .iter()
        .map(|lifetime| lifetime.seconds().map_or(0, NonZeroU32::get))
        .collect();
    assert_eq!(seconds, [86_400, 604_800, 2_592_000, 0]);
    let counts: Vec<u32> = Uses::ALL
        .iter()
        .map(|uses| uses.count().map_or(0, NonZeroU32::get))
        .collect();
    assert_eq!(counts, [1, 5, 25, 0]);
    let texts: Vec<Text> = Lifetime::ALL
        .iter()
        .map(|lifetime| lifetime.text())
        .collect();
    assert_eq!(
        texts,
        [
            Text::InvitationsLifetimeDay,
            Text::InvitationsLifetimeWeek,
            Text::InvitationsLifetimeMonth,
            Text::InvitationsLifetimeForever,
        ]
    );
}

#[test]
fn cancels_an_invitation_when_the_user_asks() {
    let (mut chat, mut outbox) = owner_with_invitations();

    chat.update(Message::Revoke(code("7Hq2kdX9mPq4")));

    assert_eq!(
        outbox(),
        Some(ClientMessage::Revoke {
            reference: Some(2),
            code: code("7Hq2kdX9mPq4"),
        })
    );
}

#[test]
fn gives_the_link_of_an_invitation_to_the_clipboard() {
    let (mut chat, _outbox) = owner_with_invitations();
    assert_eq!(
        chat.update(Message::Copy(code("7Hq2kdX9mPq4"))),
        Effect::None
    );
    chat.receive(list(vec![
        invitation("k3Wm8sTz2LpQ"),
        invitation("7Hq2kdX9mPq4"),
    ]));

    let effect = chat.update(Message::Copy(code("7Hq2kdX9mPq4")));

    assert_eq!(
        effect,
        Effect::Copy("https://chat.example.org/i/7Hq2kdX9mPq4".to_owned())
    );
    let Some(Open::Invitations(invitations)) = chat.panel() else {
        panic!("the invitations panel is open");
    };
    assert_eq!(invitations.copied, Some(code("7Hq2kdX9mPq4")));
    assert_eq!(
        chat.update(Message::Copy(code("AAAAAAAAAAAA"))),
        Effect::None
    );

    chat.update(Message::Invite);
    let Some(Open::Invitations(invitations)) = chat.panel() else {
        panic!("the invitations panel is open");
    };
    assert_eq!(invitations.copied, None);
}

#[test]
fn shows_an_answer_of_the_server_to_a_request() {
    let (mut chat, _outbox) = owner_with_invitations();

    chat.receive(ServerMessage::error(Some(1), ErrorCode::NotPermitted));
    assert_eq!(chat.notice(), Some(Text::ErrorNotPermitted));

    chat.update(Message::Invite);
    assert_eq!(chat.notice(), None);
}

#[test]
fn tells_the_user_that_the_connection_cannot_take_a_request() {
    let (mut chat, mut outbox) = chat_of(Role::Owner, "localhost", 1);
    chat.update(Message::Open(Panel::Invitations));

    chat.update(Message::Invite);

    assert_eq!(chat.notice(), Some(Text::ChatRequestNotSent));
    assert!(outbox().is_some());
    assert_eq!(outbox(), None);
}

#[test]
fn closes_a_panel_and_goes_back_to_the_message_field() {
    let (mut chat, _outbox) = owner_with_invitations();

    assert_eq!(chat.update(Message::Close), Effect::FocusDraft);

    assert_eq!(chat.panel(), None);
    assert_eq!(chat.update(Message::Close), Effect::None);
}

#[test]
fn opens_the_account_panel_with_the_present_name() {
    let (mut chat, mut outbox) = chat(4);

    let effect = chat.update(Message::Open(Panel::Account));

    assert_eq!(effect, Effect::FocusPanel);
    assert_eq!(
        chat.panel(),
        Some(&Open::Account(Account {
            name: "ana".to_owned(),
            problem: None,
        }))
    );
    assert_eq!(outbox(), None);
}

#[test]
fn asks_the_server_for_a_new_name() {
    let (mut chat, mut outbox) = chat(4);
    chat.update(Message::Open(Panel::Account));
    chat.update(Message::NameChanged("  anna ".to_owned()));

    assert_eq!(chat.update(Message::Rename), Effect::None);

    assert_eq!(
        outbox(),
        Some(ClientMessage::Rename {
            reference: Some(1),
            name: "anna".to_owned(),
        })
    );
    assert!(matches!(chat.panel(), Some(Open::Account(_))));
}

#[test]
fn tells_the_user_about_a_new_name_that_breaks_the_rules() {
    let (mut chat, mut outbox) = chat(4);
    chat.update(Message::Open(Panel::Account));
    chat.update(Message::NameChanged("a".to_owned()));

    chat.update(Message::Rename);

    assert_eq!(outbox(), None);
    let problem = |chat: &Chat| match chat.panel() {
        Some(Open::Account(account)) => account.problem,
        _ => panic!("the account panel is open"),
    };
    assert_eq!(problem(&chat), Some(NameError::TooShort));

    chat.update(Message::NameChanged("An".to_owned()));
    assert_eq!(problem(&chat), None);
    chat.update(Message::Rename);
    assert_eq!(problem(&chat), Some(NameError::InvalidCharacter));
}

#[test]
fn closes_the_account_panel_when_the_name_does_not_change() {
    let (mut chat, mut outbox) = chat(4);
    chat.update(Message::Open(Panel::Account));
    chat.update(Message::NameChanged(" ana ".to_owned()));

    assert_eq!(chat.update(Message::Rename), Effect::FocusDraft);

    assert_eq!(outbox(), None);
    assert_eq!(chat.panel(), None);
}

#[test]
fn forgets_an_error_when_a_panel_opens_or_closes() {
    let (mut chat, _outbox) = chat(4);
    chat.receive(ServerMessage::error(Some(1), ErrorCode::NoSuchChannel));
    chat.update(Message::Open(Panel::Account));
    assert_eq!(chat.notice(), None);

    chat.receive(ServerMessage::error(Some(1), ErrorCode::TooSoon));
    assert_eq!(chat.notice(), Some(Text::ErrorTooSoon));
    chat.update(Message::Close);
    assert_eq!(chat.notice(), None);
}

#[test]
fn takes_its_new_name_and_closes_the_account_panel() {
    let (mut chat, _outbox) = chat(4);
    chat.update(Message::Open(Panel::Account));
    chat.update(Message::NameChanged("anna".to_owned()));
    chat.update(Message::Rename);

    let effect = chat.receive(renamed("ana", "anna"));

    let expected = KnownServer {
        address: "localhost:7678".to_owned(),
        key: Key::from_bytes([1; 32]),
        name: name("anna"),
        tls: false,
        certificate: None,
    };
    assert_eq!(effect, Effect::Remember(expected));
    assert_eq!(chat.session().name(), &name("anna"));
    assert_eq!(chat.panel(), None);
}

#[test]
fn keeps_its_panel_when_a_different_user_changes_the_name() {
    let (mut chat, _outbox) = chat(4);
    chat.update(Message::Open(Panel::Account));

    assert_eq!(chat.receive(renamed("ben", "benito")), Effect::None);

    assert!(matches!(chat.panel(), Some(Open::Account(_))));
    assert_eq!(chat.session().name(), &name("ana"));

    let (mut chat, _outbox) = owner_with_invitations();
    assert!(matches!(
        chat.receive(renamed("ana", "anna")),
        Effect::Remember(_)
    ));
    assert!(matches!(chat.panel(), Some(Open::Invitations(_))));
}

#[test]
fn ignores_a_panel_message_when_that_panel_is_not_open() {
    let (mut chat, mut outbox) = chat(4);

    for message in [
        Message::LifetimeSelected(Lifetime::Day),
        Message::UsesSelected(Uses::One),
        Message::Invite,
        Message::Revoke(code("7Hq2kdX9mPq4")),
        Message::Copy(code("7Hq2kdX9mPq4")),
        Message::NameChanged("anna".to_owned()),
        Message::Rename,
    ] {
        assert_eq!(chat.update(message), Effect::None);
    }

    assert_eq!(outbox(), None);
    assert_eq!(chat.panel(), None);
}
