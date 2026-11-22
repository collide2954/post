use std::num::NonZeroU32;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::codec::{self, DecodeError};
use crate::{Code, Key, Name, Nonce, Part, Signature, Timestamp};

pub const PROTOCOL_VERSION: u32 = 0;
pub const DEFAULT_PORT: u16 = 7678;
pub const LOBBY: &str = "lobby";

/// A message from a client to the server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
pub enum ClientMessage {
    /// Starts the sign-in. The server answers with a `challenge` message.
    Hello {
        /// A number that the answer of the server repeats.
        #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
        reference: Option<u32>,
        /// The protocol version of the client.
        protocol: u32,
        /// The address that the client used for the connection, as `host:port`.
        address: String,
        /// A random value that the client makes for this connection.
        nonce: Nonce,
        /// The optional parts of the protocol that the client supports.
        supports: Vec<Part>,
    },
    /// Signs in to an account that exists.
    Identify {
        /// A number that the answer of the server repeats.
        #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
        reference: Option<u32>,
        /// The public key of the account.
        key: Key,
        /// The signature of the account key for this connection.
        proof: Signature,
        /// The code of an owner link. It makes the account the server owner.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        invitation: Option<Code>,
    },
    /// Makes an account and signs in to it.
    Register {
        /// A number that the answer of the server repeats.
        #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
        reference: Option<u32>,
        /// The public key of the new account.
        key: Key,
        /// The signature of the account key for this connection.
        proof: Signature,
        /// The name that the user wants.
        name: String,
        /// The code of an invitation or of an owner link.
        invitation: Code,
    },
    /// Starts a session with a temporary name and no account.
    /// This message is temporary. It goes away before version 0.2.
    Guest {
        /// A number that the answer of the server repeats.
        #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
        reference: Option<u32>,
        /// The protocol version of the client.
        protocol: u32,
        /// The name that the user wants.
        name: String,
    },
    /// Sends a text to a channel.
    Send {
        /// A number that the answer of the server repeats.
        #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
        reference: Option<u32>,
        /// The name of the channel, without the `#` sign.
        channel: String,
        /// The text of the message.
        text: String,
    },
    /// Makes an invitation. Only the server owner can make one.
    Invite {
        /// A number that the answer of the server repeats.
        #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
        reference: Option<u32>,
        /// The number of seconds until the invitation ends. Without this field, it has no end.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        lifetime: Option<NonZeroU32>,
        /// The number of accounts that the invitation can make. Without this field, it has no limit.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        uses: Option<NonZeroU32>,
    },
    /// Asks for the open invitations of the server.
    Invitations {
        /// A number that the answer of the server repeats.
        #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
        reference: Option<u32>,
    },
    /// Cancels an invitation.
    Revoke {
        /// A number that the answer of the server repeats.
        #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
        reference: Option<u32>,
        /// The code of the invitation.
        code: Code,
    },
}

