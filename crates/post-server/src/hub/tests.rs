use std::num::NonZeroU32;

use post_crypto::{Exchange, Seed};
use post_proto::{
    ClientMessage, Code, ErrorCode, Invitation, Key, LOBBY, Name, Nonce, Part, Role, ServerMessage,
    Signature, Timestamp,
};

use post_rules::{Change, Storage as _};

use super::{ConnectionId, Hub, Outgoing, Random, Settings};
use crate::store::{Store, StoreError};

mod channels;

fn ready() -> ServerMessage {
    ServerMessage::Ready {}
}

fn now() -> Timestamp {
    Timestamp::parse("2026-11-21T14:02:07.120Z").expect("valid")
}

fn later() -> Timestamp {
    Timestamp::parse("2026-11-21T14:03:00.000Z").expect("valid")
}

fn name(text: &str) -> Name {
    Name::parse(text).expect(text)
}

fn send(reference: Option<u32>, channel: &str, text: &str) -> String {
    ClientMessage::Send {
        reference,
        channel: channel.to_owned(),
        text: text.to_owned(),
    }
    .encode()
}

fn to(to: ConnectionId, message: ServerMessage) -> Outgoing {
    Outgoing { to, message }
}

fn join(who: &str) -> ServerMessage {
    ServerMessage::Join {
        channel: LOBBY.to_owned(),
        name: name(who),
        time: now(),
    }
}

fn error(reference: Option<u32>, code: ErrorCode) -> ServerMessage {
    ServerMessage::error(reference, code)
}

fn settings() -> Settings {
    Settings {
        addresses: vec!["chat.example.org:443".to_owned()],
        link: "https://chat.example.org".to_owned(),
        requires: Vec::new(),
    }
}

fn new_hub() -> Hub {
    let store = Store::in_memory().expect("a database in memory");
    Hub::new(store, settings(), random()).expect("a hub")
}

fn hub_with(names: &[&str]) -> (Hub, Vec<ConnectionId>) {
    let mut hub = new_hub();
    let link = owner_code(&mut hub);
    let mut ids = Vec::new();
    for (person, name) in (1..).zip(names) {
        let code = if person == 1 {
            link.clone()
        } else {
            invitation(&mut hub, 100 + u32::from(person))
        };
        ids.push(registered(&mut hub, person, name, &code));
    }
    (hub, ids)
}

#[test]
fn gives_each_connection_a_different_identifier() {
    let mut hub = new_hub();
    let first = hub.connect();
    let second = hub.connect();
    assert_ne!(first, second);
}

#[test]
fn sends_nothing_to_a_connection_before_its_sign_in() {
    let (mut hub, ids) = hub_with(&["ana"]);
    let open = invitation(&mut hub, 50);
    let silent = hub.connect();
    let waiting = hub.connect();
    challenged(&mut hub, waiting, &[]);
    let ben = hub.connect();
    let server = challenged(&mut hub, ben, &[]);

    let line = register(&hub, 2, &server, "ben", &open);
    let joined = hub.receive(ben, &line, now());
    let sent = hub.receive(ids[0], &send(None, "lobby", "Hello"), now());
    let left = hub.disconnect(ben, later());

    assert_eq!((joined.len(), sent.len(), left.len()), (4, 2, 1));
    for outgoing in joined.iter().chain(&sent).chain(&left) {
        assert_ne!(outgoing.to, silent);
        assert_ne!(outgoing.to, waiting);
    }
}

#[test]
fn sends_a_text_to_each_user_and_gives_the_number_only_to_the_sender() {
    let (mut hub, ids) = hub_with(&["ana", "ben", "cy"]);

    let outgoing = hub.receive(ids[1], &send(Some(4), "lobby", "Hello"), now());

    let message = |reference| ServerMessage::Message {
        reference,
        channel: "lobby".to_owned(),
        from: name("ben"),
        time: now(),
        text: "Hello".to_owned(),
    };
    assert_eq!(
        outgoing,
        [
            to(ids[0], message(None)),
            to(ids[1], message(Some(4))),
            to(ids[2], message(None)),
        ]
    );
}

#[test]
fn keeps_the_text_as_the_user_wrote_it() {
    let (mut hub, ids) = hub_with(&["ana"]);

    let outgoing = hub.receive(ids[0], &send(None, "lobby", "  two\n  lines  "), now());

    assert_eq!(
        outgoing,
        [to(
            ids[0],
            ServerMessage::Message {
                reference: None,
                channel: "lobby".to_owned(),
                from: name("ana"),
                time: now(),
                text: "  two\n  lines  ".to_owned(),
            }
        )]
    );
}

#[test]
fn refuses_a_line_that_is_not_a_message() {
    let (mut hub, ids) = hub_with(&["ana"]);

    for line in ["", "hello", r#"{"type":"dance","ref":3}"#] {
        assert_eq!(
            hub.receive(ids[0], line, now()),
            [to(ids[0], error(None, ErrorCode::BadMessage))],
            "{line}"
        );
    }
}

