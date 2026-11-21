# Architecture

This document describes the parts of the Post code in version 0.1. The [decision records](decisions/README.md) give the reasons for the design.

## Crates

The repository is one Cargo workspace (decision record D-097). Each crate depends only on the crates below it in this table.

| Crate | Content | Depends on |
|---|---|---|
| `post-app` | The application with its window. The program has the name `post`. | `post-client`, `post-proto` |
| `post-client` | The client logic without an interface | `post-proto` |
| `post-server` | The server. The program has the name `post-server`. | `post-proto` |
| `post-proto` | Message types, codec, and protocol rules | No other crate of Post |

The `post-crypto` crate starts in version 0.2, when Post gets its first key.

## How a message travels

1. The user types a text and presses the Enter key.
2. The application makes a `send` message and gives it to the connection.
3. The connection writes the message as one JSON line on the WebSocket.
4. The server reads the line and examines the rules.
5. The server makes a `message` message for each user in the channel, also for the sender.
6. Each application reads its line and adds an entry to its timeline.
7. The window shows the new entry.

The sender sees the message only when it comes back from the server. Thus all users see the same sequence.

## The protocol crate

`post-proto` contains the rules that the client and the server share.

- `name` and `text` hold the rules for a name and for the text of a message.
- `time` holds the time type. It writes and reads the one time form of the protocol.
- `message` holds the message types. The doc comments on these types go into the JSON Schemas.
- `codec` writes a message as one line and reads a line.
- `schema` makes the JSON Schemas from the message types.

A test compares the schemas and the example conversations in `docs/protocol` with the code. The check fails if they do not agree.

## The server crate

- `hub` decides who gets which message. It has no network code and no clock. Each function gets the present time as a value, thus a test controls all inputs.
- `sessions` connects the hub to one queue for each connection. If a queue is full, the server closes that connection.
- `server` holds the network code. It accepts WebSocket connections on the loopback addresses and moves lines between a socket and its queue.
- `cli` reads the arguments of the program.

The server keeps all state in memory in version 0.1.

## The client crate

- `address` reads a server address. In version 0.1, it accepts only an address of the local computer.
- `connection` opens the WebSocket, sends the `hello` message, and gives the events of the connection as a stream.
- `session` holds the timeline of one connection and makes the `send` messages.

## The application crate

The application uses the iced toolkit (D-077).

- `app` holds the state of the application and the update logic. It does not use the toolkit. An update returns an effect, for example "put the focus in the message field".
- `timeline` holds the rules that prepare a timeline entry for the window: the local time and the groups of messages.
- `palette` holds the colors of light mode and dark mode. A test examines the contrast of each color.
- `i18n` holds the texts of the interface. The catalogs for English and Latin American Spanish are in `crates/post-app/i18n`.
- `view` makes the widgets from the state. It contains no decisions.
- `shell` connects the state to the toolkit: the start, the updates, and the subscriptions.
- `main` selects the language and starts the shell.

## Tests

- Each module has unit tests. We write a test before the code that it examines (D-101).
- The server and the client have tests that use a real WebSocket connection on the local computer.
- The application has tests that operate the interface without a window. They type text and select buttons.
- One test runs the complete application without a window. It connects to a real server and exchanges text with a second user.
- The automatic checks measure the coverage of each crate (D-104). The files `view.rs` and `main.rs` do not count.
- Mutation tests examine the protocol crate each night.
