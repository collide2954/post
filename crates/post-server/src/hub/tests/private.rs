use post_proto::{ChannelKind, ClientMessage, ErrorCode, ListedChannel, Role, ServerMessage};

use super::channels::{create, join, joined, leave, list, signed_in_again, state, with_general};
use super::owner::{delete, deleted, give, given, rename as rename_channel, renamed};
use super::{challenged, channel, error, identify, later, name, now, ready, to, welcome};
use crate::hub::{ConnectionId, Hub};

fn invite(reference: u32, channel: &str, name: &str) -> String {
    ClientMessage::ChannelInvite {
        reference: Some(reference),
        channel: channel.to_owned(),
        name: name.to_owned(),
    }
    .encode()
}

fn invited(reference: Option<u32>, who: &str) -> ServerMessage {
    ServerMessage::ChannelInvite {
        reference,
        channel: channel("secret"),
        name: name(who),
        owner: name("ana"),
        time: later(),
    }
}

fn with_secret() -> (Hub, Vec<ConnectionId>) {
    let (mut hub, ids) = with_general();
    hub.receive(ids[0], &create(2, "secret", ChannelKind::Private), now());
    (hub, ids)
}

fn listed(title: &str, kind: ChannelKind) -> ListedChannel {
    ListedChannel {
        channel: channel(title),
        kind,
        topic: String::new(),
        members: 1,
    }
}

#[test]
fn the_owner_invites_a_user_to_a_private_channel_and_the_two_users_get_the_news() {
    let (mut hub, ids) = with_secret();
    let second = signed_in_again(&mut hub, 2);

    let outgoing = hub.receive(ids[0], &invite(5, "secret", "ben"), later());

    assert_eq!(
        outgoing,
        [
            to(ids[0], invited(Some(5), "ben")),
            to(ids[1], invited(None, "ben")),
            to(second, invited(None, "ben")),
        ]
    );
}

#[test]
fn an_invited_user_sees_the_private_channel_in_the_list() {
    let (mut hub, ids) = with_secret();
    hub.receive(ids[0], &invite(3, "secret", "ben"), now());

    assert_eq!(
        hub.receive(ids[1], &list(4), now()),
        [to(
            ids[1],
            ServerMessage::Channels {
                reference: Some(4),
                list: vec![
                    listed("general", ChannelKind::Public),
                    listed("secret", ChannelKind::Private),
                ],
                more: false,
            }
        )]
    );
    assert_eq!(
        hub.receive(ids[2], &list(5), now()),
        [to(
            ids[2],
            ServerMessage::Channels {
                reference: Some(5),
                list: vec![listed("general", ChannelKind::Public)],
                more: false,
            }
        )]
    );
}

#[test]
fn an_invited_user_joins_the_private_channel_one_time() {
    let (mut hub, ids) = with_secret();
    hub.receive(ids[0], &invite(3, "secret", "ben"), now());

    assert_eq!(
        hub.receive(ids[2], &join(4, "secret"), now()),
        [to(ids[2], error(Some(4), ErrorCode::NoSuchChannel))]
    );
    assert_eq!(
        hub.receive(ids[1], &join(5, "secret"), now()),
        [
            to(
                ids[1],
                state(Some(5), "secret", ChannelKind::Private, "ana")
            ),
            to(ids[0], joined("secret", "ben")),
            to(ids[1], joined("secret", "ben")),
        ]
    );

    hub.receive(ids[1], &leave(6, "secret"), now());
    assert_eq!(
        hub.receive(ids[1], &join(7, "secret"), now()),
        [to(ids[1], error(Some(7), ErrorCode::NoSuchChannel))]
    );
}

#[test]
fn gives_the_open_invitations_after_the_sign_in() {
    let (mut hub, ids) = with_secret();
    hub.receive(ids[0], &invite(3, "secret", "ben"), later());
    hub.receive(ids[0], &invite(4, "secret", "cy"), later());

    let id = hub.connect();
    let server = challenged(&mut hub, id, &[]);
    let line = identify(&hub, 2, &server, None);
    assert_eq!(
        hub.receive(id, &line, now()),
        [
            to(id, welcome("ben", Role::Member)),
            to(id, invited(None, "ben")),
            to(id, ready()),
        ]
    );

    let id = hub.connect();
    let server = challenged(&mut hub, id, &[]);
    let line = identify(&hub, 1, &server, None);
    assert_eq!(
        hub.receive(id, &line, now()),
        [
            to(id, welcome("ana", Role::Owner)),
            to(id, state(None, "general", ChannelKind::Public, "ana")),
            to(id, state(None, "secret", ChannelKind::Private, "ana")),
            to(id, invited(None, "ben")),
            to(id, invited(None, "cy")),
            to(id, ready()),
        ]
    );
}