#[test]
fn refuses_data_that_is_not_text() {
    let mut hub = new_hub();
    let ana = hub.connect();

    assert_eq!(
        hub.refuse(ana),
        [to(ana, error(None, ErrorCode::BadMessage))]
    );
}

#[test]
fn refuses_a_text_before_the_hello_message() {
    let mut hub = new_hub();
    let ana = hub.connect();

    assert_eq!(
        hub.receive(ana, &send(Some(2), "nowhere", ""), now()),
        [to(ana, error(Some(2), ErrorCode::HelloRequired))]
    );
}

#[test]
fn refuses_a_hello_message_after_the_welcome() {
    let (mut hub, ids) = hub_with(&["ana"]);

    let invalid = ClientMessage::Hello {
        reference: Some(6),
        protocol: 99,
        address: "false.example.org:1".to_owned(),
        nonce: client_nonce(),
        supports: Vec::new(),
    }
    .encode();

    assert_eq!(
        hub.receive(ids[0], &invalid, now()),
        [to(ids[0], error(Some(6), ErrorCode::DuplicateHello))]
    );
    assert_eq!(
        hub.receive(ids[0], &hello(7, ADDRESS, &[]), now()),
        [to(ids[0], error(Some(7), ErrorCode::DuplicateHello))]
    );
}

#[test]
fn refuses_a_name_that_breaks_the_rules() {
    let (mut hub, _) = hub_with(&["ana"]);
    let open = invitation(&mut hub, 50);
    let new = hub.connect();
    let server = challenged(&mut hub, new, &[]);

    for invalid in ["Ana", "a", "a name", "abcdefghij0123456789x"] {
        let line = register(&hub, 2, &server, invalid, &open);
        assert_eq!(
            hub.receive(new, &line, now()),
            [to(new, error(Some(2), ErrorCode::InvalidName))],
            "{invalid}"
        );
    }
}

#[test]
fn refuses_a_channel_that_does_not_exist() {
    let (mut hub, ids) = hub_with(&["ana"]);

    for channel in ["garden", "#lobby", "Lobby", ""] {
        assert_eq!(
            hub.receive(ids[0], &send(Some(8), channel, ""), now()),
            [to(ids[0], error(Some(8), ErrorCode::NoSuchChannel))],
            "{channel}"
        );
    }
}

#[test]
fn refuses_a_text_that_breaks_the_rules() {
    let (mut hub, ids) = hub_with(&["ana", "ben"]);

    assert_eq!(
        hub.receive(ids[0], &send(Some(1), "lobby", " \n "), now()),
        [to(ids[0], error(Some(1), ErrorCode::EmptyText))]
    );
    assert_eq!(
        hub.receive(ids[0], &send(None, "lobby", &"x".repeat(4001)), now()),
        [to(ids[0], error(None, ErrorCode::TextTooLong))]
    );
}

#[test]
fn tells_the_other_users_when_a_user_leaves() {
    let (mut hub, ids) = hub_with(&["ana", "ben", "cy"]);

    let outgoing = hub.disconnect(ids[1], later());

    let leave = ServerMessage::Leave {
        reference: None,
        channel: "lobby".to_owned(),
        name: name("ben"),
        time: later(),
    };
    assert_eq!(outgoing, [to(ids[0], leave.clone()), to(ids[2], leave)]);
}

#[test]
fn is_silent_when_a_connection_without_a_name_closes() {
    let (mut hub, _) = hub_with(&["ana"]);
    let silent = hub.connect();

    assert_eq!(hub.disconnect(silent, later()), []);
}

#[test]
fn ignores_a_connection_that_it_does_not_know() {
    let (mut hub, ids) = hub_with(&["ana"]);
    hub.disconnect(ids[0], later());

    assert_eq!(hub.receive(ids[0], &hello(1, ADDRESS, &[]), now()), []);
    assert_eq!(hub.refuse(ids[0]), []);
    assert_eq!(hub.disconnect(ids[0], later()), []);
}

const ADDRESS: &str = "chat.example.org:443";

fn random() -> Random {
    let mut nonces = 0_u8;
    let mut codes = 0_u32;
    Random {
        nonce: Box::new(move || {
            nonces += 1;
            Nonce::from_bytes([nonces; 32])
        }),
        code: Box::new(move || {
            codes += 1;
            code(codes)
        }),
    }
}

fn code(number: u32) -> Code {
    Code::parse(&format!("code{number:08}")).expect("a valid code")
}

fn seed(person: u8) -> Seed {
    Seed::from_bytes([person; 32])
}

fn client_nonce() -> Nonce {
    Nonce::from_bytes([200; 32])
}

fn hello(reference: u32, address: &str, supports: &[&str]) -> String {
    ClientMessage::Hello {
        reference: Some(reference),
        protocol: 0,
        address: address.to_owned(),
        nonce: client_nonce(),
        supports: supports
            .iter()
            .map(|word| Part::parse(word).expect(word))
            .collect(),
    }
    .encode()
}