/// A message from the server to a client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
pub enum ServerMessage {
    /// Answers a `hello` message. It proves that the server has its key.
    Challenge {
        /// The number from the `hello` message, if it had one.
        #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
        reference: Option<u32>,
        /// The protocol version of the session.
        protocol: u32,
        /// The public key of the server.
        key: Key,
        /// A random value that the server makes for this connection.
        nonce: Nonce,
        /// The signature of the server key for this connection.
        proof: Signature,
        /// The optional parts of the protocol that a client must support.
        requires: Vec<Part>,
    },
    /// Starts the session of a user. The session now has a name.
    Welcome {
        /// The number from the message that the server accepts, if it had one.
        #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
        reference: Option<u32>,
        /// The name of the user in this session.
        name: Name,
        /// The role of the user on this server.
        role: Role,
    },
    /// Tells that a user came into a channel.
    Join {
        /// The name of the channel, without the `#` sign.
        channel: String,
        /// The name of the user.
        name: Name,
        /// The time when the user came in.
        time: Timestamp,
    },
    /// Tells that a user went out of a channel.
    Leave {
        /// The name of the channel, without the `#` sign.
        channel: String,
        /// The name of the user.
        name: Name,
        /// The time when the user went out.
        time: Timestamp,
    },
    /// Carries a text that a user sent to a channel.
    Message {
        /// The number from the `send` message. Only the sender gets this field.
        #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
        reference: Option<u32>,
        /// The name of the channel, without the `#` sign.
        channel: String,
        /// The name of the sender.
        from: Name,
        /// The time when the server got the message.
        time: Timestamp,
        /// The text of the message.
        text: String,
    },
    /// Gives the open invitations of the server. The newest invitation is first.
    Invitations {
        /// The number from the request, if it had one.
        #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
        reference: Option<u32>,
        /// The open invitations.
        list: Vec<Invitation>,
    },
    /// Refuses a message from the client.
    Error {
        /// The number from the refused message, if it had one.
        #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
        reference: Option<u32>,
        /// The cause of the error, for a program.
        code: ErrorCode,
        /// The cause of the error in English, for a person.
        text: String,
    },
}

/// An open invitation to a server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Invitation {
    /// The code of the invitation.
    pub code: Code,
    /// The invite link. A person pastes it into an application.
    pub link: String,
    /// The time when the server made the invitation.
    pub created: Timestamp,
    /// The time when the invitation ends. Without this field, it has no end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<Timestamp>,
    /// The number of accounts that the invitation can make. Without this field, it has no limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uses: Option<u32>,
    /// The number of accounts that the invitation made.
    pub used: u32,
}

/// The cause of an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ErrorCode {
    /// The line is not a valid protocol message.
    BadMessage,
    /// The server does not support the protocol version of the client.
    UnsupportedProtocol,
    /// The client sent a different message before the `hello` message.
    HelloRequired,
    /// The client sent a second `hello` message.
    DuplicateHello,
    /// The server does not answer to the address that the client used.
    WrongAddress,
    /// The client sent a message that is only for a user with a session.
    IdentifyRequired,
    /// The client sent a sign-in message after a `welcome` message.
    DuplicateIdentify,
    /// The signature does not agree with the key and this connection.
    InvalidProof,
    /// The server has no account for the key.
    UnknownAccount,
    /// The key already has an account.
    AccountExists,
    /// The invitation does not exist, or it is not open.
    InvalidInvitation,
    /// The client does not support a part of the protocol that the server requires.
    MissingPart,
    /// The name does not obey the rules for a name.
    InvalidName,
    /// A different user has the name, or the name is reserved.
    NameTaken,
    /// The channel does not exist.
    NoSuchChannel,
    /// The text has no visible character.
    EmptyText,
    /// The text has too many characters.
    TextTooLong,
    /// The account does not have the permission for the request.
    NotPermitted,
    /// The server has a problem and cannot do the task.
    ServerError,
}

/// The role of an account on a server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// The account owns the server.
    Owner,
    /// The account is a normal member.
    Member,
}

impl ClientMessage {
    #[must_use]
    pub fn encode(&self) -> String {
        codec::encode(self)
    }

    pub fn decode(line: &str) -> Result<Self, DecodeError> {
        codec::decode(line)
    }
}

impl ServerMessage {
    #[must_use]
    pub fn error(reference: Option<u32>, code: ErrorCode) -> Self {
        Self::Error {
            reference,
            code,
            text: code.text().to_owned(),
        }
    }

    #[must_use]
    pub fn encode(&self) -> String {
        codec::encode(self)
    }

    pub fn decode(line: &str) -> Result<Self, DecodeError> {
        codec::decode(line)
    }
}

