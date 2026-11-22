# The Post protocol

This document is the specification of the Post protocol. A client uses the protocol to speak with a Post server.

The specification has three parts:

- This description.
- The JSON Schemas in the [schemas](schemas) directory. The build makes them from the Rust types in the `post-proto` crate.
- The example conversations in the [examples](examples) directory.

The automatic checks fail if the code, the schemas, and the examples do not agree.

## Protocol version

This document describes protocol version 0. Version 0 is not stable. It can change in each release of Post, and it makes no promise to other clients. Protocol version 1 starts with version 1.0 of the application.

Protocol version 0 changes while the project builds Post 0.2.

## Transport

A client opens a WebSocket connection to the path `/` of the server.

- The connection uses TLS 1.3, and the address has the form `wss://chat.example.org/`. The default port is 443.
- A connection without TLS is possible only to the local computer, for development. Its address has the form `ws://localhost:7678/`.
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

A channel name in a message does not have the `#` sign. At this time, Post has one channel, `lobby`.

### Text

The text of a message obeys these rules:

- It has a minimum of one visible character.
- It has a maximum of 4,000 characters. A character is one Unicode scalar value.

### Time

A time is in UTC and has milliseconds. It always has this form: `2026-11-21T14:02:07.120Z`. Only the server writes a time.

### Keys, random values, and signatures

A key, a random value, and a signature are bytes. A message contains them as base64url text (RFC 4648). The text has no `=` characters at its end.

| Value | Bytes | Characters |
|---|---|---|
| A key | 32 | 43 |
| A random value | 32 | 43 |
| A signature | 64 | 86 |

### Addresses

An address in a message has the form `host:port`, for example `chat.example.org:443`.

- The host is in lower case.
- The port is always there.
- An IPv6 address is in brackets, for example `[::1]:7678`.

### Codes

The code of an invitation has 12 letters and digits, for example `7Hq2kdX9mPq4`. An invite link ends with this code.

An invite link has this form: `https://chat.example.org/i/7Hq2kdX9mPq4`. A person pastes the link into an application. In a browser, the server shows a page that tells the person to do this.

### Parts of the protocol

Some parts of the protocol are optional. The specification gives each optional part one word. The word has 2 to 20 lower-case letters and hyphens.

- A client tells the server which parts it supports.
- A server tells the client which parts it requires. If the client does not support a required part, the server refuses the sign-in.
- A requirement applies to the client of a person, not to a bot.

Protocol version 0 defines no parts at this time. Thus the two lists are empty.

A server cannot examine a client. A requirement stops a client that is honest but not complete. It is not a security control.

## Keys

Each key is an Ed25519 key (RFC 8032).

- A server has one key pair, the server key. The server makes it at the first start.
- A user has a secret seed of 32 random bytes.
- For each server, the client calculates an account key from the seed and the public server key. The server binds the name of the user to the public account key.

### How to calculate an account key

Use HKDF with SHA-256 (RFC 5869):

| Input | Value |
|---|---|
| Input key | The 32 bytes of the seed |
| Salt | None |
| Info | The ASCII text `post-account-key-1`, then one zero byte, then the 32 bytes of the public server key |
| Length | 32 bytes |

The result is the secret Ed25519 key of the account.

### Proofs

A proof is an Ed25519 signature. It shows that the sender has a secret key, and it is valid only for one connection.

The server proof is a signature of the server key. The server signs these parts, in this sequence:

1. The ASCII text `post-server-proof-0`.
2. One zero byte.
3. The address, as ASCII text.
4. One zero byte.
5. The random value of the client.
6. The random value of the server.

The account proof is a signature of the account key. The client signs these parts, in this sequence:

1. The ASCII text `post-account-proof-0`.
2. One zero byte.
3. The address, as ASCII text.
4. One zero byte.
5. The public server key.
6. The random value of the client.
7. The random value of the server.

The address is the address that the client used for the connection.

A receiver must refuse a signature that is not in its standard form. A receiver must also refuse a public key of small order.

## The sign-in