fn challenged(hub: &mut Hub, id: ConnectionId, supports: &[&str]) -> Nonce {
    match &hub.receive(id, &hello(1, ADDRESS, supports), now())[..] {
        [
            Outgoing {
                message: ServerMessage::Challenge { nonce, .. },
                ..
            },
        ] => *nonce,
        other => panic!("the server sends a challenge, not {other:?}"),
    }
}

fn credentials(hub: &Hub, person: u8, address: &str, server: &Nonce) -> (Key, Signature) {
    let account = seed(person).account_key(&hub.key.public());
    let client = client_nonce();
    let exchange = Exchange {
        address,
        client: &client,
        server,
    };
    (
        account.public(),
        account.prove(&hub.key.public(), &exchange),
    )
}

fn identify(hub: &Hub, person: u8, server: &Nonce, invitation: Option<&Code>) -> String {
    let (key, proof) = credentials(hub, person, ADDRESS, server);
    ClientMessage::Identify {
        reference: Some(2),
        key,
        proof,
        invitation: invitation.cloned(),
    }
    .encode()
}

fn register(hub: &Hub, person: u8, server: &Nonce, who: &str, invitation: &Code) -> String {
    let (key, proof) = credentials(hub, person, ADDRESS, server);
    ClientMessage::Register {
        reference: Some(2),
        key,
        proof,
        name: who.to_owned(),
        invitation: invitation.clone(),
    }
    .encode()
}

fn welcome(who: &str, role: Role) -> ServerMessage {
    ServerMessage::Welcome {
        reference: Some(2),
        name: name(who),
        role,
    }
}

fn owner_code(hub: &mut Hub) -> Code {
    let invite = hub
        .owner_link(now())
        .expect("stored")
        .expect("the server has no owner");
    Code::parse(invite.rsplit('/').next().expect("a invite has a code")).expect("a valid code")
}

fn invitation(hub: &mut Hub, number: u32) -> Code {
    let owner = seed(1).account_key(&hub.key.public()).public();
    let owner = hub
        .rules
        .account(owner.as_bytes())
        .expect("the owner has an account")
        .id;
    hub.rules
        .invite(&mut hub.store, owner, &code(number), None, None, now())
        .expect("stored")
        .expect("the owner can invite")
        .code
}

fn new_owner_code(hub: &mut Hub, number: u32) -> Code {
    hub.rules
        .new_owner_code(&mut hub.store, &code(number), now())
        .expect("stored")
        .expect("the code is new");
    code(number)
}

fn registered(hub: &mut Hub, person: u8, who: &str, invitation: &Code) -> ConnectionId {
    let id = hub.connect();
    let server = challenged(hub, id, &[]);
    let line = register(hub, person, &server, who, invitation);
    let outgoing = hub.receive(id, &line, now());
    assert!(
        matches!(outgoing[0].message, ServerMessage::Welcome { .. }),
        "{outgoing:?}"
    );
    id
}

fn with_owner() -> (Hub, ConnectionId) {
    let mut hub = new_hub();
    let invite = owner_code(&mut hub);
    let ana = registered(&mut hub, 1, "ana", &invite);
    (hub, ana)
}

#[test]
fn answers_a_hello_with_a_challenge_that_proves_the_server_key() {
    let mut hub = new_hub();
    let id = hub.connect();

    let outgoing = hub.receive(id, &hello(7, ADDRESS, &["voice"]), now());

    let [
        Outgoing {
            to,
            message:
                ServerMessage::Challenge {
                    reference,
                    protocol,
                    key,
                    nonce,
                    proof,
                    requires,
                },
        },
    ] = &outgoing[..]
    else {
        panic!("the server sends one challenge, not {outgoing:?}");
    };
    assert_eq!((*to, *reference, *protocol), (id, Some(7), 0));
    assert_eq!(*key, hub.key.public());
    assert_eq!(*nonce, Nonce::from_bytes([1; 32]));
    assert_eq!(requires, &[]);
    let client = client_nonce();
    let exchange = Exchange {
        address: ADDRESS,
        client: &client,
        server: nonce,
    };
    assert!(exchange.server_proof_is_valid(key, proof));
}

#[test]
fn makes_a_different_random_value_for_each_connection() {
    let mut hub = new_hub();
    let first = hub.connect();
    let second = hub.connect();

    assert_ne!(
        challenged(&mut hub, first, &[]),
        challenged(&mut hub, second, &[])
    );
}

#[test]
fn refuses_a_hello_that_it_cannot_accept() {
    let mut hub = new_hub();
    let id = hub.connect();
    let other_version = ClientMessage::Hello {
        reference: Some(1),
        protocol: 1,
        address: ADDRESS.to_owned(),
        nonce: client_nonce(),
        supports: Vec::new(),
    }
    .encode();

    assert_eq!(
        hub.receive(id, &other_version, now()),
        [to(id, error(Some(1), ErrorCode::UnsupportedProtocol))]
    );
    for address in [
        "false.example.org:443",
        "chat.example.org:444",
        "chat.example.org",
        "",
    ] {
        assert_eq!(
            hub.receive(id, &hello(4, address, &[]), now()),
            [to(id, error(Some(4), ErrorCode::WrongAddress))],
            "{address}"
        );
    }

    challenged(&mut hub, id, &[]);
    assert_eq!(
        hub.receive(id, &hello(5, ADDRESS, &[]), now()),
        [to(id, error(Some(5), ErrorCode::DuplicateHello))]
    );
}

