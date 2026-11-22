use post_proto::{
    ChannelKind, ChannelName, ClientMessage, ErrorCode, LOBBY, ListedChannel, Role, ServerMessage,
};
use post_rules::{AccountId, ChannelChange, ChannelHead, ChannelId, ChannelStorage as _, Kind};
use post_testing::Directory;

use super::{
    challenged, error, hub_with, identify, later, name, new_hub, now, owner_code, random, ready,
    registered, send, settings, to, welcome, with_owner,
};
use crate::hub::{ConnectionId, Hub};
use crate::store::{Store, StoreError};

fn create(reference: u32, channel: &str, kind: ChannelKind) -> String {
    ClientMessage::ChannelCreate {
        reference: Some(reference),
        channel: channel.to_owned(),
        kind,
    }
    .encode()
}

fn channel(text: &str) -> ChannelName {
    ChannelName::parse(text).expect(text)
}

fn state(reference: Option<u32>, title: &str, kind: ChannelKind, owner: &str) -> ServerMessage {
    ServerMessage::Channel {
        reference,
        channel: channel(title),
        kind,
        topic: String::new(),
        owner: name(owner),
        created: now(),
    }
}

fn signed_in_again(hub: &mut Hub, person: u8) -> ConnectionId {
    let id = hub.connect();
    let server = challenged(hub, id, &[]);
    let line = identify(hub, person, &server, None);
    hub.receive(id, &line, now());
    id
}

#[test]
fn makes_a_channel_and_gives_its_state_to_each_connection_of_the_owner() {
    let (mut hub, ids) = hub_with(&["ana", "ben"]);
    let second = signed_in_again(&mut hub, 1);

    let outgoing = hub.receive(ids[0], &create(7, "general", ChannelKind::Public), now());

    assert_eq!(
        outgoing,
        [
            to(
                ids[0],
                state(Some(7), "general", ChannelKind::Public, "ana")
            ),
            to(second, state(None, "general", ChannelKind::Public, "ana")),
        ]
    );
}

#[test]
fn refuses_a_channel_that_breaks_a_rule() {
    let (mut hub, ids) = hub_with(&["ana", "ben"]);
    hub.receive(ids[0], &create(1, "general", ChannelKind::Public), now());
    let too_long = "x".repeat(31);

    for (wanted, code) in [
        ("General", ErrorCode::InvalidChannelName),
        ("g", ErrorCode::InvalidChannelName),
        ("#general", ErrorCode::InvalidChannelName),
        ("", ErrorCode::InvalidChannelName),
        (too_long.as_str(), ErrorCode::InvalidChannelName),
        ("general", ErrorCode::NameTaken),
    ] {
        for kind in [ChannelKind::Public, ChannelKind::Private] {
            assert_eq!(
                hub.receive(ids[1], &create(2, wanted, kind), now()),
                [to(ids[1], error(Some(2), code))],
                "{wanted}"
            );
        }
    }
}

#[test]
fn keeps_the_name_of_the_temporary_lobby_free_of_a_real_channel() {
    let (mut hub, ana) = with_owner();

    assert_eq!(
        hub.receive(ana, &create(3, LOBBY, ChannelKind::Public), now()),
        [to(ana, error(Some(3), ErrorCode::NameTaken))]
    );
}

#[test]
fn lets_an_account_own_ten_channels() {
    let (mut hub, ids) = hub_with(&["ana", "ben"]);
    for number in 0..10 {
        let title = format!("channel-{number}");
        let outgoing = hub.receive(ids[0], &create(number, &title, ChannelKind::Private), now());
        assert_eq!(
            outgoing,
            [to(
                ids[0],
                state(Some(number), &title, ChannelKind::Private, "ana")
            )]
        );
    }

    assert_eq!(
        hub.receive(ids[0], &create(11, "one-more", ChannelKind::Public), now()),
        [to(ids[0], error(Some(11), ErrorCode::TooManyChannels))]
    );
    assert_eq!(
        hub.receive(ids[1], &create(12, "one-more", ChannelKind::Public), now()),
        [to(
            ids[1],
            state(Some(12), "one-more", ChannelKind::Public, "ben")
        )]
    );
}