#[test]
fn refuses_an_invitation_to_a_channel_that_breaks_a_rule() {
    let (mut hub, ids) = with_secret();
    hub.receive(ids[1], &join(3, "general"), now());
    hub.receive(ids[0], &invite(4, "secret", "ben"), now());

    for (title, wanted, code) in [
        ("secret", "ben", ErrorCode::AlreadyInvited),
        ("secret", "ana", ErrorCode::AlreadyMember),
        ("secret", "dan", ErrorCode::NoSuchUser),
        ("secret", "Not A Name", ErrorCode::NoSuchUser),
        ("general", "cy", ErrorCode::NotPrivate),
        ("garden", "cy", ErrorCode::NoSuchChannel),
    ] {
        assert_eq!(
            hub.receive(ids[0], &invite(5, title, wanted), now()),
            [to(ids[0], error(Some(5), code))],
            "{title} {wanted}"
        );
    }
    for title in ["general", "secret"] {
        assert_eq!(
            hub.receive(ids[1], &invite(6, title, "cy"), now()),
            [to(ids[1], error(Some(6), ErrorCode::NotPermitted))],
            "{title}"
        );
    }
    assert_eq!(
        hub.receive(ids[2], &invite(7, "secret", "ben"), now()),
        [to(ids[2], error(Some(7), ErrorCode::NoSuchChannel))]
    );
}