#[test]
fn gives_an_owner_link_with_the_address_of_the_server() {
    let mut hub = new_hub();

    let first = hub.owner_link(now()).expect("stored");
    let second = hub.owner_link(later()).expect("stored");

    assert_eq!(
        first.as_deref(),
        Some("https://chat.example.org/i/code00000001")
    );
    assert_eq!(second, first);
}

#[test]
fn gives_the_link_of_a_code_only_while_its_invitation_is_open() {
    let mut hub = new_hub();
    let owner = owner_code(&mut hub);
    assert_eq!(
        hub.open_link(&owner, now()).as_deref(),
        Some("https://chat.example.org/i/code00000001")
    );
    assert_eq!(hub.open_link(&code(99), now()), None);

    let ana = registered(&mut hub, 1, "ana", &owner);
    assert_eq!(hub.open_link(&owner, now()), None);

    hub.receive(ana, &invite(3, Some(60), None), now());
    assert!(hub.open_link(&code(2), later()).is_some());
    let end = Timestamp::parse("2026-11-21T14:03:07.120Z").expect("valid");
    assert_eq!(hub.open_link(&code(2), end), None);
}

#[test]
fn makes_the_user_of_the_owner_link_the_server_owner() {
    let mut hub = new_hub();
    let invite = owner_code(&mut hub);
    let id = hub.connect();
    let server = challenged(&mut hub, id, &[]);

    let line = register(&hub, 1, &server, "ana", &invite);
    let outgoing = hub.receive(id, &line, now());

    assert_eq!(
        outgoing,
        [
            to(id, welcome("ana", Role::Owner)),
            to(id, ready()),
            to(id, join("ana"))
        ]
    );
    assert_eq!(hub.owner_link(now()).expect("stored"), None);
}

#[test]
fn makes_an_invited_user_a_member_and_tells_the_lobby() {
    let (mut hub, ana) = with_owner();
    let code = invitation(&mut hub, 50);
    let ben = hub.connect();
    let server = challenged(&mut hub, ben, &[]);

    let line = register(&hub, 2, &server, "ben", &code);
    let outgoing = hub.receive(ben, &line, now());

    assert_eq!(
        outgoing,
        [
            to(ben, welcome("ben", Role::Member)),
            to(ben, ready()),
            to(ana, join("ben")),
            to(ben, join("ben")),
        ]
    );
}

#[test]
fn signs_in_a_user_who_has_an_account() {
    let (mut hub, ana) = with_owner();
    hub.disconnect(ana, now());
    let id = hub.connect();
    let server = challenged(&mut hub, id, &[]);

    let line = identify(&hub, 1, &server, None);
    let outgoing = hub.receive(id, &line, now());

    assert_eq!(
        outgoing,
        [
            to(id, welcome("ana", Role::Owner)),
            to(id, ready()),
            to(id, join("ana"))
        ]
    );
}

#[test]
fn refuses_a_key_that_has_no_account() {
    let (mut hub, _ana) = with_owner();
    let id = hub.connect();
    let server = challenged(&mut hub, id, &[]);

    let line = identify(&hub, 2, &server, None);

    assert_eq!(
        hub.receive(id, &line, now()),
        [to(id, error(Some(2), ErrorCode::UnknownAccount))]
    );
}

#[test]
fn refuses_a_sign_in_message_at_an_incorrect_moment() {
    let (mut hub, ana) = with_owner();
    let invite = code(1);
    let new = hub.connect();

    for line in [
        identify(&hub, 1, &client_nonce(), None),
        register(&hub, 2, &client_nonce(), "ben", &invite),
    ] {
        assert_eq!(
            hub.receive(new, &line, now()),
            [to(new, error(Some(2), ErrorCode::HelloRequired))]
        );
        assert_eq!(
            hub.receive(ana, &line, now()),
            [to(ana, error(Some(2), ErrorCode::DuplicateIdentify))]
        );
    }

    challenged(&mut hub, new, &[]);
    assert_eq!(
        hub.receive(new, &send(Some(8), LOBBY, "Hello"), now()),
        [to(new, error(Some(8), ErrorCode::IdentifyRequired))]
    );
}

