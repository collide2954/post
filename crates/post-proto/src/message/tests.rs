use std::num::NonZeroU32;

use super::{
    ChannelKind, ClientMessage, ErrorCode, Invitation, ListedChannel, PROTOCOL_VERSION, Role,
    ServerMessage,
};
use crate::{
    ChannelName, Code, DecodeError, Key, MAX_LINE_BYTES, Name, Nonce, Part, Signature, Timestamp,
};

fn name(text: &str) -> Name {
    Name::parse(text).expect(text)
}

#[test]
fn writes_a_role_as_a_lower_case_word() {
    for (role, json) in [(Role::Owner, "\"owner\""), (Role::Member, "\"member\"")] {
        assert_eq!(serde_json::to_string(&role).expect("valid"), json);
        assert_eq!(serde_json::from_str::<Role>(json).expect("valid"), role);
    }
    assert!(serde_json::from_str::<Role>("\"Owner\"").is_err());
}

#[test]
fn writes_the_kind_of_a_channel_as_a_lower_case_word() {
    for (kind, json) in [
        (ChannelKind::Public, "\"public\""),
        (ChannelKind::Private, "\"private\""),
    ] {
        assert_eq!(serde_json::to_string(&kind).expect("valid"), json);
        assert_eq!(
            serde_json::from_str::<ChannelKind>(json).expect("valid"),
            kind
        );
    }
    assert!(serde_json::from_str::<ChannelKind>("\"Public\"").is_err());
}

fn time(text: &str) -> Timestamp {
    Timestamp::parse(text).expect(text)
}

fn channel(text: &str) -> ChannelName {
    ChannelName::parse(text).expect(text)
}

fn client_examples() -> Vec<(ClientMessage, &'static str)> {
    let mut examples = channel_requests();
    examples.extend(owner_requests());
    examples.extend(invitation_requests());
    examples.extend(other_requests());
    examples
}

fn invitation_requests() -> Vec<(ClientMessage, &'static str)> {
    vec![
        (
            ClientMessage::ChannelInvite {
                reference: Some(13),
                channel: "secret".to_owned(),
                name: "ben".to_owned(),
            },
            r#"{"type":"channel-invite","ref":13,"channel":"secret","name":"ben"}"#,
        ),
        (
            ClientMessage::ChannelInvite {
                reference: None,
                channel: "secret".to_owned(),
                name: "Not A Name".to_owned(),
            },
            r#"{"type":"channel-invite","channel":"secret","name":"Not A Name"}"#,
        ),
        (
            ClientMessage::ChannelCancel {
                reference: Some(14),
                channel: "secret".to_owned(),
                name: "ben".to_owned(),
            },
            r#"{"type":"channel-cancel","ref":14,"channel":"secret","name":"ben"}"#,
        ),
        (
            ClientMessage::ChannelCancel {
                reference: None,
                channel: "secret".to_owned(),
                name: "ben".to_owned(),
            },
            r#"{"type":"channel-cancel","channel":"secret","name":"ben"}"#,
        ),
        (
            ClientMessage::ChannelRefuse {
                reference: Some(15),
                channel: "secret".to_owned(),
            },
            r#"{"type":"channel-refuse","ref":15,"channel":"secret"}"#,
        ),
        (
            ClientMessage::ChannelRefuse {
                reference: None,
                channel: "secret".to_owned(),
            },
            r#"{"type":"channel-refuse","channel":"secret"}"#,
        ),
    ]
}

fn owner_requests() -> Vec<(ClientMessage, &'static str)> {
    vec![
        (
            ClientMessage::ChannelTopic {
                reference: Some(9),
                channel: "general".to_owned(),
                topic: "Plans for the week".to_owned(),
            },
            r#"{"type":"channel-topic","ref":9,"channel":"general","topic":"Plans for the week"}"#,
        ),
        (
            ClientMessage::ChannelTopic {
                reference: None,
                channel: "general".to_owned(),
                topic: String::new(),
            },
            r#"{"type":"channel-topic","channel":"general","topic":""}"#,
        ),
        (
            ClientMessage::ChannelRename {
                reference: Some(10),
                channel: "general".to_owned(),
                name: "lounge".to_owned(),
            },
            r#"{"type":"channel-rename","ref":10,"channel":"general","name":"lounge"}"#,
        ),
        (
            ClientMessage::ChannelRename {
                reference: None,
                channel: "general".to_owned(),
                name: "Not A Name".to_owned(),
            },
            r#"{"type":"channel-rename","channel":"general","name":"Not A Name"}"#,
        ),
        (
            ClientMessage::ChannelDelete {
                reference: Some(11),
                channel: "general".to_owned(),
            },
            r#"{"type":"channel-delete","ref":11,"channel":"general"}"#,
        ),
        (
            ClientMessage::ChannelDelete {
                reference: None,
                channel: "general".to_owned(),
            },
            r#"{"type":"channel-delete","channel":"general"}"#,
        ),
        (
            ClientMessage::ChannelGive {
                reference: Some(12),
                channel: "general".to_owned(),
                name: "ben".to_owned(),
            },
            r#"{"type":"channel-give","ref":12,"channel":"general","name":"ben"}"#,
        ),
        (
            ClientMessage::ChannelGive {
                reference: None,
                channel: "general".to_owned(),
                name: "Not A Name".to_owned(),
            },
            r#"{"type":"channel-give","channel":"general","name":"Not A Name"}"#,
        ),
    ]
}

