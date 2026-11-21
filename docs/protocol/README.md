# The Post protocol

This document is the specification of the Post protocol. A client uses the protocol to speak with a Post server.

The specification has three parts:

- This description.
- The JSON Schemas in the [schemas](schemas) directory. The build makes them from the Rust types in the `post-proto` crate.
- The example conversations in the [examples](examples) directory.

The automatic checks fail if the code, the schemas, and the examples do not agree.

## Protocol version

This document describes protocol version 0. Version 0 is not stable. It can change in each release of Post, and it makes no promise to other clients. Protocol version 1 starts with version 1.0 of the application.

Protocol version 0 in Post 0.1 is only for a test on one computer. It has no accounts and no encryption.

## Transport

A client opens a WebSocket connection to the path `/` of the server.

- In Post 0.1, the address is `ws://localhost:7678/`. The server listens only on the loopback addresses of the computer.
- Each protocol message is one WebSocket text message.
- The server answers a binary message with the error `bad-message`.

## Messages

Each message is one JSON object on one line.

- The object is flat. The `type` field gives the kind of message.
- A line has a maximum of 65,536 bytes. The server closes a connection that sends a longer line.
- A line contains no line break. JSON writes a line break in a text as `\n`.
- A message has only the fields that this document gives. The receiver refuses an object with a different field.

### The `ref` field

A client can put a `ref` field in each message that it sends. The value is an integer from 0 to 4,294,967,295. The client selects the value.

When the server answers a message, the answer contains the same `ref` value. If the message had no `ref` field, the answer has none.

### Names

A name identifies a user. A name obeys these rules:

- It has 2 to 20 characters.
- It contains only the letters `a` to `z`, the digits `0` to `9`, the hyphen, and the underscore.

A channel name in a message does not have the `#` sign. Post 0.1 has one channel, `lobby`.

### Text

The text of a message obeys these rules:

- It has a minimum of one visible character.
- It has a maximum of 4,000 characters. A character is one Unicode scalar value.

### Time

A time is in UTC and has milliseconds. It always has this form: `2026-11-21T14:02:07.120Z`. Only the server writes a time.

## Messages from the client

### `guest`

The `guest` message starts a session with a temporary name and no account. It must be the first message on a connection.

This message is temporary. The project removes it before version 0.2, when the application uses accounts.

| Field | Value |
|---|---|
| `type` | `"guest"` |
| `ref` | Optional. See "The `ref` field". |
| `protocol` | The protocol version of the client. This document describes version 0. |
| `name` | The name that the user wants. |

The server answers with a `welcome` message or with an `error` message. After an error, the client can send a new `guest` message on the same connection.

### `send`

The `send` message sends a text to a channel.

| Field | Value |
|---|---|
| `type` | `"send"` |
| `ref` | Optional. See "The `ref` field". |
| `channel` | The name of the channel. |
| `text` | The text. |

The server answers with a `message` message or with an `error` message.

## Messages from the server

### `welcome`

The `welcome` message starts the session of a user. The session now has a name.

| Field | Value |
|---|---|
| `type` | `"welcome"` |
| `ref` | The value from the message that the server accepts, if it had one. |
| `name` | The name of the user in this session. |
| `role` | The role of the user on this server: `"owner"` or `"member"`. A user with a temporary name is a member. |

After the `welcome` message, the server puts the user into the channel `lobby`. The server sends a `join` message to each user in the channel, also to the new user.

### `join`

The `join` message tells that a user came into a channel.

| Field | Value |
|---|---|
| `type` | `"join"` |
| `channel` | The name of the channel. |
| `name` | The name of the user. |
| `time` | The time when the user came in. |

### `leave`

The `leave` message tells that a user went out of a channel. In Post 0.1, this occurs when the connection of the user closes.

| Field | Value |
|---|---|
| `type` | `"leave"` |
| `channel` | The name of the channel. |
| `name` | The name of the user. |
| `time` | The time when the user went out. |

### `message`

The `message` message carries a text that a user sent to a channel. The server sends it to each user in the channel, also to the sender.

| Field | Value |
|---|---|
| `type` | `"message"` |
| `ref` | The value from the `send` message, if it had one. Only the sender gets this field. |
| `channel` | The name of the channel. |
| `from` | The name of the sender. |
| `time` | The time when the server got the message. |
| `text` | The text. |

### `error`

The `error` message refuses a message from the client. The connection stays open.

| Field | Value |
|---|---|
| `type` | `"error"` |
| `ref` | The value from the refused message, if it had one. |
| `code` | The cause of the error, for a program. |
| `text` | The cause of the error in English, for a person. |

A client must use the `code` field to make a decision. The `text` field can change.

| Code | Cause |
|---|---|
| `bad-message` | The line is not a valid protocol message. |
| `unsupported-protocol` | The server does not support the protocol version of the client. |
| `hello-required` | The client sent a different message before the `guest` message. |
| `duplicate-hello` | The client sent a `guest` message after a `welcome` message. |
| `invalid-name` | The name does not obey the rules for a name. |
| `name-taken` | A different connected user has the name. |
| `no-such-channel` | The channel does not exist. |
| `empty-text` | The text has no visible character. |
| `text-too-long` | The text has more than 4,000 characters. |

## Example conversations

Each file in the [examples](examples) directory shows one connection.

- A line that starts with `C: ` is a message from the client.
- A line that starts with `S: ` is a message from the server.
- A line that starts with `#` is a comment.

| File | Content |
|---|---|
| [two-users.txt](examples/two-users.txt) | Two users exchange text. |
| [errors.txt](examples/errors.txt) | The server refuses messages. Each error code shows one time. |