#[test]
fn refuses_a_proof_that_is_not_for_this_connection() {
    let (mut hub, ana) = with_owner();
    hub.disconnect(ana, now());
    let invite = invitation(&mut hub, 50);
    let id = hub.connect();
    let server = challenged(&mut hub, id, &[]);
    let earlier = Nonce::from_bytes([1; 32]);
    assert_ne!(server, earlier);

    let (key, _) = credentials(&hub, 1, ADDRESS, &server);
    let (_, for_other_address) = credentials(&hub, 1, "false.example.org:443", &server);
    let (_, from_other_seed) = credentials(&hub, 2, ADDRESS, &server);
    let lines = [
        identify(&hub, 1, &earlier, None),
        register(&hub, 2, &earlier, "ben", &invite),
        ClientMessage::Identify {
            reference: Some(2),
            key,
            proof: for_other_address,
            invitation: None,
        }
        .encode(),
        ClientMessage::Identify {
            reference: Some(2),
            key,
            proof: from_other_seed,
            invitation: None,
        }
        .encode(),
    ];
    for line in lines {
        assert_eq!(
            hub.receive(id, &line, now()),
            [to(id, error(Some(2), ErrorCode::InvalidProof))],
            "{line}"
        );
    }

    let line = identify(&hub, 1, &server, None);
    assert_eq!(
        hub.receive(id, &line, now())[0],
        to(id, welcome("ana", Role::Owner))
    );
}

#[test]
fn refuses_a_new_account_that_breaks_a_rule() {
    let (mut hub, _ana) = with_owner();
    let open = invitation(&mut hub, 50);
    let id = hub.connect();
    let server = challenged(&mut hub, id, &[]);

    let attempts = [
        (2, "Not A Name", open.clone(), ErrorCode::InvalidName),
        (2, "ben", code(99), ErrorCode::InvalidInvitation),
        (2, "ben", code(1), ErrorCode::InvalidInvitation),
        (2, "ana", open.clone(), ErrorCode::NameTaken),
        (1, "ana-2", open.clone(), ErrorCode::AccountExists),
    ];
    for (person, who, invitation, refused) in attempts {
        let line = register(&hub, person, &server, who, &invitation);
        assert_eq!(
            hub.receive(id, &line, now()),
            [to(id, error(Some(2), refused))],
            "{who}"
        );
    }

    let line = register(&hub, 2, &server, "ben", &open);
    assert_eq!(
        hub.receive(id, &line, now())[0],
        to(id, welcome("ben", Role::Member))
    );
}

#[test]
fn makes_an_account_that_exists_the_owner_with_a_new_owner_link() {
    let (mut hub, ana) = with_owner();
    let open = invitation(&mut hub, 50);
    let ben = registered(&mut hub, 2, "ben", &open);
    hub.disconnect(ana, now());
    hub.disconnect(ben, now());
    new_owner_code(&mut hub, 60);

    let id = hub.connect();
    let server = challenged(&mut hub, id, &[]);
    let line = identify(&hub, 2, &server, Some(&code(60)));
    assert_eq!(
        hub.receive(id, &line, now())[0],
        to(id, welcome("ben", Role::Owner))
    );

    let id = hub.connect();
    let server = challenged(&mut hub, id, &[]);
    let line = identify(&hub, 1, &server, Some(&code(60)));
    assert_eq!(
        hub.receive(id, &line, now())[0],
        to(id, welcome("ana", Role::Member))
    );
}

#[test]
fn ignores_an_invitation_at_the_sign_in_of_a_member() {
    let (mut hub, ana) = with_owner();
    let open = invitation(&mut hub, 50);
    let ben = registered(&mut hub, 2, "ben", &open);
    hub.disconnect(ben, now());

    let id = hub.connect();
    let server = challenged(&mut hub, id, &[]);
    let line = identify(&hub, 2, &server, Some(&open));
    let outgoing = hub.receive(id, &line, now());

    assert_eq!(
        outgoing,
        [
            to(id, welcome("ben", Role::Member)),
            to(id, ready()),
            to(ana, join("ben")),
            to(id, join("ben")),
        ]
    );
    assert_eq!(hub.rules.open_invitations(now())[0].used, 1);
}

#[test]
fn refuses_a_client_that_does_not_support_a_required_part() {
    let store = Store::in_memory().expect("a database in memory");
    let requires = vec![Part::parse("sealed").expect("valid")];
    let settings = Settings {
        requires: requires.clone(),
        ..settings()
    };
    let mut hub = Hub::new(store, settings, random()).expect("a hub");
    let invite = owner_code(&mut hub);

    let id = hub.connect();
    match &hub.receive(id, &hello(1, ADDRESS, &[]), now())[..] {
        [
            Outgoing {
                message: ServerMessage::Challenge { requires: sent, .. },
                ..
            },
        ] => assert_eq!(sent, &requires),
        other => panic!("{other:?}"),
    }
    let server = Nonce::from_bytes([1; 32]);
    let line = register(&hub, 1, &server, "ana", &invite);
    assert_eq!(
        hub.receive(id, &line, now()),
        [to(id, error(Some(2), ErrorCode::MissingPart))]
    );

    let id = hub.connect();
    let server = challenged(&mut hub, id, &["voice", "sealed"]);
    let line = register(&hub, 1, &server, "ana", &invite);
    assert_eq!(
        hub.receive(id, &line, now())[0],
        to(id, welcome("ana", Role::Owner))
    );
    hub.disconnect(id, now());

    let id = hub.connect();
    let server = challenged(&mut hub, id, &["voice"]);
    let line = identify(&hub, 1, &server, None);
    assert_eq!(
        hub.receive(id, &line, now()),
        [to(id, error(Some(2), ErrorCode::MissingPart))]
    );
}

