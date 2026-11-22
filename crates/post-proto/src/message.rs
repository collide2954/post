use std::num::NonZeroU32;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::codec::{self, DecodeError};
use crate::{ChannelName, Code, Key, Name, Nonce, Part, Signature, Timestamp};

pub const PROTOCOL_VERSION: u32 = 0;
pub const DEFAULT_PORT: u16 = 7678;

/// A message from a client to the server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
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
    /// Changes the name of the account.
    Rename {
        /// A number that the answer of the server repeats.
        #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
        reference: Option<u32>,
        /// The new name.
        name: String,
    },
    /// Makes a channel. The sender becomes its owner and its first member.
    ChannelCreate {
        /// A number that the answer of the server repeats.
        #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
        reference: Option<u32>,
        /// The name of the new channel, without the `#` sign.
        channel: String,
        /// The kind of the channel.
        kind: ChannelKind,
    },
    /// Joins a channel.
    Join {
        /// A number that the answer of the server repeats.
        #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
        reference: Option<u32>,
        /// The name of the channel, without the `#` sign.
        channel: String,
    },
    /// Leaves a channel.
    Leave {
        /// A number that the answer of the server repeats.
        #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
        reference: Option<u32>,
        /// The name of the channel, without the `#` sign.
        channel: String,
    },
    /// Asks for the channel list of the server.
    Channels {
        /// A number that the answer of the server repeats.
        #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
        reference: Option<u32>,
    },
    /// Changes the topic of a channel. Only the owner of the channel can change it.
    ChannelTopic {
        /// A number that the answer of the server repeats.
        #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
        reference: Option<u32>,
        /// The name of the channel, without the `#` sign.
        channel: String,
        /// The new topic. An empty text removes the topic.
        topic: String,
    },
    /// Changes the name of a channel. Only the owner of the channel can change it.
    ChannelRename {
        /// A number that the answer of the server repeats.
        #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
        reference: Option<u32>,
        /// The present name of the channel, without the `#` sign.
        channel: String,
        /// The new name of the channel.
        name: String,
    },
}

/// A message from the server to a client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
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
    /// Gives the state of a channel to one of its members.
    Channel {
        /// The number from the request. Only the connection that sent the request gets this field.
        #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
        reference: Option<u32>,
        /// The name of the channel, without the `#` sign.
        channel: ChannelName,
        /// The kind of the channel.
        kind: ChannelKind,
        /// The topic of the channel. The topic can be empty.
        topic: String,
        /// The name of the owner of the channel.
        owner: Name,
        /// The time when the owner made the channel.
        created: Timestamp,
    },
    /// Tells that the state after the sign-in is complete.
    Ready {},
    /// Gives a part of the channel list of the server.
    Channels {
        /// The number from the request, if it had one.
        #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
        reference: Option<u32>,
        /// The channels of this part, in the sequence of their names.
        list: Vec<ListedChannel>,
        /// The value `true` tells that a later message gives more channels.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        more: bool,
    },
    /// Tells that a user came into a channel.
    Join {
        /// The name of the channel, without the `#` sign.
        channel: ChannelName,
        /// The name of the user.
        name: Name,
        /// The time when the user came in.
        time: Timestamp,
    },
    /// Tells that a user went out of a channel.
    Leave {
        /// The number from the `leave` message. Only the connection that sent it gets this field.
        #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
        reference: Option<u32>,
        /// The name of the channel, without the `#` sign.
        channel: ChannelName,
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
        channel: ChannelName,
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
    /// Tells that a user has a new name.
    Rename {
        /// The number from the `rename` message. Only the connection that sent it gets this field.
        #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
        reference: Option<u32>,
        /// The earlier name of the user.
        old: Name,
        /// The new name of the user.
        new: Name,
        /// The time of the change.
        time: Timestamp,
    },
    /// Tells that a channel has a new topic.
    ChannelTopic {
        /// The number from the `channel-topic` message. Only the connection that sent it gets this field.
        #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
        reference: Option<u32>,
        /// The name of the channel, without the `#` sign.
        channel: ChannelName,
        /// The new topic of the channel. The topic can be empty.
        topic: String,
        /// The name of the user who changed the topic.
        by: Name,
        /// The time of the change.
        time: Timestamp,
    },
    /// Tells that a channel has a new name.
    ChannelRename {
        /// The number from the `channel-rename` message. Only the connection that sent it gets this field.
        #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
        reference: Option<u32>,
        /// The earlier name of the channel, without the `#` sign.
        channel: ChannelName,
        /// The new name of the channel.
        name: ChannelName,
        /// The time of the change.
        time: Timestamp,
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

/// A channel in the channel list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListedChannel {
    /// The name of the channel, without the `#` sign.
    pub channel: ChannelName,
    /// The kind of the channel.
    pub kind: ChannelKind,
    /// The topic of the channel. The topic can be empty.
    pub topic: String,
    /// The number of the members of the channel.
    pub members: u32,
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
    /// The account or the channel changed its name a short time ago.
    TooSoon,
    /// The name does not obey the rules for a channel name.
    InvalidChannelName,
    /// The account owns the maximum number of channels.
    TooManyChannels,
    /// The user is already a member of the channel.
    AlreadyMember,
    /// The user is not a member of the channel.
    NotMember,
    /// The owner of a channel cannot leave it.
    OwnerCannotLeave,
    /// The topic does not obey the rules for a topic.
    InvalidTopic,
    /// The server has a problem and cannot do the task.
    ServerError,
}

/// The kind of a channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ChannelKind {
    /// Each member of the server can join the channel.
    Public,
    /// An invitation from the owner is necessary to join the channel.
    Private,
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
    pub fn reference(&self) -> Option<u32> {
        match self {
            Self::Hello { reference, .. }
            | Self::Identify { reference, .. }
            | Self::Register { reference, .. }
            | Self::Send { reference, .. }
            | Self::Invite { reference, .. }
            | Self::Invitations { reference }
            | Self::Revoke { reference, .. }
            | Self::Rename { reference, .. }
            | Self::ChannelCreate { reference, .. }
            | Self::Join { reference, .. }
            | Self::Leave { reference, .. }
            | Self::Channels { reference }
            | Self::ChannelTopic { reference, .. }
            | Self::ChannelRename { reference, .. } => *reference,
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
            Self::TooSoon => "The name changed a short time ago. Try again later.",
            Self::InvalidChannelName => {
                "A channel name has 2 to 30 characters: a to z, 0 to 9, hyphen, and underscore."
            }
            Self::TooManyChannels => "The account owns the maximum number of channels.",
            Self::AlreadyMember => "The user is already a member of this channel.",
            Self::NotMember => "You are not a member of this channel.",
            Self::OwnerCannotLeave => "The owner of a channel cannot leave it.",
            Self::InvalidTopic => "A topic is one line of text with a maximum of 300 characters.",
            Self::ServerError => "The server has a problem. Try again later.",
        }
    }
}

#[cfg(test)]
mod tests;
