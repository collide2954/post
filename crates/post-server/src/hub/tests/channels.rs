use post_proto::{ChannelKind, ChannelName, ClientMessage, ErrorCode, LOBBY, Role, ServerMessage};
use post_rules::{AccountId, ChannelChange, ChannelHead, ChannelId, ChannelStorage as _, Kind};
use post_testing::Directory;

use super::{
    challenged, error, hub_with, identify, later, name, new_hub, now, owner_code, random, ready,
    registered, settings, to, welcome, with_owner,
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