#[test]
fn tells_the_lobby_one_time_about_a_user_with_two_connections() {
    let (mut hub, first) = with_owner();
    let open = invitation(&mut hub, 50);
    let ben = registered(&mut hub, 2, "ben", &open);

    let second = hub.connect();
    let server = challenged(&mut hub, second, &[]);
    let line = identify(&hub, 1, &server, None);
    assert_eq!(
        hub.receive(second, &line, now()),
        [to(second, welcome("ana", Role::Owner)), to(second, ready())]
    );

    let delivered = hub.receive(second, &send(Some(4), LOBBY, "Hello"), now());
    let receivers: Vec<ConnectionId> = delivered.iter().map(|outgoing| outgoing.to).collect();
    assert_eq!(receivers, [first, ben, second]);

    assert_eq!(hub.disconnect(first, later()), []);
    assert_eq!(
        hub.disconnect(second, later()),
        [to(
            ben,
            ServerMessage::Leave {
                reference: None,
                channel: LOBBY.to_owned(),
                name: name("ana"),
                time: later(),
            }
        )]
    );
}

#[test]
fn answers_with_a_server_error_when_the_storage_fails() {
    let (mut hub, ana) = with_owner();
    let open = invitation(&mut hub, 50);
    let link = new_owner_code(&mut hub, 60);
    hub.disconnect(ana, now());
    let id = hub.connect();
    let server = challenged(&mut hub, id, &[]);
    hub.store.lose_its_tables();

    for line in [
        identify(&hub, 1, &server, Some(&link)),
        register(&hub, 2, &server, "ben", &open),
    ] {
        assert_eq!(
            hub.receive(id, &line, now()),
            [to(id, error(Some(2), ErrorCode::ServerError))]
        );
    }
}

#[test]
fn needs_no_storage_for_a_sign_in_that_changes_nothing() {
    let (mut hub, ana) = with_owner();
    hub.disconnect(ana, now());
    let id = hub.connect();
    let server = challenged(&mut hub, id, &[]);
    hub.store.lose_its_tables();

    let line = identify(&hub, 1, &server, None);

    assert_eq!(
        hub.receive(id, &line, now())[0],
        to(id, welcome("ana", Role::Owner))
    );
}

#[test]
fn reports_an_owner_link_that_it_cannot_store() {
    let mut hub = new_hub();
    hub.store.lose_its_tables();

    assert!(matches!(
        hub.owner_link(now()),
        Err(StoreError::Database(_))
    ));
}

#[test]
fn reports_a_new_code_that_an_invitation_has() {
    let mut store = Store::in_memory().expect("a database in memory");
    store
        .write(&[Change::PutInvitation(stored_invitation(1, Some(5), 0))])
        .expect("stored");
    let mut hub = Hub::new(store, settings(), random()).expect("a hub");

    assert!(matches!(hub.owner_link(now()), Err(StoreError::CodeInUse)));
}

#[test]
fn does_not_start_with_data_that_breaks_a_rule() {
    let mut store = Store::in_memory().expect("a database in memory");
    store
        .write(&[Change::PutInvitation(stored_invitation(1, Some(1), 2))])
        .expect("stored");

    let result = Hub::new(store, settings(), random());

    assert!(matches!(result, Err(StoreError::BrokenRules)));
}

fn invite(reference: u32, lifetime: Option<u32>, uses: Option<u32>) -> String {
    ClientMessage::Invite {
        reference: Some(reference),
        lifetime: lifetime.and_then(NonZeroU32::new),
        uses: uses.and_then(NonZeroU32::new),
    }
    .encode()
}

fn list(reference: u32) -> String {
    ClientMessage::Invitations {
        reference: Some(reference),
    }
    .encode()
}

fn revoke(reference: u32, code: &Code) -> String {
    ClientMessage::Revoke {
        reference: Some(reference),
        code: code.clone(),
    }
    .encode()
}

fn shown(number: u32, created: Timestamp) -> Invitation {
    Invitation {
        code: code(number),
        link: format!("https://chat.example.org/i/{}", code(number)),
        created,
        end: None,
        uses: None,
        used: 0,
    }
}

fn invitations(reference: u32, list: Vec<Invitation>) -> ServerMessage {
    ServerMessage::Invitations {
        reference: Some(reference),
        list,
    }
}

#[test]
fn makes_an_invitation_for_the_owner_and_gives_the_newest_one_first() {
    let (mut hub, ana) = with_owner();

    let first = hub.receive(ana, &invite(3, Some(604_800), Some(2)), now());
    let second = hub.receive(ana, &invite(4, None, None), later());

    let limited = Invitation {
        end: Some(Timestamp::parse("2026-11-28T14:02:07.120Z").expect("valid")),
        uses: Some(2),
        ..shown(2, now())
    };
    assert_eq!(first, [to(ana, invitations(3, vec![limited.clone()]))]);
    assert_eq!(
        second,
        [to(ana, invitations(4, vec![shown(3, later()), limited]))]
    );
}

