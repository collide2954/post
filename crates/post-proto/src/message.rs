use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::codec::{self, DecodeError};
use crate::{Name, Timestamp};

pub const PROTOCOL_VERSION: u32 = 0;

/// A message from a client to the server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
pub enum ClientMessage {
    /// Starts a session. This must be the first message on a connection.
    Hello {
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
}

/// A message from the server to a client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
pub enum ServerMessage {
    /// Accepts a `hello` message. The session now has a name.
    Welcome {
        /// The number from the `hello` message, if it had one.
        #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
        reference: Option<u32>,
        /// The protocol version of the session.
        protocol: u32,
        /// The name of the user in this session.
        name: Name,
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
    /// The client sent a second `hello` message after a `welcome` message.
    DuplicateHello,
    /// The name does not obey the rules for a name.
    InvalidName,
    /// A different connected user has the name.
    NameTaken,
    /// The channel does not exist.
    NoSuchChannel,
    /// The text has no visible character.
    EmptyText,
    /// The text has too many characters.
    TextTooLong,
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
            Self::DuplicateHello => "The session already has a name.",
            Self::InvalidName => {
                "A name has 2 to 20 characters: a to z, 0 to 9, hyphen, and underscore."
            }
            Self::NameTaken => "A different user has this name.",
            Self::NoSuchChannel => "The channel does not exist.",
            Self::EmptyText => "A message must contain text.",
            Self::TextTooLong => "A message can have a maximum of 4000 characters.",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ClientMessage, ErrorCode, PROTOCOL_VERSION, ServerMessage};
    use crate::{DecodeError, MAX_LINE_BYTES, Name, Timestamp};

    fn name(text: &str) -> Name {
        Name::parse(text).expect(text)
    }

    fn time(text: &str) -> Timestamp {
        Timestamp::parse(text).expect(text)
    }

    fn client_examples() -> Vec<(ClientMessage, &'static str)> {
        vec![
            (
                ClientMessage::Hello {
                    reference: None,
                    protocol: 0,
                    name: "ana".to_owned(),
                },
                r#"{"type":"hello","protocol":0,"name":"ana"}"#,
            ),
            (
                ClientMessage::Hello {
                    reference: Some(7),
                    protocol: 3,
                    name: "Not A Name".to_owned(),
                },
                r#"{"type":"hello","ref":7,"protocol":3,"name":"Not A Name"}"#,
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
        ]
    }

    fn server_examples() -> Vec<(ServerMessage, &'static str)> {
        vec![
            (
                ServerMessage::Welcome {
                    reference: None,
                    protocol: 0,
                    name: name("ana"),
                },
                r#"{"type":"welcome","protocol":0,"name":"ana"}"#,
            ),
            (
                ServerMessage::Welcome {
                    reference: Some(4_294_967_295),
                    protocol: 0,
                    name: name("ana"),
                },
                r#"{"type":"welcome","ref":4294967295,"protocol":0,"name":"ana"}"#,
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
    fn the_protocol_version_is_zero() {
        assert_eq!(PROTOCOL_VERSION, 0);
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
                "The session already has a name.",
            ),
            (
                ErrorCode::InvalidName,
                "invalid-name",
                "A name has 2 to 20 characters: a to z, 0 to 9, hyphen, and underscore.",
            ),
            (
                ErrorCode::NameTaken,
                "name-taken",
                "A different user has this name.",
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
        ];
        for (code, word, text) in codes {
            assert_eq!(code.text(), text);
            let line = format!(r#"{{"type":"error","ref":5,"code":"{word}","text":"{text}"}}"#);
            assert_eq!(ServerMessage::error(Some(5), code).encode(), line);
            assert_eq!(
                ServerMessage::decode(&line).expect(word),
                ServerMessage::Error {
                    reference: Some(5),
                    code,
                    text: text.to_owned(),
                }
            );
        }
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
            r#"{"type":"hello"}"#,
            r#"{"type":"hello","protocol":0}"#,
            r#"{"type":"hello","protocol":"0","name":"ana"}"#,
            r#"{"type":"hello","protocol":-1,"name":"ana"}"#,
            r#"{"type":"hello","protocol":0,"name":"ana","extra":1}"#,
            r#"{"type":"hello","ref":-1,"protocol":0,"name":"ana"}"#,
            r#"{"type":"hello","ref":4294967296,"protocol":0,"name":"ana"}"#,
            r#"{"type":"hello","ref":1.5,"protocol":0,"name":"ana"}"#,
            r#"{"type":"HELLO","protocol":0,"name":"ana"}"#,
            r#"{"type":"welcome","protocol":0,"name":"ana"}"#,
            r#"{"type":"send","channel":"lobby"}"#,
            r#"{"type":"hello","protocol":0,"name":"ana"} x"#,
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
            r#"{"type":"welcome","protocol":0,"name":"Ana"}"#,
            r#"{"type":"welcome","protocol":0,"name":"a"}"#,
            r#"{"type":"join","channel":"lobby","name":"ben","time":"yesterday"}"#,
            r#"{"type":"join","channel":"lobby","name":"ben","time":1795269727120}"#,
            r#"{"type":"error","code":"no-such-code","text":"x"}"#,
            r#"{"type":"hello","protocol":0,"name":"ana"}"#,
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
            "{\"type\":\"hello\",\n\"protocol\":0,\"name\":\"ana\"}",
            "{\"type\":\"hello\",\"protocol\":0,\"name\":\"ana\"}\n",
            "{\"type\":\"hello\",\r\"protocol\":0,\"name\":\"ana\"}",
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

        let start = r#"{"type":"hello","protocol":0,"name":""#;
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