#[test]
fn refuses_a_new_channel_before_the_sign_in() {
    let mut hub = new_hub();
    let id = hub.connect();
    let line = create(1, "general", ChannelKind::Public);

    assert_eq!(
        hub.receive(id, &line, now()),
        [to(id, error(Some(1), ErrorCode::HelloRequired))]
    );
    challenged(&mut hub, id, &[]);
    assert_eq!(
        hub.receive(id, &line, now()),
        [to(id, error(Some(1), ErrorCode::IdentifyRequired))]
    );
}

#[test]
fn gives_the_state_of_each_channel_of_the_user_after_the_welcome() {
    let (mut hub, ids) = hub_with(&["ana", "ben"]);
    hub.receive(ids[0], &create(1, "general", ChannelKind::Public), now());
    hub.receive(ids[1], &create(2, "dev", ChannelKind::Private), now());
    hub.receive(ids[0], &create(3, "talk", ChannelKind::Private), now());
    hub.disconnect(ids[0], now());

    let id = hub.connect();
    let server = challenged(&mut hub, id, &[]);
    let line = identify(&hub, 1, &server, None);
    let outgoing = hub.receive(id, &line, later());

    let join = ServerMessage::Join {
        channel: LOBBY.to_owned(),
        name: name("ana"),
        time: later(),
    };
    assert_eq!(
        outgoing,
        [
            to(id, welcome("ana", Role::Owner)),
            to(id, state(None, "general", ChannelKind::Public, "ana")),
            to(id, state(None, "talk", ChannelKind::Private, "ana")),
            to(id, ready()),
            to(ids[1], join.clone()),
            to(id, join),
        ]
    );
}

#[test]
fn keeps_the_channels_when_the_server_starts_again() {
    let directory = Directory::new();
    let path = directory.path().join("post.db");
    {
        let store = Store::open(&path).expect("a new database");
        let mut hub = Hub::new(store, settings(), random()).expect("a hub");
        let link = owner_code(&mut hub);
        let ana = registered(&mut hub, 1, "ana", &link);
        hub.receive(ana, &create(1, "general", ChannelKind::Public), now());
    }

    let store = Store::open(&path).expect("the same database");
    let mut hub = Hub::new(store, settings(), random()).expect("a hub");
    let id = signed_in_again(&mut hub, 1);

    assert_eq!(
        hub.receive(id, &create(2, "general", ChannelKind::Public), now()),
        [to(id, error(Some(2), ErrorCode::NameTaken))]
    );
    let second = hub.connect();
    let server = challenged(&mut hub, second, &[]);
    let line = identify(&hub, 1, &server, None);
    assert_eq!(
        hub.receive(second, &line, now())[1],
        to(second, state(None, "general", ChannelKind::Public, "ana"))
    );
}

#[test]
fn answers_with_a_server_error_when_it_cannot_store_a_channel() {
    let (mut hub, ana) = with_owner();
    hub.store.lose_its_tables();

    assert_eq!(
        hub.receive(ana, &create(4, "general", ChannelKind::Public), now()),
        [to(ana, error(Some(4), ErrorCode::ServerError))]
    );
}

#[test]
fn does_not_start_with_channel_data_that_breaks_a_rule() {
    let directory = Directory::new();
    let path = directory.path().join("post.db");
    {
        let store = Store::open(&path).expect("a new database");
        let mut hub = Hub::new(store, settings(), random()).expect("a hub");
        let link = owner_code(&mut hub);
        registered(&mut hub, 1, "ana", &link);
    }
    let mut store = Store::open(&path).expect("the same database");
    let without_a_member = ChannelHead {
        id: ChannelId(0),
        name: channel("general"),
        kind: Kind::Public,
        owner: AccountId(0),
        topic: String::new(),
        created: now(),
        renamed: None,
    };
    store
        .write(&[
            ChannelChange::PutNext(ChannelId(1)),
            ChannelChange::PutChannel(without_a_member),
        ])
        .expect("stored");

    let result = Hub::new(store, settings(), random());

    assert!(matches!(result, Err(StoreError::BrokenRules)));
}