1. The client sends a `hello` message. It contains the address that the client used and a new random value.
2. The server answers with a `challenge` message. It contains the public server key, a new random value, and the server proof.
3. The client examines the server proof. If the proof is not valid, the client must close the connection. The client must not calculate or send an account key in this condition.
4. The client calculates the account key and sends an `identify` message with the account proof.
5. If the key has an account, the server answers with a `welcome` message. The session starts.
6. If the key has no account, the server answers with the error `unknown-account`. The client can then send a `register` message with a name and the code of an invitation.

A server signs only its own addresses. Thus a false server cannot show the key of a different server. The client then calculates a key that has no value for a different server.

## Invitations

A person makes an account with the code of an invitation (see the `register` message). An invitation is open until one of these conditions is true:

- Its end time is in the past.
- It made as many accounts as its limit permits.
- The server owner canceled it.

In protocol version 0, only the server owner makes, lists, and cancels invitations. The server answers a different account with the error `not-permitted`.

An owner link is a special invitation. The server program makes it, and a client cannot list it or cancel it.

## Messages from the client

### `hello`

The `hello` message starts the sign-in. It must be the first message on a connection.

| Field | Value |
|---|---|
| `type` | `"hello"` |
| `ref` | Optional. See "The `ref` field". |
| `protocol` | The protocol version of the client. This document describes version 0. |
| `address` | The address that the client used for the connection. |
| `nonce` | A random value that the client makes for this connection. |
| `supports` | The list of the optional parts that the client supports. |

The server answers with a `challenge` message or with an `error` message. After an error, the client can send a new `hello` message on the same connection.

### `identify`

The `identify` message signs in to an account that exists.

| Field | Value |
|---|---|
| `type` | `"identify"` |
| `ref` | Optional. |
| `key` | The public key of the account. |
| `proof` | The account proof. |
| `invitation` | Optional. The code of an owner link. It makes the account the server owner. The server ignores a different code. |

The server answers with a `welcome` message or with an `error` message.

### `register`

The `register` message makes an account and signs in to it.

| Field | Value |
|---|---|
| `type` | `"register"` |
| `ref` | Optional. |
| `key` | The public key of the new account. |
| `proof` | The account proof. |
| `name` | The name that the user wants. |
| `invitation` | The code of an invitation or of an owner link. An owner link makes the new account the server owner. |

The server answers with a `welcome` message or with an `error` message. After an error, the client can send a new `register` message on the same connection.


### `send`

The `send` message sends a text to a channel.

| Field | Value |
|---|---|
| `type` | `"send"` |
| `ref` | Optional. |
| `channel` | The name of the channel. |
| `text` | The text. |

The server answers with a `message` message or with an `error` message.

### `invite`

The `invite` message makes an invitation.

| Field | Value |
|---|---|
| `type` | `"invite"` |
| `ref` | Optional. |
| `lifetime` | Optional. The number of seconds until the invitation ends, from 1 to 4,294,967,295. Without this field, the invitation has no end. |
| `uses` | Optional. The number of accounts that the invitation can make, from 1 to 4,294,967,295. Without this field, the invitation has no limit. |

The server answers with an `invitations` message or with an `error` message. The new invitation is the first entry of the list.

### `invitations`

The `invitations` message asks for the open invitations of the server.

| Field | Value |
|---|---|
| `type` | `"invitations"` |
| `ref` | Optional. |

The server answers with an `invitations` message or with an `error` message.

### `revoke`

The `revoke` message cancels an invitation. A person cannot make an account with the invitation after that.

| Field | Value |
|---|---|
| `type` | `"revoke"` |
| `ref` | Optional. |
| `code` | The code of the invitation. |

The server answers with an `invitations` message or with an `error` message. If the invitation does not exist, or if it is not open, the error is `invalid-invitation`.

### `rename`

The `rename` message changes the name of the account.

| Field | Value |
|---|---|
| `type` | `"rename"` |
| `ref` | Optional. |
| `name` | The new name. |

The server answers with a `rename` message or with an `error` message.

- The earlier name stays reserved for the account for 30 days. In this time, no other account can take it.
- An account can change its name one time in 30 days. Before the end of this time, the error is `too-soon`.

The two times are fixed in protocol version 0.

## Messages from the server

### `challenge`

The `challenge` message answers a `hello` message.

| Field | Value |
|---|---|
| `type` | `"challenge"` |
| `ref` | The value from the `hello` message, if it had one. |
| `protocol` | The protocol version of the session. |
| `key` | The public server key. |
| `nonce` | A random value that the server makes for this connection. |
| `proof` | The server proof. |
| `requires` | The list of the optional parts that a client must support. |