impl ErrorCode {
    #[must_use]
    pub fn text(self) -> &'static str {
        match self {
            Self::BadMessage => "The message is not a valid protocol message.",
            Self::UnsupportedProtocol => "The server does not support this protocol version.",
            Self::HelloRequired => "The first message must be a hello message.",
            Self::DuplicateHello => "The connection already had a hello message.",
            Self::WrongAddress => "This server does not answer to this address.",
            Self::IdentifyRequired => "Sign in before you send this message.",
            Self::DuplicateIdentify => "The session already has an account.",
            Self::InvalidProof => "The signature is not valid for this connection.",
            Self::UnknownAccount => "This server has no account for this key.",
            Self::AccountExists => "This key already has an account.",
            Self::InvalidInvitation => "The invitation is not valid.",
            Self::MissingPart => {
                "The client does not support a part of the protocol that this server requires."
            }
            Self::InvalidName => {
                "A name has 2 to 20 characters: a to z, 0 to 9, hyphen, and underscore."
            }
            Self::NameTaken => "This name is not available.",
            Self::NoSuchChannel => "The channel does not exist.",
            Self::EmptyText => "A message must contain text.",
            Self::TextTooLong => "A message can have a maximum of 4000 characters.",
            Self::NotPermitted => "Your account does not have the permission for this request.",
            Self::ServerError => "The server has a problem. Try again later.",
        }
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use super::{ClientMessage, ErrorCode, Invitation, PROTOCOL_VERSION, Role, ServerMessage};
    use crate::{Code, DecodeError, Key, MAX_LINE_BYTES, Name, Nonce, Part, Signature, Timestamp};

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

    fn time(text: &str) -> Timestamp {
        Timestamp::parse(text).expect(text)
    }