fn channel_requests() -> Vec<(ClientMessage, &'static str)> {
    vec![
        (
            ClientMessage::Channels { reference: Some(8) },
            r#"{"type":"channels","ref":8}"#,
        ),
        (
            ClientMessage::Channels { reference: None },
            r#"{"type":"channels"}"#,
        ),
        (
            ClientMessage::Join {
                reference: Some(6),
                channel: "general".to_owned(),
            },
            r#"{"type":"join","ref":6,"channel":"general"}"#,
        ),
        (
            ClientMessage::Join {
                reference: None,
                channel: "#general".to_owned(),
            },
            r##"{"type":"join","channel":"#general"}"##,
        ),
        (
            ClientMessage::Leave {
                reference: Some(7),
                channel: "general".to_owned(),
            },
            r#"{"type":"leave","ref":7,"channel":"general"}"#,
        ),
        (
            ClientMessage::Leave {
                reference: None,
                channel: "general".to_owned(),
            },
            r#"{"type":"leave","channel":"general"}"#,
        ),
        (
            ClientMessage::ChannelCreate {
                reference: Some(5),
                channel: "general".to_owned(),
                kind: ChannelKind::Public,
            },
            r#"{"type":"channel-create","ref":5,"channel":"general","kind":"public"}"#,
        ),
        (
            ClientMessage::ChannelCreate {
                reference: None,
                channel: "Not A Name".to_owned(),
                kind: ChannelKind::Private,
            },
            r#"{"type":"channel-create","channel":"Not A Name","kind":"private"}"#,
        ),
    ]
}

fn other_requests() -> Vec<(ClientMessage, &'static str)> {
    vec![
        (
            ClientMessage::Send {
                reference: Some(1),
                channel: "general".to_owned(),
                text: "Hello".to_owned(),
            },
            r#"{"type":"send","ref":1,"channel":"general","text":"Hello"}"#,
        ),
        (
            ClientMessage::Send {
                reference: None,
                channel: "general".to_owned(),
                text: "two\nlines".to_owned(),
            },
            r#"{"type":"send","channel":"general","text":"two\nlines"}"#,
        ),
        (
            ClientMessage::Invite {
                reference: Some(2),
                lifetime: NonZeroU32::new(604_800),
                uses: NonZeroU32::new(1),
            },
            r#"{"type":"invite","ref":2,"lifetime":604800,"uses":1}"#,
        ),
        (
            ClientMessage::Invite {
                reference: None,
                lifetime: None,
                uses: None,
            },
            r#"{"type":"invite"}"#,
        ),
        (
            ClientMessage::Invitations { reference: Some(3) },
            r#"{"type":"invitations","ref":3}"#,
        ),
        (
            ClientMessage::Revoke {
                reference: None,
                code: Code::parse("7Hq2kdX9mPq4").expect("valid"),
            },
            r#"{"type":"revoke","code":"7Hq2kdX9mPq4"}"#,
        ),
        (
            ClientMessage::Rename {
                reference: Some(4),
                name: "anna".to_owned(),
            },
            r#"{"type":"rename","ref":4,"name":"anna"}"#,
        ),
        (
            ClientMessage::Rename {
                reference: None,
                name: "Not A Name".to_owned(),
            },
            r#"{"type":"rename","name":"Not A Name"}"#,
        ),
    ]
}

