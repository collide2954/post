use post_proto::{ChannelKind, ClientMessage, ErrorCode, ListedChannel, ServerMessage, Timestamp};

use super::channels::{create, join, leave, list, signed_in_again, state, with_general};
use super::{challenged, channel, error, identify, later, name, now, send, to};

fn topic(reference: u32, channel: &str, topic: &str) -> String {
    ClientMessage::ChannelTopic {
        reference: Some(reference),
        channel: channel.to_owned(),
        topic: topic.to_owned(),
    }
    .encode()
}

fn topic_changed(reference: Option<u32>, topic: &str) -> ServerMessage {
    ServerMessage::ChannelTopic {
        reference,
        channel: channel("general"),
        topic: topic.to_owned(),
        by: name("ana"),
        time: later(),
    }
}

#[test]
fn the_owner_changes_the_topic_and_each_member_gets_the_news() {
    let (mut hub, ids) = with_general();
    hub.receive(ids[1], &join(2, "general"), now());
    let second = signed_in_again(&mut hub, 1);

    let outgoing = hub.receive(ids[0], &topic(5, "general", "Plans for the week"), later());

    assert_eq!(
        outgoing,
        [
            to(ids[0], topic_changed(Some(5), "Plans for the week")),
            to(second, topic_changed(None, "Plans for the week")),
            to(ids[1], topic_changed(None, "Plans for the week")),
        ]
    );
}

#[test]
fn gives_the_new_topic_in_the_state_and_in_the_list_of_the_channels() {
    let (mut hub, ids) = with_general();
    hub.receive(ids[0], &topic(2, "general", "Plans for the week"), now());

    let id = hub.connect();
    let server = challenged(&mut hub, id, &[]);
    let line = identify(&hub, 1, &server, None);
    let state = ServerMessage::Channel {
        reference: None,
        channel: channel("general"),
        kind: ChannelKind::Public,
        topic: "Plans for the week".to_owned(),
        owner: name("ana"),
        created: now(),
    };
    assert_eq!(hub.receive(id, &line, later())[1], to(id, state));

    let listed = ListedChannel {
        channel: channel("general"),
        kind: ChannelKind::Public,
        topic: "Plans for the week".to_owned(),
        members: 1,
    };
    assert_eq!(
        hub.receive(ids[2], &list(3), later()),
        [to(
            ids[2],
            ServerMessage::Channels {
                reference: Some(3),
                list: vec![listed],
                more: false,
            }
        )]
    );
}

#[test]
fn the_owner_removes_the_topic_with_an_empty_text() {
    let (mut hub, ids) = with_general();
    hub.receive(ids[0], &topic(1, "general", "Plans"), now());

    assert_eq!(
        hub.receive(ids[0], &topic(2, "general", ""), later()),
        [to(ids[0], topic_changed(Some(2), ""))]
    );
}

#[test]
fn refuses_a_topic_that_breaks_a_rule() {
    let (mut hub, ids) = with_general();
    hub.receive(ids[0], &create(2, "secret", ChannelKind::Private), now());
    hub.receive(ids[1], &join(3, "general"), now());
    let too_long = "x".repeat(301);

    for text in ["two\nlines", "a\ttab", too_long.as_str()] {
        assert_eq!(
            hub.receive(ids[0], &topic(4, "general", text), now()),
            [to(ids[0], error(Some(4), ErrorCode::InvalidTopic))],
            "{text}"
        );
    }
    let longest = "x".repeat(300);
    assert_eq!(
        hub.receive(ids[0], &topic(4, "general", &longest), now())
            .len(),
        2
    );
    for not_the_owner in [ids[1], ids[2]] {
        assert_eq!(
            hub.receive(not_the_owner, &topic(5, "general", "Plans"), now()),
            [to(not_the_owner, error(Some(5), ErrorCode::NotPermitted))]
        );
    }
    for title in ["garden", "secret", "General", ""] {
        assert_eq!(
            hub.receive(ids[1], &topic(6, title, "Plans"), now()),
            [to(ids[1], error(Some(6), ErrorCode::NoSuchChannel))],
            "{title}"
        );
    }
}