    fn client_examples() -> Vec<(ClientMessage, &'static str)> {
        vec![
            (
                ClientMessage::Guest {
                    reference: None,
                    protocol: 0,
                    name: "ana".to_owned(),
                },
                r#"{"type":"guest","protocol":0,"name":"ana"}"#,
            ),
            (
                ClientMessage::Guest {
                    reference: Some(7),
                    protocol: 3,
                    name: "Not A Name".to_owned(),
                },
                r#"{"type":"guest","ref":7,"protocol":3,"name":"Not A Name"}"#,
            ),
            (
                ClientMessage::Send {
                    reference: Some(1),
                    channel: "lobby".to_owned(),
                    text: "Hello".to_owned(),
                },
                r#"{"type":"send","ref":1,"channel":"lobby","text":"Hello"}"#,
            ),
            (
                ClientMessage::Send {
                    reference: None,
                    channel: "lobby".to_owned(),
                    text: "two\nlines".to_owned(),
                },
                r#"{"type":"send","channel":"lobby","text":"two\nlines"}"#,
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
        ]
    }

    fn server_examples() -> Vec<(ServerMessage, &'static str)> {
        vec![
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
                    channel: "lobby".to_owned(),
                    name: name("ben"),
                    time: time("2026-11-21T14:02:05.000Z"),
                },
                r#"{"type":"join","channel":"lobby","name":"ben","time":"2026-11-21T14:02:05.000Z"}"#,
            ),
            (
                ServerMessage::Leave {
                    channel: "lobby".to_owned(),
                    name: name("ben"),
                    time: time("2026-11-21T14:03:00.000Z"),
                },
                r#"{"type":"leave","channel":"lobby","name":"ben","time":"2026-11-21T14:03:00.000Z"}"#,
            ),
            (
                ServerMessage::Message {
                    reference: Some(1),
                    channel: "lobby".to_owned(),
                    from: name("ana"),
                    time: time("2026-11-21T14:02:07.120Z"),
                    text: "Hello".to_owned(),
                },
                r#"{"type":"message","ref":1,"channel":"lobby","from":"ana","time":"2026-11-21T14:02:07.120Z","text":"Hello"}"#,
            ),
            (
                ServerMessage::Message {
                    reference: None,
                    channel: "lobby".to_owned(),
                    from: name("ben"),
                    time: time("2026-11-21T14:02:09.480Z"),
                    text: "Hi".to_owned(),
                },
                r#"{"type":"message","channel":"lobby","from":"ben","time":"2026-11-21T14:02:09.480Z","text":"Hi"}"#,
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
                ServerMessage::Error {
                    reference: Some(2),
                    code: ErrorCode::NoSuchChannel,
                    text: "The channel does not exist.".to_owned(),
                },
                r#"{"type":"error","ref":2,"code":"no-such-channel","text":"The channel does not exist."}"#,
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
            format!(
                r#"{{"type":"identify","key":"{key}","proof":"{proof}","invitation":"short"}}"#
            ),
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
    fn the_one_channel_has_the_name_lobby() {
        assert_eq!(crate::LOBBY, "lobby");
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
    fn reads_each_client_message() {
        for (message, line) in client_examples() {
            assert_eq!(ClientMessage::decode(line).expect(line), message);
        }
    }

    #[test]
    fn writes_each_server_message_as_one_line() {
        for (message, line) in server_examples() {
            assert_eq!(message.encode(), line);
        }
    }

    #[test]
    fn reads_each_server_message() {
        for (message, line) in server_examples() {
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
        ];
        examine_error_codes(&codes);
    }

    #[test]
    fn writes_a_line_break_in_a_text_as_an_escape() {
        let line = ClientMessage::Send {
            reference: None,
            channel: "lobby".to_owned(),
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
            r#"{"type":"guest"}"#,
            r#"{"type":"guest","protocol":0}"#,
            r#"{"type":"guest","protocol":"0","name":"ana"}"#,
            r#"{"type":"guest","protocol":-1,"name":"ana"}"#,
            r#"{"type":"guest","protocol":0,"name":"ana","extra":1}"#,
            r#"{"type":"guest","ref":-1,"protocol":0,"name":"ana"}"#,
            r#"{"type":"guest","ref":4294967296,"protocol":0,"name":"ana"}"#,
            r#"{"type":"guest","ref":1.5,"protocol":0,"name":"ana"}"#,
            r#"{"type":"GUEST","protocol":0,"name":"ana"}"#,
            r#"{"type":"welcome","name":"ana","role":"member"}"#,
            r#"{"type":"send","channel":"lobby"}"#,
            r#"{"type":"guest","protocol":0,"name":"ana"} x"#,
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
            r#"{"type":"join","channel":"lobby","name":"ben","time":"yesterday"}"#,
            r#"{"type":"join","channel":"lobby","name":"ben","time":1795269727120}"#,
            r#"{"type":"error","code":"no-such-code","text":"x"}"#,
            r#"{"type":"guest","protocol":0,"name":"ana"}"#,
            r#"{"type":"invitations"}"#,
            r#"{"type":"invitations","list":{}}"#,
            r#"{"type":"invitations","list":[{"code":"7Hq2kdX9mPq4","link":"x","created":"2026-11-21T14:02:05.000Z"}]}"#,
            r#"{"type":"invitations","list":[{"code":"short","link":"x","created":"2026-11-21T14:02:05.000Z","used":0}]}"#,
            r#"{"type":"invitations","list":[{"code":"7Hq2kdX9mPq4","link":"x","created":"2026-11-21T14:02:05.000Z","used":0,"owner":true}]}"#,
            r#"{"type":"invitations","list":[{"code":"7Hq2kdX9mPq4","link":"x","created":"2026-11-21T14:02:05.000Z","used":-1}]}"#,
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
            "{\"type\":\"guest\",\n\"protocol\":0,\"name\":\"ana\"}",
            "{\"type\":\"guest\",\"protocol\":0,\"name\":\"ana\"}\n",
            "{\"type\":\"guest\",\r\"protocol\":0,\"name\":\"ana\"}",
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

        let start = r#"{"type":"guest","protocol":0,"name":""#;
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
}