fn server_examples() -> Vec<(ServerMessage, &'static str)> {
    vec![
        (
            ServerMessage::Channel {
                reference: Some(5),
                channel: channel("general"),
                kind: ChannelKind::Public,
                topic: String::new(),
                owner: name("ana"),
                created: time("2026-11-21T14:02:05.000Z"),
            },
            r#"{"type":"channel","ref":5,"channel":"general","kind":"public","topic":"","owner":"ana","created":"2026-11-21T14:02:05.000Z"}"#,
        ),
        (
            ServerMessage::Channel {
                reference: None,
                channel: channel("dev-2"),
                kind: ChannelKind::Private,
                topic: "Plans for the week".to_owned(),
                owner: name("ben"),
                created: time("2026-11-21T14:02:05.000Z"),
            },
            r#"{"type":"channel","channel":"dev-2","kind":"private","topic":"Plans for the week","owner":"ben","created":"2026-11-21T14:02:05.000Z"}"#,
        ),
        (ServerMessage::Ready {}, r#"{"type":"ready"}"#),
        (
            ServerMessage::Welcome {
                reference: None,
                name: name("ana"),
                role: Role::Member,
            },
            r#"{"type":"welcome","name":"ana","role":"member"}"#,
        ),
        (
            ServerMessage::Welcome {
                reference: Some(4_294_967_295),
                name: name("ana"),
                role: Role::Member,
            },
            r#"{"type":"welcome","ref":4294967295,"name":"ana","role":"member"}"#,
        ),
        (
            ServerMessage::Join {
                channel: channel("general"),
                name: name("ben"),
                time: time("2026-11-21T14:02:05.000Z"),
            },
            r#"{"type":"join","channel":"general","name":"ben","time":"2026-11-21T14:02:05.000Z"}"#,
        ),
        (
            ServerMessage::Leave {
                reference: None,
                channel: channel("general"),
                name: name("ben"),
                time: time("2026-11-21T14:03:00.000Z"),
            },
            r#"{"type":"leave","channel":"general","name":"ben","time":"2026-11-21T14:03:00.000Z"}"#,
        ),
        (
            ServerMessage::Leave {
                reference: Some(7),
                channel: channel("general"),
                name: name("ana"),
                time: time("2026-11-21T14:03:00.000Z"),
            },
            r#"{"type":"leave","ref":7,"channel":"general","name":"ana","time":"2026-11-21T14:03:00.000Z"}"#,
        ),
        (
            ServerMessage::Message {
                reference: Some(1),
                channel: channel("general"),
                from: name("ana"),
                time: time("2026-11-21T14:02:07.120Z"),
                text: "Hello".to_owned(),
            },
            r#"{"type":"message","ref":1,"channel":"general","from":"ana","time":"2026-11-21T14:02:07.120Z","text":"Hello"}"#,
        ),
        (
            ServerMessage::Message {
                reference: None,
                channel: channel("general"),
                from: name("ben"),
                time: time("2026-11-21T14:02:09.480Z"),
                text: "Hi".to_owned(),
            },
            r#"{"type":"message","channel":"general","from":"ben","time":"2026-11-21T14:02:09.480Z","text":"Hi"}"#,
        ),
        (
            ServerMessage::Error {
                reference: Some(2),
                code: ErrorCode::NoSuchChannel,
                text: "The channel does not exist.".to_owned(),
            },
            r#"{"type":"error","ref":2,"code":"no-such-channel","text":"The channel does not exist."}"#,
        ),
    ]
}

fn all_server_examples() -> Vec<(ServerMessage, &'static str)> {
    let mut examples = server_examples();
    examples.extend(server_list_examples());
    examples.extend(owner_events());
    examples.extend(invitation_events());
    examples
}

fn invitation_events() -> Vec<(ServerMessage, &'static str)> {
    vec![
        (
            ServerMessage::ChannelInvite {
                reference: Some(13),
                channel: channel("secret"),
                name: name("ben"),
                owner: name("ana"),
                time: time("2026-11-21T14:12:00.000Z"),
            },
            r#"{"type":"channel-invite","ref":13,"channel":"secret","name":"ben","owner":"ana","time":"2026-11-21T14:12:00.000Z"}"#,
        ),
        (
            ServerMessage::ChannelInvite {
                reference: None,
                channel: channel("secret"),
                name: name("ben"),
                owner: name("ana"),
                time: time("2026-11-21T14:12:00.000Z"),
            },
            r#"{"type":"channel-invite","channel":"secret","name":"ben","owner":"ana","time":"2026-11-21T14:12:00.000Z"}"#,
        ),
        (
            ServerMessage::ChannelCancel {
                reference: Some(14),
                channel: channel("secret"),
                name: name("ben"),
                time: time("2026-11-21T14:14:00.000Z"),
            },
            r#"{"type":"channel-cancel","ref":14,"channel":"secret","name":"ben","time":"2026-11-21T14:14:00.000Z"}"#,
        ),
        (
            ServerMessage::ChannelCancel {
                reference: None,
                channel: channel("secret"),
                name: name("ben"),
                time: time("2026-11-21T14:14:00.000Z"),
            },
            r#"{"type":"channel-cancel","channel":"secret","name":"ben","time":"2026-11-21T14:14:00.000Z"}"#,
        ),
        (
            ServerMessage::ChannelRefuse {
                reference: Some(15),
                channel: channel("secret"),
                name: name("ben"),
                time: time("2026-11-21T14:15:00.000Z"),
            },
            r#"{"type":"channel-refuse","ref":15,"channel":"secret","name":"ben","time":"2026-11-21T14:15:00.000Z"}"#,
        ),
        (
            ServerMessage::ChannelRefuse {
                reference: None,
                channel: channel("secret"),
                name: name("ben"),
                time: time("2026-11-21T14:15:00.000Z"),
            },
            r#"{"type":"channel-refuse","channel":"secret","name":"ben","time":"2026-11-21T14:15:00.000Z"}"#,
        ),
    ]
}

fn owner_events() -> Vec<(ServerMessage, &'static str)> {
    vec![
        (
            ServerMessage::ChannelTopic {
                reference: Some(9),
                channel: channel("general"),
                topic: "Plans for the week".to_owned(),
                by: name("ana"),
                time: time("2026-11-21T14:06:00.000Z"),
            },
            r#"{"type":"channel-topic","ref":9,"channel":"general","topic":"Plans for the week","by":"ana","time":"2026-11-21T14:06:00.000Z"}"#,
        ),
        (
            ServerMessage::ChannelTopic {
                reference: None,
                channel: channel("general"),
                topic: String::new(),
                by: name("ana"),
                time: time("2026-11-21T14:06:00.000Z"),
            },
            r#"{"type":"channel-topic","channel":"general","topic":"","by":"ana","time":"2026-11-21T14:06:00.000Z"}"#,
        ),
        (
            ServerMessage::ChannelRename {
                reference: Some(10),
                channel: channel("general"),
                name: channel("lounge"),
                time: time("2026-11-21T14:08:00.000Z"),
            },
            r#"{"type":"channel-rename","ref":10,"channel":"general","name":"lounge","time":"2026-11-21T14:08:00.000Z"}"#,
        ),
        (
            ServerMessage::ChannelRename {
                reference: None,
                channel: channel("general"),
                name: channel("lounge"),
                time: time("2026-11-21T14:08:00.000Z"),
            },
            r#"{"type":"channel-rename","channel":"general","name":"lounge","time":"2026-11-21T14:08:00.000Z"}"#,
        ),
        (
            ServerMessage::ChannelDelete {
                reference: Some(11),
                channel: channel("general"),
                time: time("2026-11-21T14:09:00.000Z"),
            },
            r#"{"type":"channel-delete","ref":11,"channel":"general","time":"2026-11-21T14:09:00.000Z"}"#,
        ),
        (
            ServerMessage::ChannelDelete {
                reference: None,
                channel: channel("general"),
                time: time("2026-11-21T14:09:00.000Z"),
            },
            r#"{"type":"channel-delete","channel":"general","time":"2026-11-21T14:09:00.000Z"}"#,
        ),
        (
            ServerMessage::ChannelGive {
                reference: Some(12),
                channel: channel("general"),
                name: name("ben"),
                time: time("2026-11-21T14:10:00.000Z"),
            },
            r#"{"type":"channel-give","ref":12,"channel":"general","name":"ben","time":"2026-11-21T14:10:00.000Z"}"#,
        ),
        (
            ServerMessage::ChannelGive {
                reference: None,
                channel: channel("general"),
                name: name("ben"),
                time: time("2026-11-21T14:10:00.000Z"),
            },
            r#"{"type":"channel-give","channel":"general","name":"ben","time":"2026-11-21T14:10:00.000Z"}"#,
        ),
    ]
}

fn server_list_examples() -> Vec<(ServerMessage, &'static str)> {
    vec![
        (
            ServerMessage::Channels {
                reference: Some(8),
                list: vec![
                    ListedChannel {
                        channel: channel("dev-2"),
                        kind: ChannelKind::Public,
                        topic: "Plans for the week".to_owned(),
                        members: 12,
                    },
                    ListedChannel {
                        channel: channel("general"),
                        kind: ChannelKind::Private,
                        topic: String::new(),
                        members: 1,
                    },
                ],
                more: true,
            },
            r#"{"type":"channels","ref":8,"list":[{"channel":"dev-2","kind":"public","topic":"Plans for the week","members":12},{"channel":"general","kind":"private","topic":"","members":1}],"more":true}"#,
        ),
        (
            ServerMessage::Channels {
                reference: None,
                list: Vec::new(),
                more: false,
            },
            r#"{"type":"channels","list":[]}"#,
        ),
        (
            ServerMessage::Invitations {
                reference: Some(3),
                list: vec![
                    Invitation {
                        code: Code::parse("7Hq2kdX9mPq4").expect("valid"),
                        link: "https://chat.example.org/i/7Hq2kdX9mPq4".to_owned(),
                        created: time("2026-11-21T14:02:05.000Z"),
                        end: Some(time("2026-11-28T14:02:05.000Z")),
                        uses: Some(5),
                        used: 2,
                    },
                    Invitation {
                        code: Code::parse("AAAAAAAAAAAA").expect("valid"),
                        link: "https://chat.example.org/i/AAAAAAAAAAAA".to_owned(),
                        created: time("2026-11-20T09:00:00.000Z"),
                        end: None,
                        uses: None,
                        used: 0,
                    },
                ],
            },
            r#"{"type":"invitations","ref":3,"list":[{"code":"7Hq2kdX9mPq4","link":"https://chat.example.org/i/7Hq2kdX9mPq4","created":"2026-11-21T14:02:05.000Z","end":"2026-11-28T14:02:05.000Z","uses":5,"used":2},{"code":"AAAAAAAAAAAA","link":"https://chat.example.org/i/AAAAAAAAAAAA","created":"2026-11-20T09:00:00.000Z","used":0}]}"#,
        ),
        (
            ServerMessage::Invitations {
                reference: None,
                list: Vec::new(),
            },
            r#"{"type":"invitations","list":[]}"#,
        ),
        (
            ServerMessage::Rename {
                reference: Some(4),
                old: name("ana"),
                new: name("anna"),
                time: time("2026-11-21T14:04:00.000Z"),
            },
            r#"{"type":"rename","ref":4,"old":"ana","new":"anna","time":"2026-11-21T14:04:00.000Z"}"#,
        ),
        (
            ServerMessage::Rename {
                reference: None,
                old: name("ana"),
                new: name("anna"),
                time: time("2026-11-21T14:04:00.000Z"),
            },
            r#"{"type":"rename","old":"ana","new":"anna","time":"2026-11-21T14:04:00.000Z"}"#,
        ),
    ]
}

#[test]
fn writes_and_reads_each_sign_in_message() {
    let key = "A".repeat(43);
    let nonce = format!("{}-_s", "-_v7".repeat(10));
    let proof = "A".repeat(86);
    let code = Code::parse("7Hq2kdX9mPq4").expect("valid");
    let part = |word: &str| Part::parse(word).expect(word);

    let client = [
        (
            ClientMessage::Hello {
                reference: Some(1),
                protocol: 0,
                address: "chat.example.org:443".to_owned(),
                nonce: Nonce::from_bytes([0xfb; 32]),
                supports: Vec::new(),
            },
            format!(
                r#"{{"type":"hello","ref":1,"protocol":0,"address":"chat.example.org:443","nonce":"{nonce}","supports":[]}}"#
            ),
        ),
        (
            ClientMessage::Hello {
                reference: None,
                protocol: 0,
                address: "localhost:7678".to_owned(),
                nonce: Nonce::from_bytes([0xfb; 32]),
                supports: vec![part("sealed"), part("voice")],
            },
            format!(
                r#"{{"type":"hello","protocol":0,"address":"localhost:7678","nonce":"{nonce}","supports":["sealed","voice"]}}"#
            ),
        ),
        (
            ClientMessage::Identify {
                reference: Some(2),
                key: Key::from_bytes([0; 32]),
                proof: Signature::from_bytes([0; 64]),
                invitation: None,
            },
            format!(r#"{{"type":"identify","ref":2,"key":"{key}","proof":"{proof}"}}"#),
        ),
        (
            ClientMessage::Identify {
                reference: None,
                key: Key::from_bytes([0; 32]),
                proof: Signature::from_bytes([0; 64]),
                invitation: Some(code.clone()),
            },
            format!(
                r#"{{"type":"identify","key":"{key}","proof":"{proof}","invitation":"7Hq2kdX9mPq4"}}"#
            ),
        ),
        (
            ClientMessage::Register {
                reference: Some(3),
                key: Key::from_bytes([0; 32]),
                proof: Signature::from_bytes([0; 64]),
                name: "ana".to_owned(),
                invitation: code,
            },
            format!(
                r#"{{"type":"register","ref":3,"key":"{key}","proof":"{proof}","name":"ana","invitation":"7Hq2kdX9mPq4"}}"#
            ),
        ),
    ];
    for (message, line) in client {
        assert_eq!(message.encode(), line);
        assert_eq!(ClientMessage::decode(&line).expect(&line), message);
    }

    let challenge = ServerMessage::Challenge {
        reference: Some(1),
        protocol: 0,
        key: Key::from_bytes([0; 32]),
        nonce: Nonce::from_bytes([0xfb; 32]),
        proof: Signature::from_bytes([0; 64]),
        requires: vec![part("sealed")],
    };
    let line = format!(
        r#"{{"type":"challenge","ref":1,"protocol":0,"key":"{key}","nonce":"{nonce}","proof":"{proof}","requires":["sealed"]}}"#
    );
    assert_eq!(challenge.encode(), line);
    assert_eq!(ServerMessage::decode(&line).expect(&line), challenge);
}

#[test]
fn refuses_a_sign_in_message_with_a_value_that_breaks_a_rule() {
    let key = "A".repeat(43);
    let proof = "A".repeat(86);
    let lines = [
        format!(r#"{{"type":"hello","protocol":0,"address":"a:1","nonce":"{key}"}}"#),
        format!(
            r#"{{"type":"hello","protocol":"0","address":"a:1","nonce":"{key}","supports":[]}}"#
        ),
        format!(
            r#"{{"type":"hello","protocol":-1,"address":"a:1","nonce":"{key}","supports":[]}}"#
        ),
        format!(
            r#"{{"type":"hello","protocol":0,"address":"a:1","nonce":"{key}A","supports":[]}}"#
        ),
        format!(
            r#"{{"type":"hello","protocol":0,"address":"a:1","nonce":"{key}","supports":["Sealed"]}}"#
        ),
        format!(
            r#"{{"type":"hello","protocol":0,"address":"a:1","nonce":"{key}","supports":"sealed"}}"#
        ),
        format!(r#"{{"type":"identify","key":"{key}A","proof":"{proof}"}}"#),
        format!(r#"{{"type":"identify","key":"{key}","proof":"{key}"}}"#),
        format!(r#"{{"type":"identify","key":"{key}","proof":"{proof}","invitation":"short"}}"#),
        format!(r#"{{"type":"identify","key":"{key}","proof":"{proof}","name":"ana"}}"#),
        format!(r#"{{"type":"register","key":"{key}","proof":"{proof}","name":"ana"}}"#),
        format!(
            r#"{{"type":"register","key":"{key}","proof":"{proof}","invitation":"7Hq2kdX9mPq4"}}"#
        ),
        format!(
            r#"{{"type":"challenge","protocol":0,"key":"{key}","nonce":"{key}","proof":"{proof}","requires":[]}}"#
        ),
    ];
    for line in lines {
        assert!(
            matches!(ClientMessage::decode(&line), Err(DecodeError::Invalid(_))),
            "{line}"
        );
    }

    let lines = [
        format!(
            r#"{{"type":"challenge","protocol":0,"key":"{key}","nonce":"{key}","proof":"{proof}"}}"#
        ),
        format!(
            r#"{{"type":"challenge","protocol":0,"key":"{key}","nonce":"{key}","proof":"{key}","requires":[]}}"#
        ),
        format!(
            r#"{{"type":"challenge","protocol":0,"key":"{key}","nonce":"{key}","proof":"{proof}","requires":["a"]}}"#
        ),
    ];
    for line in lines {
        assert!(
            matches!(ServerMessage::decode(&line), Err(DecodeError::Invalid(_))),
            "{line}"
        );
    }
}

#[test]
fn the_protocol_version_is_zero() {
    assert_eq!(PROTOCOL_VERSION, 0);
}

#[test]
fn the_default_port_spells_post_on_a_telephone_keypad() {
    assert_eq!(crate::DEFAULT_PORT, 7678);
}

#[test]
fn writes_each_client_message_as_one_line() {
    for (message, line) in client_examples() {
        assert_eq!(message.encode(), line);
    }
}

#[test]
fn gives_the_number_that_a_client_message_carries() {
    let hello = |reference| ClientMessage::Hello {
        reference,
        protocol: 0,
        address: "localhost:7678".to_owned(),
        nonce: Nonce::from_bytes([0xfb; 32]),
        supports: Vec::new(),
    };
    assert_eq!(hello(Some(4_294_967_295)).reference(), Some(4_294_967_295));
    assert_eq!(hello(None).reference(), None);

    for (message, line) in client_examples() {
        let value: serde_json::Value = serde_json::from_str(line).expect(line);
        let number = value.get("ref").and_then(serde_json::Value::as_u64);
        assert_eq!(message.reference().map(u64::from), number, "{line}");
    }
}

#[test]
fn reads_each_client_message() {
    for (message, line) in client_examples() {
        assert_eq!(ClientMessage::decode(line).expect(line), message);
    }
}

#[test]
fn writes_each_server_message_as_one_line() {
    for (message, line) in all_server_examples() {
        assert_eq!(message.encode(), line);
    }
}

#[test]
fn reads_each_server_message() {
    for (message, line) in all_server_examples() {
        assert_eq!(ServerMessage::decode(line).expect(line), message);
    }
}

fn examine_error_codes(codes: &[(ErrorCode, &str, &str)]) {
    for (code, word, text) in codes {
        assert_eq!(code.text(), *text);
        let line = format!(r#"{{"type":"error","ref":5,"code":"{word}","text":"{text}"}}"#);
        assert_eq!(ServerMessage::error(Some(5), *code).encode(), line);
        assert_eq!(
            ServerMessage::decode(&line).expect(word),
            ServerMessage::Error {
                reference: Some(5),
                code: *code,
                text: (*text).to_owned(),
            }
        );
    }
}

#[test]
fn each_error_code_of_the_sign_in_has_a_word_form_and_a_text() {
    let codes = [
        (
            ErrorCode::WrongAddress,
            "wrong-address",
            "This server does not answer to this address.",
        ),
        (
            ErrorCode::IdentifyRequired,
            "identify-required",
            "Sign in before you send this message.",
        ),
        (
            ErrorCode::DuplicateIdentify,
            "duplicate-identify",
            "The session already has an account.",
        ),
        (
            ErrorCode::InvalidProof,
            "invalid-proof",
            "The signature is not valid for this connection.",
        ),
        (
            ErrorCode::UnknownAccount,
            "unknown-account",
            "This server has no account for this key.",
        ),
        (
            ErrorCode::AccountExists,
            "account-exists",
            "This key already has an account.",
        ),
        (
            ErrorCode::InvalidInvitation,
            "invalid-invitation",
            "The invitation is not valid.",
        ),
        (
            ErrorCode::MissingPart,
            "missing-part",
            "The client does not support a part of the protocol that this server requires.",
        ),
        (
            ErrorCode::ServerError,
            "server-error",
            "The server has a problem. Try again later.",
        ),
    ];
    examine_error_codes(&codes);
}
#[test]
fn each_error_code_has_a_word_form_and_a_text() {
    let codes = [
        (
            ErrorCode::BadMessage,
            "bad-message",
            "The message is not a valid protocol message.",
        ),
        (
            ErrorCode::UnsupportedProtocol,
            "unsupported-protocol",
            "The server does not support this protocol version.",
        ),
        (
            ErrorCode::HelloRequired,
            "hello-required",
            "The first message must be a hello message.",
        ),
        (
            ErrorCode::DuplicateHello,
            "duplicate-hello",
            "The connection already had a hello message.",
        ),
        (
            ErrorCode::InvalidName,
            "invalid-name",
            "A name has 2 to 20 characters: a to z, 0 to 9, hyphen, and underscore.",
        ),
        (
            ErrorCode::NameTaken,
            "name-taken",
            "This name is not available.",
        ),
        (
            ErrorCode::NoSuchChannel,
            "no-such-channel",
            "The channel does not exist.",
        ),
        (
            ErrorCode::EmptyText,
            "empty-text",
            "A message must contain text.",
        ),
        (
            ErrorCode::TextTooLong,
            "text-too-long",
            "A message can have a maximum of 4000 characters.",
        ),
        (
            ErrorCode::NotPermitted,
            "not-permitted",
            "Your account does not have the permission for this request.",
        ),
        (
            ErrorCode::TooSoon,
            "too-soon",
            "The name changed a short time ago. Try again later.",
        ),
    ];
    examine_error_codes(&codes);
}

#[test]
fn each_error_code_of_a_channel_has_a_word_form_and_a_text() {
    let codes = [
        (
            ErrorCode::InvalidTopic,
            "invalid-topic",
            "A topic is one line of text with a maximum of 300 characters.",
        ),
        (
            ErrorCode::NoSuchUser,
            "no-such-user",
            "The server has no user with this name.",
        ),
        (
            ErrorCode::NoSuchMember,
            "no-such-member",
            "The user is not a member of this channel.",
        ),
        (
            ErrorCode::NotPrivate,
            "not-private",
            "Only a private channel has invitations.",
        ),
        (
            ErrorCode::AlreadyInvited,
            "already-invited",
            "The user already has an invitation to this channel.",
        ),
        (
            ErrorCode::NotInvited,
            "not-invited",
            "The user has no invitation to this channel.",
        ),
        (
            ErrorCode::InvalidChannelName,
            "invalid-channel-name",
            "A channel name has 2 to 30 characters: a to z, 0 to 9, hyphen, and underscore.",
        ),
        (
            ErrorCode::TooManyChannels,
            "too-many-channels",
            "The account owns the maximum number of channels.",
        ),
        (
            ErrorCode::AlreadyMember,
            "already-member",
            "The user is already a member of this channel.",
        ),
        (
            ErrorCode::NotMember,
            "not-member",
            "You are not a member of this channel.",
        ),
        (
            ErrorCode::OwnerCannotLeave,
            "owner-cannot-leave",
            "The owner of a channel cannot leave it.",
        ),
    ];
    examine_error_codes(&codes);
}

#[test]
fn writes_a_line_break_in_a_text_as_an_escape() {
    let line = ClientMessage::Send {
        reference: None,
        channel: "general".to_owned(),
        text: "one\r\ntwo".to_owned(),
    }
    .encode();
    assert!(!line.contains('\n'));
    assert!(!line.contains('\r'));
    assert!(line.contains(r"one\r\ntwo"));
}

#[test]
fn refuses_a_line_that_is_not_a_known_message() {
    for line in [
        "",
        "hello",
        "[]",
        "{}",
        r#"{"type":"dance"}"#,
        r#"{"type":"send"}"#,
        r#"{"type":"send","channel":"general"}"#,
        r#"{"type":"send","channel":7,"text":"Hello"}"#,
        r#"{"type":"send","channel":"general","text":"Hello","extra":1}"#,
        r#"{"type":"send","ref":-1,"channel":"general","text":"Hello"}"#,
        r#"{"type":"send","ref":4294967296,"channel":"general","text":"Hello"}"#,
        r#"{"type":"send","ref":1.5,"channel":"general","text":"Hello"}"#,
        r#"{"type":"SEND","channel":"general","text":"Hello"}"#,
        r#"{"type":"guest","protocol":0,"name":"ana"}"#,
        r#"{"type":"welcome","name":"ana","role":"member"}"#,
        r#"{"type":"send","channel":"general","text":"Hello"} x"#,
        r#"{"type":"invite","lifetime":0}"#,
        r#"{"type":"invite","lifetime":-1}"#,
        r#"{"type":"invite","lifetime":4294967296}"#,
        r#"{"type":"invite","lifetime":"7 days"}"#,
        r#"{"type":"invite","uses":0}"#,
        r#"{"type":"invite","uses":1.5}"#,
        r#"{"type":"invite","end":"2026-11-28T14:02:05.000Z"}"#,
        r#"{"type":"invitations","list":[]}"#,
        r#"{"type":"revoke"}"#,
        r#"{"type":"revoke","code":"short"}"#,
        r#"{"type":"revoke","code":"7Hq2kdX9mPq4","link":"x"}"#,
        r#"{"type":"rename"}"#,
        r#"{"type":"rename","name":7}"#,
        r#"{"type":"rename","old":"ana","new":"anna","time":"2026-11-21T14:04:00.000Z"}"#,
        r#"{"type":"channel-create","channel":"general"}"#,
        r#"{"type":"channel-create","kind":"public"}"#,
        r#"{"type":"channel-create","channel":"general","kind":"secret"}"#,
        r#"{"type":"channel-create","channel":7,"kind":"public"}"#,
        r#"{"type":"channel-create","channel":"general","kind":"public","topic":"x"}"#,
        r#"{"type":"channelcreate","channel":"general","kind":"public"}"#,
        r#"{"type":"channel_create","channel":"general","kind":"public"}"#,
        r#"{"type":"ready"}"#,
        r#"{"type":"join"}"#,
        r#"{"type":"join","channel":7}"#,
        r#"{"type":"join","channel":"general","name":"ben","time":"2026-11-21T14:02:05.000Z"}"#,
        r#"{"type":"leave"}"#,
        r#"{"type":"leave","channel":"general","name":"ben"}"#,
        r#"{"type":"channels","list":[]}"#,
        r#"{"type":"channels","more":true}"#,
        r#"{"type":"channel-topic","channel":"general"}"#,
        r#"{"type":"channel-topic","topic":"Plans"}"#,
        r#"{"type":"channel-topic","channel":"general","topic":7}"#,
        r#"{"type":"channel-topic","channel":"general","topic":"Plans","by":"ana"}"#,
        r#"{"type":"channel-rename","channel":"general"}"#,
        r#"{"type":"channel-rename","name":"lounge"}"#,
        r#"{"type":"channel-rename","channel":"general","name":"lounge","time":"2026-11-21T14:08:00.000Z"}"#,
        r#"{"type":"channel-delete"}"#,
        r#"{"type":"channel-delete","channel":7}"#,
        r#"{"type":"channel-delete","channel":"general","time":"2026-11-21T14:09:00.000Z"}"#,
        r#"{"type":"channel-give","channel":"general"}"#,
        r#"{"type":"channel-give","name":"ben"}"#,
        r#"{"type":"channel-give","channel":"general","name":"ben","time":"2026-11-21T14:10:00.000Z"}"#,
        r#"{"type":"channel-invite","channel":"secret"}"#,
        r#"{"type":"channel-invite","name":"ben"}"#,
        r#"{"type":"channel-invite","channel":"secret","name":"ben","owner":"ana"}"#,
        r#"{"type":"channel-cancel","channel":"secret"}"#,
        r#"{"type":"channel-cancel","name":"ben"}"#,
        r#"{"type":"channel-refuse"}"#,
        r#"{"type":"channel-refuse","channel":"secret","name":"ben"}"#,
    ] {
        assert!(
            matches!(ClientMessage::decode(line), Err(DecodeError::Invalid(_))),
            "{line}"
        );
    }
}

#[test]
fn refuses_a_server_message_with_a_value_that_breaks_a_rule() {
    for line in [
        r#"{"type":"welcome","name":"Ana","role":"member"}"#,
        r#"{"type":"welcome","name":"a","role":"member"}"#,
        r#"{"type":"join","channel":"general","name":"ben","time":"yesterday"}"#,
        r#"{"type":"join","channel":"general","name":"ben","time":1795269727120}"#,
        r#"{"type":"error","code":"no-such-code","text":"x"}"#,
        r#"{"type":"send","channel":"general","text":"Hello"}"#,
        r#"{"type":"invitations"}"#,
        r#"{"type":"invitations","list":{}}"#,
        r#"{"type":"rename","name":"anna"}"#,
        r#"{"type":"rename","old":"ana","new":"Anna","time":"2026-11-21T14:04:00.000Z"}"#,
        r#"{"type":"rename","old":"ana","new":"anna"}"#,
        r#"{"type":"invitations","list":[{"code":"7Hq2kdX9mPq4","link":"x","created":"2026-11-21T14:02:05.000Z"}]}"#,
        r#"{"type":"invitations","list":[{"code":"short","link":"x","created":"2026-11-21T14:02:05.000Z","used":0}]}"#,
        r#"{"type":"invitations","list":[{"code":"7Hq2kdX9mPq4","link":"x","created":"2026-11-21T14:02:05.000Z","used":0,"owner":true}]}"#,
        r#"{"type":"invitations","list":[{"code":"7Hq2kdX9mPq4","link":"x","created":"2026-11-21T14:02:05.000Z","used":-1}]}"#,
        r#"{"type":"channel","channel":"General","kind":"public","topic":"","owner":"ana","created":"2026-11-21T14:02:05.000Z"}"#,
        r#"{"type":"channel","channel":"general","kind":"open","topic":"","owner":"ana","created":"2026-11-21T14:02:05.000Z"}"#,
        r#"{"type":"channel","channel":"general","kind":"public","owner":"ana","created":"2026-11-21T14:02:05.000Z"}"#,
        r#"{"type":"channel","channel":"general","kind":"public","topic":"","owner":"Ana","created":"2026-11-21T14:02:05.000Z"}"#,
        r#"{"type":"channel","channel":"general","kind":"public","topic":"","owner":"ana","created":"2026-11-21T14:02:05.000Z","members":1}"#,
        r#"{"type":"ready","ref":1}"#,
        r#"{"type":"join","channel":"general"}"#,
        r#"{"type":"join","channel":"Not A Name","name":"ben","time":"2026-11-21T14:02:05.000Z"}"#,
        r#"{"type":"leave","channel":"g","name":"ben","time":"2026-11-21T14:03:00.000Z"}"#,
        r#"{"type":"message","channel":"General","from":"ana","time":"2026-11-21T14:02:07.120Z","text":"Hello"}"#,
        r#"{"type":"leave","channel":"general"}"#,
        r#"{"type":"leave","ref":"7","channel":"general","name":"ben","time":"2026-11-21T14:03:00.000Z"}"#,
        r#"{"type":"channels"}"#,
        r#"{"type":"channels","list":[],"more":"yes"}"#,
        r#"{"type":"channels","list":[{"channel":"general","kind":"public","topic":""}]}"#,
        r#"{"type":"channels","list":[{"channel":"general","kind":"public","topic":"","members":-1}]}"#,
        r#"{"type":"channels","list":[{"channel":"General","kind":"public","topic":"","members":1}]}"#,
        r#"{"type":"channels","list":[{"channel":"general","kind":"public","topic":"","members":1,"owner":"ana"}]}"#,
        r#"{"type":"channel-create","channel":"general","kind":"public"}"#,
        r#"{"type":"channel-topic","channel":"general","topic":"Plans"}"#,
        r#"{"type":"channel-topic","channel":"general","by":"ana","time":"2026-11-21T14:06:00.000Z"}"#,
        r#"{"type":"channel-topic","channel":"General","topic":"Plans","by":"ana","time":"2026-11-21T14:06:00.000Z"}"#,
        r#"{"type":"channel-topic","channel":"general","topic":"Plans","by":"Ana","time":"2026-11-21T14:06:00.000Z"}"#,
        r#"{"type":"channel-rename","channel":"general","name":"lounge"}"#,
        r#"{"type":"channel-rename","channel":"general","name":"Lounge","time":"2026-11-21T14:08:00.000Z"}"#,
        r#"{"type":"channel-rename","channel":"General","name":"lounge","time":"2026-11-21T14:08:00.000Z"}"#,
        r#"{"type":"channel-delete","channel":"general"}"#,
        r#"{"type":"channel-delete","channel":"General","time":"2026-11-21T14:09:00.000Z"}"#,
        r#"{"type":"channel-give","channel":"general","name":"ben"}"#,
        r#"{"type":"channel-give","channel":"general","name":"Ben","time":"2026-11-21T14:10:00.000Z"}"#,
        r#"{"type":"channel-give","channel":"General","name":"ben","time":"2026-11-21T14:10:00.000Z"}"#,
        r#"{"type":"channel-invite","channel":"secret","name":"ben","time":"2026-11-21T14:12:00.000Z"}"#,
        r#"{"type":"channel-invite","channel":"secret","name":"Ben","owner":"ana","time":"2026-11-21T14:12:00.000Z"}"#,
        r#"{"type":"channel-invite","channel":"Secret","name":"ben","owner":"ana","time":"2026-11-21T14:12:00.000Z"}"#,
        r#"{"type":"channel-cancel","channel":"secret","name":"ben"}"#,
        r#"{"type":"channel-cancel","channel":"secret","name":"Ben","time":"2026-11-21T14:14:00.000Z"}"#,
        r#"{"type":"channel-refuse","channel":"secret","time":"2026-11-21T14:15:00.000Z"}"#,
        r#"{"type":"channel-refuse","channel":"Secret","name":"ben","time":"2026-11-21T14:15:00.000Z"}"#,
    ] {
        assert!(
            matches!(ServerMessage::decode(line), Err(DecodeError::Invalid(_))),
            "{line}"
        );
    }
}

#[test]
fn refuses_a_message_on_more_than_one_line() {
    for line in [
        "{\"type\":\"send\",\n\"channel\":\"general\",\"text\":\"Hello\"}",
        "{\"type\":\"send\",\"channel\":\"general\",\"text\":\"Hello\"}\n",
        "{\"type\":\"send\",\r\"channel\":\"general\",\"text\":\"Hello\"}",
    ] {
        assert!(
            matches!(ClientMessage::decode(line), Err(DecodeError::LineBreak)),
            "{line:?}"
        );
        assert!(
            matches!(ServerMessage::decode(line), Err(DecodeError::LineBreak)),
            "{line:?}"
        );
    }
}

#[test]
fn refuses_a_line_that_is_too_long() {
    assert_eq!(MAX_LINE_BYTES, 65_536);

    let start = r#"{"type":"send","channel":"general","text":""#;
    let end = r#""}"#;
    let fill = MAX_LINE_BYTES - start.len() - end.len();

    let longest = format!("{start}{}{end}", "a".repeat(fill));
    assert_eq!(longest.len(), MAX_LINE_BYTES);
    assert!(ClientMessage::decode(&longest).is_ok());

    let too_long = format!("{start}{}{end}", "a".repeat(fill + 1));
    assert!(matches!(
        ClientMessage::decode(&too_long),
        Err(DecodeError::TooLong)
    ));
    assert!(matches!(
        ServerMessage::decode(&too_long),
        Err(DecodeError::TooLong)
    ));
}

#[test]
fn describes_each_decode_error() {
    assert_eq!(
        DecodeError::TooLong.to_string(),
        "a line can have a maximum of 65536 bytes"
    );
    assert_eq!(
        DecodeError::LineBreak.to_string(),
        "a message must be on one line"
    );
    let invalid = ClientMessage::decode("{}").expect_err("not a message");
    assert!(
        invalid
            .to_string()
            .starts_with("the line is not a valid message: ")
    );
    assert!(std::error::Error::source(&invalid).is_some());
    assert!(std::error::Error::source(&DecodeError::TooLong).is_none());
}

#[test]
fn reads_a_channel_list_that_tells_that_it_is_complete() {
    let line = r#"{"type":"channels","list":[],"more":false}"#;

    assert_eq!(
        ServerMessage::decode(line).expect(line),
        ServerMessage::Channels {
            reference: None,
            list: Vec::new(),
            more: false,
        }
    );
}