#[test]
fn refuses_a_change_of_an_invitation_to_a_channel_before_the_sign_in() {
    let (mut hub, _ids) = with_secret();
    let id = hub.connect();
    let lines = [
        invite(1, "secret", "ben"),
        cancel(1, "secret", "ben"),
        refuse(1, "secret"),
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
fn answers_with_a_server_error_when_it_cannot_store_an_invitation_to_a_channel() {
    let (mut hub, ids) = with_secret();
    hub.receive(ids[0], &invite(2, "secret", "ben"), now());
    hub.store.lose_its_tables();

    assert_eq!(
        hub.receive(ids[0], &invite(3, "secret", "cy"), now()),
        [to(ids[0], error(Some(3), ErrorCode::ServerError))]
    );
    assert_eq!(
        hub.receive(ids[0], &cancel(4, "secret", "ben"), now()),
        [to(ids[0], error(Some(4), ErrorCode::ServerError))]
    );
    assert_eq!(
        hub.receive(ids[1], &refuse(5, "secret"), now()),
        [to(ids[1], error(Some(5), ErrorCode::ServerError))]
    );
}

fn cancel(reference: u32, channel: &str, name: &str) -> String {
    ClientMessage::ChannelCancel {
        reference: Some(reference),
        channel: channel.to_owned(),
        name: name.to_owned(),
    }
    .encode()
}

fn refuse(reference: u32, channel: &str) -> String {
    ClientMessage::ChannelRefuse {
        reference: Some(reference),
        channel: channel.to_owned(),
    }
    .encode()
}

fn canceled(reference: Option<u32>, who: &str) -> ServerMessage {
    ServerMessage::ChannelCancel {
        reference,
        channel: channel("secret"),
        name: name(who),
        time: later(),
    }
}

fn refused(reference: Option<u32>, who: &str) -> ServerMessage {
    ServerMessage::ChannelRefuse {
        reference,
        channel: channel("secret"),
        name: name(who),
        time: later(),
    }
}

#[test]
fn the_owner_cancels_an_invitation_and_the_two_users_get_the_news() {
    let (mut hub, ids) = with_secret();
    hub.receive(ids[0], &invite(3, "secret", "ben"), now());

    let outgoing = hub.receive(ids[0], &cancel(5, "secret", "ben"), later());

    assert_eq!(
        outgoing,
        [
            to(ids[0], canceled(Some(5), "ben")),
            to(ids[1], canceled(None, "ben")),
        ]
    );
    assert_eq!(
        hub.receive(ids[1], &join(6, "secret"), later()),
        [to(ids[1], error(Some(6), ErrorCode::NoSuchChannel))]
    );
}

#[test]
fn an_invited_user_refuses_the_invitation_and_the_two_users_get_the_news() {
    let (mut hub, ids) = with_secret();
    hub.receive(ids[0], &invite(3, "secret", "ben"), now());

    let outgoing = hub.receive(ids[1], &refuse(5, "secret"), later());

    assert_eq!(
        outgoing,
        [
            to(ids[0], refused(None, "ben")),
            to(ids[1], refused(Some(5), "ben")),
        ]
    );
    assert_eq!(
        hub.receive(ids[1], &join(6, "secret"), later()),
        [to(ids[1], error(Some(6), ErrorCode::NoSuchChannel))]
    );
    let again = hub.receive(ids[0], &invite(7, "secret", "ben"), later());
    assert_eq!(again.len(), 2);
}

#[test]
fn does_not_give_an_invitation_that_ended_after_the_sign_in() {
    let (mut hub, ids) = with_secret();
    hub.receive(ids[0], &invite(3, "secret", "ben"), now());
    hub.receive(ids[0], &invite(4, "secret", "cy"), now());
    hub.receive(ids[0], &cancel(5, "secret", "ben"), now());
    hub.receive(ids[2], &refuse(6, "secret"), now());

    let id = hub.connect();
    let server = challenged(&mut hub, id, &[]);
    let line = identify(&hub, 1, &server, None);

    assert_eq!(
        hub.receive(id, &line, now()),
        [
            to(id, welcome("ana", Role::Owner)),
            to(id, state(None, "general", ChannelKind::Public, "ana")),
            to(id, state(None, "secret", ChannelKind::Private, "ana")),
            to(id, ready()),
        ]
    );
}

#[test]
fn refuses_to_end_an_invitation_when_a_rule_does_not_permit_it() {
    let (mut hub, ids) = with_secret();
    hub.receive(ids[1], &join(3, "general"), now());
    hub.receive(ids[0], &invite(4, "secret", "ben"), now());

    for (title, wanted, code) in [
        ("secret", "cy", ErrorCode::NotInvited),
        ("secret", "ana", ErrorCode::NotInvited),
        ("secret", "dan", ErrorCode::NoSuchUser),
        ("secret", "Not A Name", ErrorCode::NoSuchUser),
        ("general", "ben", ErrorCode::NotInvited),
        ("garden", "ben", ErrorCode::NoSuchChannel),
    ] {
        assert_eq!(
            hub.receive(ids[0], &cancel(5, title, wanted), now()),
            [to(ids[0], error(Some(5), code))],
            "{title} {wanted}"
        );
    }
    assert_eq!(
        hub.receive(ids[1], &cancel(6, "secret", "ben"), now()),
        [to(ids[1], error(Some(6), ErrorCode::NotPermitted))]
    );
    assert_eq!(
        hub.receive(ids[2], &cancel(7, "secret", "ben"), now()),
        [to(ids[2], error(Some(7), ErrorCode::NoSuchChannel))]
    );

    for (who, title, code) in [
        (ids[0], "secret", ErrorCode::NotInvited),
        (ids[1], "general", ErrorCode::NotInvited),
        (ids[2], "general", ErrorCode::NotInvited),
        (ids[2], "secret", ErrorCode::NoSuchChannel),
        (ids[1], "garden", ErrorCode::NoSuchChannel),
    ] {
        assert_eq!(
            hub.receive(who, &refuse(8, title), now()),
            [to(who, error(Some(8), code))],
            "{title}"
        );
    }
}

#[test]
fn tells_an_invited_user_about_the_new_name_of_the_channel() {
    let (mut hub, ids) = with_secret();
    hub.receive(ids[0], &invite(3, "secret", "ben"), now());

    let outgoing = hub.receive(ids[0], &rename_channel(4, "secret", "hidden"), later());

    assert_eq!(
        outgoing,
        [
            to(ids[0], renamed(Some(4), "secret", "hidden")),
            to(ids[1], renamed(None, "secret", "hidden")),
        ]
    );
}

#[test]
fn tells_an_invited_user_that_the_owner_deleted_the_channel() {
    let (mut hub, ids) = with_secret();
    hub.receive(ids[0], &invite(3, "secret", "ben"), now());

    let outgoing = hub.receive(ids[0], &delete(4, "secret"), later());

    assert_eq!(
        outgoing,
        [
            to(ids[0], deleted(Some(4), "secret")),
            to(ids[1], deleted(None, "secret")),
        ]
    );
}

#[test]
fn gives_the_open_invitations_to_the_new_owner_of_a_channel() {
    let (mut hub, ids) = with_secret();
    hub.receive(ids[0], &invite(3, "secret", "ben"), now());
    hub.receive(ids[1], &join(4, "secret"), now());
    hub.receive(ids[0], &invite(5, "secret", "cy"), later());

    let outgoing = hub.receive(ids[0], &give(6, "secret", "ben"), later());

    let invitation = ServerMessage::ChannelInvite {
        reference: None,
        channel: channel("secret"),
        name: name("cy"),
        owner: name("ben"),
        time: later(),
    };
    assert_eq!(
        outgoing,
        [
            to(ids[0], given(Some(6), "secret", "ben")),
            to(ids[1], given(None, "secret", "ben")),
            to(ids[1], invitation.clone()),
            to(ids[2], invitation),
        ]
    );
}

#[test]
fn tells_the_owner_and_the_invited_user_about_a_new_name_of_the_other_user() {
    let (mut hub, ids) = with_secret();
    hub.receive(ids[0], &invite(3, "secret", "ben"), now());
    let receivers = |outgoing: Vec<super::Outgoing>| -> Vec<ConnectionId> {
        outgoing.into_iter().map(|outgoing| outgoing.to).collect()
    };

    let outgoing = hub.receive(ids[1], &super::rename(4, "benito"), later());
    assert_eq!(receivers(outgoing), [ids[0], ids[1]]);

    let outgoing = hub.receive(ids[0], &super::rename(5, "anna"), later());
    assert_eq!(receivers(outgoing), [ids[0], ids[1]]);

    let outgoing = hub.receive(ids[2], &super::rename(6, "cyrus"), later());
    assert_eq!(receivers(outgoing), [ids[2]]);
}