### `welcome`

The `welcome` message starts the session of a user. The session now has a name.

| Field | Value |
|---|---|
| `type` | `"welcome"` |
| `ref` | The value from the message that the server accepts, if it had one. |
| `name` | The name of the user in this session. |
| `role` | The role of the user on this server: `"owner"` or `"member"`. |

After the `welcome` message, the server puts the user into the channel `lobby`. The server sends a `join` message to each user in the channel, also to the new user.

One account can have more than one connection at the same time. The server sends the `join` message only for the first connection of an account, and the `leave` message only for the last one.

### `join`

The `join` message tells that a user came into a channel.

| Field | Value |
|---|---|
| `type` | `"join"` |
| `channel` | The name of the channel. |
| `name` | The name of the user. |
| `time` | The time when the user came in. |

### `leave`

The `leave` message tells that a user went out of a channel. At this time, this occurs when the last connection of the user closes.

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

### `invitations`

The `invitations` message gives the open invitations of the server. The newest invitation is first.

| Field | Value |
|---|---|
| `type` | `"invitations"` |
| `ref` | The value from the request, if it had one. |
| `list` | The list of the open invitations. |

Each entry of the list is an object with these fields:

| Field | Value |
|---|---|
| `code` | The code of the invitation. |
| `link` | The invite link. A person pastes it into an application. |
| `created` | The time when the server made the invitation. |
| `end` | Optional. The time when the invitation ends. |
| `uses` | Optional. The number of accounts that the invitation can make. |
| `used` | The number of accounts that the invitation made. |

### `rename`

The `rename` message tells that a user has a new name. The server sends it to each user in the channel, also to the user with the new name.

| Field | Value |
|---|---|
| `type` | `"rename"` |
| `ref` | The value from the `rename` message of the client, if it had one. Only the connection that sent that message gets this field. |
| `old` | The earlier name of the user. |
| `new` | The new name of the user. |
| `time` | The time of the change. |

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
| `hello-required` | The client sent a different message before the `hello` message. |
| `duplicate-hello` | The client sent a second `hello` message. |
| `wrong-address` | The server does not answer to the address in the `hello` message. |
| `identify-required` | The client sent a message that is only for a user with a session. |
| `duplicate-identify` | The client sent an `identify` or a `register` message after a `welcome` message. |
| `invalid-proof` | The account proof does not agree with the key and this connection. |
| `unknown-account` | The server has no account for the key. |
| `account-exists` | The key in a `register` message already has an account. |
| `invalid-invitation` | The invitation does not exist, or it is not open. |
| `missing-part` | The client does not support a part of the protocol that the server requires. |
| `invalid-name` | The name does not obey the rules for a name. |
| `name-taken` | A different user has the name, or the name is reserved. |
| `no-such-channel` | The channel does not exist. |
| `empty-text` | The text has no visible character. |
| `text-too-long` | The text has more than 4,000 characters. |
| `not-permitted` | The account does not have the permission for the request. |
| `too-soon` | The account changed its name a short time ago. |
| `server-error` | The server has a problem and cannot do the task. |

## Example conversations

Each file in the [examples](examples) directory shows one connection.

- A line that starts with `C: ` is a message from the client.
- A line that starts with `S: ` is a message from the server.
- A line that starts with `#` is a comment.

| File | Content |
|---|---|
| [sign-in.txt](examples/sign-in.txt) | A person makes an account with an invitation. The keys and the signatures are real, and a test examines them. |
| [sign-in-errors.txt](examples/sign-in-errors.txt) | The server refuses messages of the sign-in. |
| [requirements.txt](examples/requirements.txt) | A server requires a part of the protocol that the client does not support. |
| [invitations.txt](examples/invitations.txt) | The server owner makes, lists, and cancels invitations. |
| [rename.txt](examples/rename.txt) | A user changes the name of the account. |
| [storage-problem.txt](examples/storage-problem.txt) | The server has a problem with its storage. |
| [two-users.txt](examples/two-users.txt) | Two users exchange text. |
| [errors.txt](examples/errors.txt) | The server refuses other messages. |