fn join(reference: u32, channel: &str) -> String {
    ClientMessage::Join {
        reference: Some(reference),
        channel: channel.to_owned(),
    }
    .encode()
}

fn leave(reference: u32, channel: &str) -> String {
    ClientMessage::Leave {
        reference: Some(reference),
        channel: channel.to_owned(),
    }
    .encode()
}

fn joined(title: &str, who: &str) -> ServerMessage {
    ServerMessage::Join {
        channel: title.to_owned(),
        name: name(who),
        time: now(),
    }
}

fn with_general() -> (Hub, Vec<ConnectionId>) {
    let (mut hub, ids) = hub_with(&["ana", "ben", "cy"]);
    hub.receive(ids[0], &create(1, "general", ChannelKind::Public), now());
    (hub, ids)
}

#[test]
fn a_user_joins_a_public_channel_and_each_member_gets_the_news() {
    let (mut hub, ids) = with_general();
    let second = signed_in_again(&mut hub, 2);

    let outgoing = hub.receive(ids[1], &join(5, "general"), now());

    assert_eq!(
        outgoing,
        [
            to(
                ids[1],
                state(Some(5), "general", ChannelKind::Public, "ana")
            ),
            to(second, state(None, "general", ChannelKind::Public, "ana")),
            to(ids[0], joined("general", "ben")),
            to(ids[1], joined("general", "ben")),
            to(second, joined("general", "ben")),
        ]
    );
}

#[test]
fn refuses_a_join_that_breaks_a_rule() {
    let (mut hub, ids) = with_general();
    hub.receive(ids[0], &create(2, "secret", ChannelKind::Private), now());
    hub.receive(ids[1], &join(3, "general"), now());

    for member in [ids[0], ids[1]] {
        assert_eq!(
            hub.receive(member, &join(4, "general"), now()),
            [to(member, error(Some(4), ErrorCode::AlreadyMember))]
        );
    }
    for channel in ["garden", "secret", "General", "#general", "", LOBBY] {
        assert_eq!(
            hub.receive(ids[2], &join(5, channel), now()),
            [to(ids[2], error(Some(5), ErrorCode::NoSuchChannel))],
            "{channel}"
        );
    }
}

#[test]
fn a_member_leaves_a_channel_and_each_member_gets_the_news() {
    let (mut hub, ids) = with_general();
    hub.receive(ids[1], &join(2, "general"), now());
    hub.receive(ids[2], &join(3, "general"), now());

    let outgoing = hub.receive(ids[1], &leave(6, "general"), later());

    let left = |reference| ServerMessage::Leave {
        reference,
        channel: "general".to_owned(),
        name: name("ben"),
        time: later(),
    };
    assert_eq!(
        outgoing,
        [
            to(ids[0], left(None)),
            to(ids[1], left(Some(6))),
            to(ids[2], left(None)),
        ]
    );
    assert_eq!(
        hub.receive(ids[1], &leave(7, "general"), later()),
        [to(ids[1], error(Some(7), ErrorCode::NotMember))]
    );
    let again = hub.receive(ids[1], &join(8, "general"), now());
    assert_eq!(
        again,
        [
            to(
                ids[1],
                state(Some(8), "general", ChannelKind::Public, "ana")
            ),
            to(ids[0], joined("general", "ben")),
            to(ids[2], joined("general", "ben")),
            to(ids[1], joined("general", "ben")),
        ]
    );
}