#[test]
fn lets_a_person_make_an_account_with_an_invitation_of_the_owner() {
    let (mut hub, ana) = with_owner();
    hub.receive(ana, &invite(3, None, Some(1)), now());
    hub.receive(ana, &invite(4, None, None), now());

    registered(&mut hub, 2, "ben", &code(2));
    registered(&mut hub, 3, "cy", &code(3));

    let still_open = Invitation {
        used: 1,
        ..shown(3, now())
    };
    assert_eq!(
        hub.receive(ana, &list(5), now()),
        [to(ana, invitations(5, vec![still_open]))]
    );
    let id = hub.connect();
    let server = challenged(&mut hub, id, &[]);
    let line = register(&hub, 4, &server, "dee", &code(2));
    assert_eq!(
        hub.receive(id, &line, now()),
        [to(id, error(Some(2), ErrorCode::InvalidInvitation))]
    );
}

#[test]
fn does_not_show_an_invitation_after_its_end() {
    let (mut hub, ana) = with_owner();
    hub.receive(ana, &invite(3, Some(60), None), now());
    let end = Timestamp::parse("2026-11-21T14:03:07.120Z").expect("valid");
    let open = Invitation {
        end: Some(end),
        ..shown(2, now())
    };

    assert_eq!(
        hub.receive(ana, &list(4), later()),
        [to(ana, invitations(4, vec![open]))]
    );
    assert_eq!(
        hub.receive(ana, &list(5), end),
        [to(ana, invitations(5, Vec::new()))]
    );
}

#[test]
fn cancels_an_invitation_for_the_owner() {
    let (mut hub, ana) = with_owner();
    hub.receive(ana, &invite(3, None, None), now());
    hub.receive(ana, &invite(4, None, None), now());
    let link = new_owner_code(&mut hub, 60);

    assert_eq!(
        hub.receive(ana, &revoke(5, &code(2)), later()),
        [to(ana, invitations(5, vec![shown(3, now())]))]
    );
    for not_open in [code(2), code(99), link.clone()] {
        assert_eq!(
            hub.receive(ana, &revoke(6, &not_open), later()),
            [to(ana, error(Some(6), ErrorCode::InvalidInvitation))],
            "{not_open}"
        );
    }
    assert!(hub.rules.code_is_open(&link, later()));
}

#[test]
fn lets_only_the_owner_use_the_invitation_messages() {
    let (mut hub, _ana) = with_owner();
    let open = invitation(&mut hub, 50);
    let ben = registered(&mut hub, 2, "ben", &open);

    for line in [invite(1, None, None), list(1), revoke(1, &open)] {
        assert_eq!(
            hub.receive(ben, &line, now()),
            [to(ben, error(Some(1), ErrorCode::NotPermitted))],
            "{line}"
        );
    }
    assert_eq!(hub.rules.open_invitations(now()).len(), 1);
}

#[test]
fn obeys_the_present_owner_and_not_the_role_at_the_sign_in() {
    let (mut hub, ana) = with_owner();
    let open = invitation(&mut hub, 50);
    let ben = registered(&mut hub, 2, "ben", &open);
    hub.disconnect(ben, now());
    new_owner_code(&mut hub, 60);
    let ben = hub.connect();
    let server = challenged(&mut hub, ben, &[]);
    let line = identify(&hub, 2, &server, Some(&code(60)));
    hub.receive(ben, &line, now());

    assert_eq!(
        hub.receive(ana, &list(1), now()),
        [to(ana, error(Some(1), ErrorCode::NotPermitted))]
    );
    let used = Invitation {
        used: 1,
        ..shown(50, now())
    };
    assert_eq!(
        hub.receive(ben, &list(1), now()),
        [to(ben, invitations(1, vec![used]))]
    );
}

#[test]
fn refuses_an_invitation_message_before_the_sign_in() {
    let mut hub = new_hub();
    let id = hub.connect();
    let lines = [invite(1, None, None), list(1), revoke(1, &code(5))];

    for line in &lines {
        assert_eq!(
            hub.receive(id, line, now()),
            [to(id, error(Some(1), ErrorCode::HelloRequired))]
        );
    }
    challenged(&mut hub, id, &[]);
    for line in &lines {
        assert_eq!(
            hub.receive(id, line, now()),
            [to(id, error(Some(1), ErrorCode::IdentifyRequired))]
        );
    }
}

#[test]
fn answers_with_a_server_error_when_it_cannot_store_an_invitation() {
    let (mut hub, ana) = with_owner();
    hub.receive(ana, &invite(3, None, None), now());
    hub.store.lose_its_tables();

    for line in [invite(4, None, None), revoke(4, &code(2))] {
        assert_eq!(
            hub.receive(ana, &line, now()),
            [to(ana, error(Some(4), ErrorCode::ServerError))]
        );
    }
    assert_eq!(
        hub.receive(ana, &list(5), now()),
        [to(ana, invitations(5, vec![shown(2, now())]))]
    );
}