#[test]
fn refuses_a_change_of_a_channel_before_the_sign_in() {
    let (mut hub, _ids) = with_general();
    let id = hub.connect();
    let lines = [
        topic(1, "general", "Plans"),
        rename(1, "general", "lounge"),
        delete(1, "general"),
        give(1, "general", "ben"),
    ];

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
fn answers_with_a_server_error_when_it_cannot_store_a_change_of_a_channel() {
    let (mut hub, ids) = with_general();
    hub.receive(ids[1], &join(1, "general"), now());
    hub.store.lose_its_tables();

    assert_eq!(
        hub.receive(ids[0], &topic(2, "general", "Plans"), now()),
        [to(ids[0], error(Some(2), ErrorCode::ServerError))]
    );
    assert_eq!(
        hub.receive(ids[0], &rename(3, "general", "lounge"), now()),
        [to(ids[0], error(Some(3), ErrorCode::ServerError))]
    );
    assert_eq!(
        hub.receive(ids[0], &give(5, "general", "ben"), now()),
        [to(ids[0], error(Some(5), ErrorCode::ServerError))]
    );
    assert_eq!(
        hub.receive(ids[0], &delete(4, "general"), now()),
        [to(ids[0], error(Some(4), ErrorCode::ServerError))]
    );
}

pub(super) fn rename(reference: u32, channel: &str, name: &str) -> String {
    ClientMessage::ChannelRename {
        reference: Some(reference),
        channel: channel.to_owned(),
        name: name.to_owned(),
    }
    .encode()
}

pub(super) fn renamed(reference: Option<u32>, old: &str, new: &str) -> ServerMessage {
    ServerMessage::ChannelRename {
        reference,
        channel: channel(old),
        name: channel(new),
        time: later(),
    }
}

fn next_month() -> Timestamp {
    Timestamp::parse("2026-12-22T14:02:07.120Z").expect("valid")
}

#[test]
fn the_owner_changes_the_name_of_a_channel_and_each_member_gets_the_news() {
    let (mut hub, ids) = with_general();
    hub.receive(ids[1], &join(2, "general"), now());

    let outgoing = hub.receive(ids[0], &rename(5, "general", "lounge"), later());

    assert_eq!(
        outgoing,
        [
            to(ids[0], renamed(Some(5), "general", "lounge")),
            to(ids[1], renamed(None, "general", "lounge")),
        ]
    );
    assert_eq!(
        hub.receive(ids[1], &send(Some(6), "general", "Hello"), later()),
        [to(ids[1], error(Some(6), ErrorCode::NoSuchChannel))]
    );
    let sent = hub.receive(ids[1], &send(Some(7), "lounge", "Hello"), later());
    assert_eq!(sent.len(), 2);
}

#[test]
fn keeps_the_earlier_name_of_a_channel_for_that_channel() {
    let (mut hub, ids) = with_general();
    hub.receive(ids[0], &rename(2, "general", "lounge"), now());

    assert_eq!(
        hub.receive(ids[1], &create(3, "general", ChannelKind::Public), later()),
        [to(ids[1], error(Some(3), ErrorCode::NameTaken))]
    );
    assert_eq!(
        hub.receive(ids[0], &rename(4, "lounge", "garden"), later()),
        [to(ids[0], error(Some(4), ErrorCode::TooSoon))]
    );

    let back = hub.receive(ids[0], &rename(5, "lounge", "general"), next_month());
    assert_eq!(back.len(), 1);
    assert_eq!(
        hub.receive(
            ids[1],
            &create(6, "lounge", ChannelKind::Public),
            next_month()
        ),
        [to(ids[1], error(Some(6), ErrorCode::NameTaken))]
    );
}

#[test]
fn refuses_a_new_name_of_a_channel_that_breaks_a_rule() {
    let (mut hub, ids) = with_general();
    hub.receive(ids[1], &create(2, "dev", ChannelKind::Public), now());
    hub.receive(ids[0], &create(3, "secret", ChannelKind::Private), now());
    hub.receive(ids[1], &join(4, "general"), now());

    for (wanted, code) in [
        ("General", ErrorCode::InvalidChannelName),
        ("g", ErrorCode::InvalidChannelName),
        ("", ErrorCode::InvalidChannelName),
        ("dev", ErrorCode::NameTaken),
        ("general", ErrorCode::NameTaken),
        ("secret", ErrorCode::NameTaken),
    ] {
        assert_eq!(
            hub.receive(ids[0], &rename(5, "general", wanted), now()),
            [to(ids[0], error(Some(5), code))],
            "{wanted}"
        );
    }
    for not_the_owner in [ids[1], ids[2]] {
        assert_eq!(
            hub.receive(not_the_owner, &rename(6, "general", "lounge"), now()),
            [to(not_the_owner, error(Some(6), ErrorCode::NotPermitted))]
        );
    }
    for title in ["garden", "secret", "General"] {
        assert_eq!(
            hub.receive(ids[1], &rename(7, title, "lounge"), now()),
            [to(ids[1], error(Some(7), ErrorCode::NoSuchChannel))],
            "{title}"
        );
    }
}

pub(super) fn delete(reference: u32, channel: &str) -> String {
    ClientMessage::ChannelDelete {
        reference: Some(reference),
        channel: channel.to_owned(),
    }
    .encode()
}

pub(super) fn deleted(reference: Option<u32>, title: &str) -> ServerMessage {
    ServerMessage::ChannelDelete {
        reference,
        channel: channel(title),
        time: later(),
    }
}

#[test]
fn the_owner_deletes_a_channel_and_each_member_gets_the_news() {
    let (mut hub, ids) = with_general();
    hub.receive(ids[1], &join(2, "general"), now());

    let outgoing = hub.receive(ids[0], &delete(5, "general"), later());

    assert_eq!(
        outgoing,
        [
            to(ids[0], deleted(Some(5), "general")),
            to(ids[1], deleted(None, "general")),
        ]
    );
    assert_eq!(
        hub.receive(ids[1], &send(Some(6), "general", "Hello"), later()),
        [to(ids[1], error(Some(6), ErrorCode::NoSuchChannel))]
    );
    let again = hub.receive(ids[1], &create(7, "general", ChannelKind::Public), later());
    assert_eq!(again.len(), 1);
}

#[test]
fn does_not_give_a_deleted_channel_after_the_sign_in_or_in_the_list() {
    let (mut hub, ids) = with_general();
    hub.receive(ids[0], &delete(2, "general"), now());

    let id = hub.connect();
    let server = challenged(&mut hub, id, &[]);
    let line = identify(&hub, 1, &server, None);
    assert_eq!(hub.receive(id, &line, later()).len(), 2);
    assert_eq!(
        hub.receive(ids[2], &list(3), later()),
        [to(
            ids[2],
            ServerMessage::Channels {
                reference: Some(3),
                list: Vec::new(),
                more: false,
            }
        )]
    );
}

#[test]
fn refuses_to_delete_a_channel_for_a_user_who_is_not_its_owner() {
    let (mut hub, ids) = with_general();
    hub.receive(ids[0], &create(2, "secret", ChannelKind::Private), now());
    hub.receive(ids[1], &join(3, "general"), now());

    for not_the_owner in [ids[1], ids[2]] {
        assert_eq!(
            hub.receive(not_the_owner, &delete(4, "general"), now()),
            [to(not_the_owner, error(Some(4), ErrorCode::NotPermitted))]
        );
    }
    for title in ["garden", "secret", "General"] {
        assert_eq!(
            hub.receive(ids[1], &delete(5, title), now()),
            [to(ids[1], error(Some(5), ErrorCode::NoSuchChannel))],
            "{title}"
        );
    }
    let sent = hub.receive(ids[1], &send(Some(6), "general", "Hello"), now());
    assert_eq!(sent.len(), 2);
}

pub(super) fn give(reference: u32, channel: &str, name: &str) -> String {
    ClientMessage::ChannelGive {
        reference: Some(reference),
        channel: channel.to_owned(),
        name: name.to_owned(),
    }
    .encode()
}

pub(super) fn given(reference: Option<u32>, title: &str, to: &str) -> ServerMessage {
    ServerMessage::ChannelGive {
        reference,
        channel: channel(title),
        name: name(to),
        time: later(),
    }
}

#[test]
fn the_owner_gives_a_channel_to_a_member_and_each_member_gets_the_news() {
    let (mut hub, ids) = with_general();
    hub.receive(ids[1], &join(2, "general"), now());

    let outgoing = hub.receive(ids[0], &give(5, "general", "ben"), later());

    assert_eq!(
        outgoing,
        [
            to(ids[0], given(Some(5), "general", "ben")),
            to(ids[1], given(None, "general", "ben")),
        ]
    );
    assert_eq!(
        hub.receive(ids[0], &topic(6, "general", "Plans"), later()),
        [to(ids[0], error(Some(6), ErrorCode::NotPermitted))]
    );
    let changed = hub.receive(ids[1], &topic(7, "general", "Plans"), later());
    assert_eq!(changed.len(), 2);
    assert_eq!(
        hub.receive(ids[1], &leave(8, "general"), later()),
        [to(ids[1], error(Some(8), ErrorCode::OwnerCannotLeave))]
    );
    let left = hub.receive(ids[0], &leave(9, "general"), later());
    assert_eq!(left.len(), 2);
}

#[test]
fn gives_the_name_of_the_new_owner_in_the_state_of_the_channel() {
    let (mut hub, ids) = with_general();
    hub.receive(ids[1], &join(2, "general"), now());
    hub.receive(ids[0], &give(3, "general", "ben"), now());

    let id = hub.connect();
    let server = challenged(&mut hub, id, &[]);
    let line = identify(&hub, 1, &server, None);

    assert_eq!(
        hub.receive(id, &line, later())[1],
        to(id, state(None, "general", ChannelKind::Public, "ben"))
    );
}

#[test]
fn refuses_to_give_a_channel_when_a_rule_does_not_permit_it() {
    let (mut hub, ids) = with_general();
    hub.receive(ids[0], &create(2, "secret", ChannelKind::Private), now());
    hub.receive(ids[1], &join(3, "general"), now());

    for (wanted, code) in [
        ("cy", ErrorCode::NoSuchMember),
        ("dan", ErrorCode::NoSuchUser),
        ("Not A Name", ErrorCode::NoSuchUser),
        ("", ErrorCode::NoSuchUser),
    ] {
        assert_eq!(
            hub.receive(ids[0], &give(4, "general", wanted), now()),
            [to(ids[0], error(Some(4), code))],
            "{wanted}"
        );
    }
    for not_the_owner in [ids[1], ids[2]] {
        for wanted in ["ben", "dan"] {
            assert_eq!(
                hub.receive(not_the_owner, &give(5, "general", wanted), now()),
                [to(not_the_owner, error(Some(5), ErrorCode::NotPermitted))]
            );
        }
    }
    for title in ["garden", "secret", "General"] {
        assert_eq!(
            hub.receive(ids[1], &give(6, title, "ben"), now()),
            [to(ids[1], error(Some(6), ErrorCode::NoSuchChannel))],
            "{title}"
        );
    }
}

#[test]
fn does_not_give_a_channel_to_a_member_who_owns_ten_channels() {
    let (mut hub, ids) = with_general();
    hub.receive(ids[1], &join(2, "general"), now());
    for number in 0..10 {
        let title = format!("channel-{number}");
        hub.receive(ids[1], &create(number, &title, ChannelKind::Public), now());
    }

    assert_eq!(
        hub.receive(ids[0], &give(3, "general", "ben"), now()),
        [to(ids[0], error(Some(3), ErrorCode::TooManyChannels))]
    );
}