#[test]
fn refuses_a_leave_that_breaks_a_rule() {
    let (mut hub, ids) = with_general();
    hub.receive(ids[0], &create(2, "secret", ChannelKind::Private), now());

    assert_eq!(
        hub.receive(ids[0], &leave(1, "general"), now()),
        [to(ids[0], error(Some(1), ErrorCode::OwnerCannotLeave))]
    );
    assert_eq!(
        hub.receive(ids[0], &leave(1, "secret"), now()),
        [to(ids[0], error(Some(1), ErrorCode::OwnerCannotLeave))]
    );
    assert_eq!(
        hub.receive(ids[1], &leave(2, "general"), now()),
        [to(ids[1], error(Some(2), ErrorCode::NotMember))]
    );
    for channel in ["garden", "secret", "Not A Name", LOBBY] {
        assert_eq!(
            hub.receive(ids[1], &leave(3, channel), now()),
            [to(ids[1], error(Some(3), ErrorCode::NoSuchChannel))],
            "{channel}"
        );
    }
}

#[test]
fn refuses_a_join_and_a_leave_before_the_sign_in() {
    let (mut hub, _ids) = with_general();
    let id = hub.connect();
    let lines = [join(1, "general"), leave(1, "general")];

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
fn a_user_stays_a_member_when_the_connection_closes() {
    let (mut hub, ids) = with_general();
    hub.receive(ids[1], &join(2, "general"), now());
    hub.disconnect(ids[1], now());

    let id = hub.connect();
    let server = challenged(&mut hub, id, &[]);
    let line = identify(&hub, 2, &server, None);
    let outgoing = hub.receive(id, &line, now());

    assert_eq!(
        outgoing[..3],
        [
            to(id, welcome("ben", Role::Member)),
            to(id, state(None, "general", ChannelKind::Public, "ana")),
            to(id, ready()),
        ]
    );
    assert_eq!(
        hub.receive(id, &join(3, "general"), now()),
        [to(id, error(Some(3), ErrorCode::AlreadyMember))]
    );
}

#[test]
fn answers_with_a_server_error_when_it_cannot_store_a_join_or_a_leave() {
    let (mut hub, ids) = with_general();
    hub.receive(ids[1], &join(2, "general"), now());
    hub.store.lose_its_tables();

    assert_eq!(
        hub.receive(ids[2], &join(3, "general"), now()),
        [to(ids[2], error(Some(3), ErrorCode::ServerError))]
    );
    assert_eq!(
        hub.receive(ids[1], &leave(4, "general"), now()),
        [to(ids[1], error(Some(4), ErrorCode::ServerError))]
    );
}

#[test]
fn sends_a_text_to_each_connection_of_each_member_of_the_channel() {
    let (mut hub, ids) = with_general();
    hub.receive(ids[1], &join(2, "general"), now());
    let second = signed_in_again(&mut hub, 2);

    let outgoing = hub.receive(ids[1], &send(Some(9), "general", "Hello"), later());

    let message = |reference| ServerMessage::Message {
        reference,
        channel: "general".to_owned(),
        from: name("ben"),
        time: later(),
        text: "Hello".to_owned(),
    };
    assert_eq!(
        outgoing,
        [
            to(ids[0], message(None)),
            to(ids[1], message(Some(9))),
            to(second, message(None)),
        ]
    );
}

#[test]
fn refuses_a_text_from_a_user_who_is_not_a_member_of_the_channel() {
    let (mut hub, ids) = with_general();
    hub.receive(ids[0], &create(2, "secret", ChannelKind::Private), now());

    for text in ["Hello", "", " \n "] {
        assert_eq!(
            hub.receive(ids[1], &send(Some(1), "general", text), now()),
            [to(ids[1], error(Some(1), ErrorCode::NotMember))]
        );
    }
    for channel in ["secret", "garden", "#general"] {
        assert_eq!(
            hub.receive(ids[1], &send(Some(2), channel, "Hello"), now()),
            [to(ids[1], error(Some(2), ErrorCode::NoSuchChannel))],
            "{channel}"
        );
    }
}

#[test]
fn examines_the_text_of_a_message_to_a_channel() {
    let (mut hub, ids) = with_general();

    assert_eq!(
        hub.receive(ids[0], &send(Some(1), "general", " \n "), now()),
        [to(ids[0], error(Some(1), ErrorCode::EmptyText))]
    );
    assert_eq!(
        hub.receive(ids[0], &send(None, "general", &"x".repeat(4001)), now()),
        [to(ids[0], error(None, ErrorCode::TextTooLong))]
    );
    assert_eq!(
        hub.receive(ids[0], &send(None, "general", &"x".repeat(4000)), now())
            .len(),
        1
    );
}

#[test]
fn keeps_the_temporary_lobby_and_the_channels_apart() {
    let (mut hub, ids) = with_general();

    let to_the_lobby = hub.receive(ids[1], &send(None, LOBBY, "Hi"), now());
    let to_the_channel = hub.receive(ids[0], &send(None, "general", "Hi"), now());

    assert_eq!(to_the_lobby.len(), 3);
    assert_eq!(to_the_channel.len(), 1);
}

fn list(reference: u32) -> String {
    ClientMessage::Channels {
        reference: Some(reference),
    }
    .encode()
}

fn listed(title: &str, members: u32) -> ListedChannel {
    ListedChannel {
        channel: channel(title),
        kind: ChannelKind::Public,
        topic: String::new(),
        members,
    }
}

#[test]
fn lists_the_public_channels_by_name_with_the_number_of_their_members() {
    let (mut hub, ids) = with_general();
    hub.receive(ids[1], &create(2, "dev", ChannelKind::Public), now());
    hub.receive(ids[0], &create(3, "secret", ChannelKind::Private), now());
    hub.receive(ids[2], &join(4, "general"), now());

    for id in [ids[0], ids[2]] {
        assert_eq!(
            hub.receive(id, &list(5), now()),
            [to(
                id,
                ServerMessage::Channels {
                    reference: Some(5),
                    list: vec![listed("dev", 1), listed("general", 2)],
                    more: false,
                }
            )]
        );
    }
}

#[test]
fn gives_an_empty_list_when_the_server_has_no_public_channel() {
    let (mut hub, ana) = with_owner();
    hub.receive(ana, &create(1, "secret", ChannelKind::Private), now());

    assert_eq!(
        hub.receive(ana, &list(2), now()),
        [to(
            ana,
            ServerMessage::Channels {
                reference: Some(2),
                list: Vec::new(),
                more: false,
            }
        )]
    );
}

#[test]
fn gives_a_long_channel_list_in_parts() {
    let (mut hub, ids) = hub_with(&["ana", "ben", "cy", "dee", "eli"]);
    for (person, id) in ids.iter().enumerate() {
        for number in 0..9 {
            let title = format!("room-{person}-{number}");
            hub.receive(*id, &create(1, &title, ChannelKind::Public), now());
        }
    }

    let outgoing = hub.receive(ids[4], &list(7), now());

    let parts: Vec<(Option<u32>, Vec<String>, bool)> = outgoing
        .iter()
        .map(|part| match &part.message {
            ServerMessage::Channels {
                reference,
                list,
                more,
            } => {
                assert_eq!(part.to, ids[4]);
                let names = list.iter().map(|entry| entry.channel.to_string()).collect();
                (*reference, names, *more)
            }
            other => panic!("the server sends the channel list, not {other:?}"),
        })
        .collect();
    assert_eq!(parts.len(), 2);
    assert_eq!(
        (parts[0].0, parts[0].1.len(), parts[0].2),
        (Some(7), 40, true)
    );
    assert_eq!(
        (parts[1].0, parts[1].1.len(), parts[1].2),
        (Some(7), 5, false)
    );
    let names: Vec<&String> = parts.iter().flat_map(|part| &part.1).collect();
    assert!(names.is_sorted());
    assert_eq!(names[0], "room-0-0");
    assert_eq!(names[44], "room-4-8");
}

#[test]
fn refuses_the_channel_list_before_the_sign_in() {
    let mut hub = new_hub();
    let id = hub.connect();

    assert_eq!(
        hub.receive(id, &list(1), now()),
        [to(id, error(Some(1), ErrorCode::HelloRequired))]
    );
    challenged(&mut hub, id, &[]);
    assert_eq!(
        hub.receive(id, &list(1), now()),
        [to(id, error(Some(1), ErrorCode::IdentifyRequired))]
    );
}