#[test]
fn answers_with_a_server_error_when_a_new_code_is_not_free() {
    let store = Store::in_memory().expect("a database in memory");
    let random = Random {
        code: Box::new(|| code(7)),
        ..random()
    };
    let mut hub = Hub::new(store, settings(), random).expect("a hub");
    let link = owner_code(&mut hub);
    let ana = registered(&mut hub, 1, "ana", &link);

    assert_eq!(
        hub.receive(ana, &invite(3, None, None), now()),
        [to(ana, error(Some(3), ErrorCode::ServerError))]
    );
}

fn rename(reference: u32, name: &str) -> String {
    ClientMessage::Rename {
        reference: Some(reference),
        name: name.to_owned(),
    }
    .encode()
}

fn sender_of_a_text(hub: &mut Hub, from: ConnectionId) -> Name {
    let sent = hub.receive(from, &send(None, LOBBY, "Hello"), later());
    match &sent[0].message {
        ServerMessage::Message { from, .. } => from.clone(),
        other => panic!("the server sends the text, not {other:?}"),
    }
}

#[test]
fn changes_the_name_of_a_user_and_tells_each_user() {
    let (mut hub, ids) = hub_with(&["ana", "ben"]);

    let outgoing = hub.receive(ids[1], &rename(5, "benito"), later());

    let renamed = |reference| ServerMessage::Rename {
        reference,
        old: name("ben"),
        new: name("benito"),
        time: later(),
    };
    assert_eq!(
        outgoing,
        [to(ids[0], renamed(None)), to(ids[1], renamed(Some(5)))]
    );
    assert_eq!(sender_of_a_text(&mut hub, ids[1]), name("benito"));
    assert_eq!(sender_of_a_text(&mut hub, ids[0]), name("ana"));
}

#[test]
fn gives_the_new_name_to_each_connection_of_the_account() {
    let (mut hub, ids) = hub_with(&["ana", "ben"]);
    let second = hub.connect();
    let server = challenged(&mut hub, second, &[]);
    let line = identify(&hub, 1, &server, None);
    hub.receive(second, &line, now());

    let outgoing = hub.receive(ids[0], &rename(5, "anna"), later());

    assert_eq!(outgoing.len(), 3);
    assert_eq!(sender_of_a_text(&mut hub, second), name("anna"));
    hub.disconnect(ids[0], later());
    assert_eq!(
        hub.disconnect(second, later()),
        [to(
            ids[1],
            ServerMessage::Leave {
                reference: None,
                channel: LOBBY.to_owned(),
                name: name("anna"),
                time: later(),
            }
        )]
    );

    let again = hub.connect();
    let server = challenged(&mut hub, again, &[]);
    let line = identify(&hub, 1, &server, None);
    assert_eq!(
        hub.receive(again, &line, later())[0],
        to(again, welcome("anna", Role::Owner))
    );
}

#[test]
fn refuses_a_name_change_that_breaks_a_rule() {
    let (mut hub, ids) = hub_with(&["ana", "ben"]);

    for (wanted, code) in [
        ("Not A Name", ErrorCode::InvalidName),
        ("a", ErrorCode::InvalidName),
        ("ben", ErrorCode::NameTaken),
        ("ana", ErrorCode::NameTaken),
    ] {
        assert_eq!(
            hub.receive(ids[0], &rename(1, wanted), now()),
            [to(ids[0], error(Some(1), code))],
            "{wanted}"
        );
    }

    assert_eq!(hub.receive(ids[0], &rename(2, "anna"), now()).len(), 2);
    assert_eq!(
        hub.receive(ids[0], &rename(3, "annie"), later()),
        [to(ids[0], error(Some(3), ErrorCode::TooSoon))]
    );
    assert_eq!(
        hub.receive(ids[1], &rename(4, "ana"), later()),
        [to(ids[1], error(Some(4), ErrorCode::NameTaken))]
    );
    assert_eq!(sender_of_a_text(&mut hub, ids[0]), name("anna"));
    assert_eq!(sender_of_a_text(&mut hub, ids[1]), name("ben"));
}

#[test]
fn refuses_a_name_change_before_the_sign_in() {
    let mut hub = new_hub();
    let id = hub.connect();

    assert_eq!(
        hub.receive(id, &rename(1, "anna"), now()),
        [to(id, error(Some(1), ErrorCode::HelloRequired))]
    );
    challenged(&mut hub, id, &[]);
    assert_eq!(
        hub.receive(id, &rename(1, "anna"), now()),
        [to(id, error(Some(1), ErrorCode::IdentifyRequired))]
    );
}

#[test]
fn keeps_the_name_when_it_cannot_store_the_change() {
    let (mut hub, ana) = with_owner();
    hub.store.lose_its_tables();

    assert_eq!(
        hub.receive(ana, &rename(4, "anna"), now()),
        [to(ana, error(Some(4), ErrorCode::ServerError))]
    );
    assert_eq!(sender_of_a_text(&mut hub, ana), name("ana"));
}

fn stored_invitation(number: u32, limit: Option<u32>, used: u32) -> post_rules::Invitation {
    post_rules::Invitation {
        code: code(number),
        owner: false,
        maker: None,
        created: now(),
        end: None,
        uses: limit,
        used,
        revoked: None,
    }
}
