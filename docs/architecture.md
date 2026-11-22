# Architecture

This document describes the parts of the Post code. The project changes it together with the code. The [decision records](decisions/README.md) give the reasons for the design.

## Crates

The repository is one Cargo workspace (decision record D-097). Each crate depends only on the crates below it in this table.

| Crate | Content | Depends on |
|---|---|---|
| `post-app` | The application with its window. The program has the name `post`. | `post-client`, `post-proto` |
| `post-client` | The client logic without an interface | `post-crypto`, `post-proto` |
| `post-server` | The server. The program has the name `post-server`. | `post-crypto`, `post-proto`, `post-rules` |
| `post-crypto` | Keys and proofs of accounts and servers | `post-proto` |
| `post-rules` | The proved rules of a server for accounts, handles, and invitations | `post-proto` |
| `post-proto` | Message types, codec, and protocol rules | No other crate of Post |
| `post-testing` | Helpers for the tests | No other crate of Post |

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
- `address` holds the form of a server address that the client and the server share.
- `bytes`, `code`, and `part` hold the text forms of keys, signatures, invitation codes, and the words for parts of the protocol.
- `time` holds the time type. It writes and reads the one time form of the protocol.
- `base64` writes and reads the base64url text of keys, random values, and signatures (D-179).
- `ascii` holds the classes of characters that the rules use.
- `message` holds the message types. The doc comments on these types go into the JSON Schemas.
- `codec` writes a message as one line and reads a line.
- `schema` makes the JSON Schemas from the message types.

A test compares the schemas and the example conversations in `docs/protocol` with the code. The check fails if they do not agree.

Verus proves `base64` and the rules of `name`, `code`, `part`, and `time`. The [verification](verification.md) document tells what each proof shows.

## The cryptography crate

`post-crypto` uses libraries for all cryptography (D-168). It contains no algorithm of its own.

- `seed` holds the secret seed of a profile. It calculates the account key for a server, and it writes and reads the text form of the seed.
- `keys` holds the server key and the account key. It makes and examines the proofs of the sign-in.
- `transcript` writes the bytes that a proof of the sign-in signs.
- `hex` writes and reads the hexadecimal text of the seed.
- `random` makes random values and invitation codes.

A test examines the proofs in the example conversations of the protocol specification.

Verus proves `transcript`, `hex`, the text form of the seed, and the generator of invitation codes in `random` (see the [verification](verification.md) document).

## The rules crate

`post-rules` is the rules core of the server (D-178). It has no input and no output, and Verus proves it (see the [verification](verification.md) document).

- `row` holds the rows of the data: an account, an invitation, and a reserved name. It also holds the changes that the core gives to the storage.
- `rule` holds the rules as specifications, and the small decisions about time.
- `rules` holds the state and one function for each operation. An operation examines the rules, gives the changes to the storage, and only then changes the state.


## The server crate

- `hub` does the sign-in and decides who gets which message. It asks the rules core for each decision about an account or an invitation. It has no network code and no clock. Each function gets the present time as a value, and a test can give it fixed random values. Thus a test controls all inputs.
- `store` keeps the server key in SQLite (D-051). It writes the changes of the rules core, and it loads the rows when the server starts. It makes no decisions. A lock file lets only one program use the data. The other modules do not use SQL.
- `config` reads the configuration file of the server. Without a file, the server operates in development mode on the local computer.
- `sessions` connects the hub to one queue for each connection. If a queue is full, the server closes that connection.
- `server` holds the network code. It accepts WebSocket connections on the loopback addresses and moves lines between a socket and its queue.
- `cli` reads the arguments of the program. The command `owner-link` makes a new owner link (D-172). The operator stops the server before this command.

The server keeps its storage in a data directory. In development mode, the directory has the name `post-server-data`. At this time, the server cannot use TLS.

## The client crate

- `destination` reads an invite link or a server address. It decides if the connection uses TLS.
- `home` finds the directories of the user for the settings and for the data.
- `profile` keeps the profiles in files (D-147). A profile has a name, a seed, and a list of the servers that it knows. A lock file lets only one window use a profile.
- `connection` opens the WebSocket, does the sign-in, and gives the events of the connection as a stream. It examines the proof of the server before it calculates the account key.
- `session` holds the timeline of one connection and makes the `send` messages.
- `address` reads a server address for the temporary `guest` message. It goes away together with that message.

## The application crate

The application uses the iced toolkit (D-077).

- `app` holds the state of the application and the update logic. It does not use the toolkit, it reads no file, and it holds no secret. An update returns an effect, for example "put the focus in the message field".
- `timeline` holds the rules that prepare a timeline entry for the window: the local time and the groups of messages.
- `palette` holds the colors of light mode and dark mode. A test examines the contrast of each color.
- `i18n` holds the texts of the interface. The catalogs for English and Latin American Spanish are in `crates/post-app/i18n`.
- `view` makes the widgets from the state. It contains no decisions.
- `shell` connects the state to the toolkit: the start, the updates, and the subscriptions. It opens the profile, gives the seed to the connection, and writes the known servers.
- `main` selects the language, finds the directories of the user, and starts the shell.

## The website

The website is not a part of the product (D-175).

- The `site` directory holds the home page in English and in Spanish, the page layout for documents, the styles, the fonts, and the icon.
- The program `post-site` in `tools/site` makes the website. It copies the files of the `site` directory. It also makes one web page from each document of the repository.
- In a document page, each decision record number is a link to that record.
- A link to a file that is not a document goes to the repository on GitHub.

A test makes the website from the repository and examines each link. The same test fails if a page loads content from a different server.

## Tests

- Each module has unit tests. We write a test before the code that it examines (D-101).
- The server and the client have tests that use a real WebSocket connection on the local computer.
- The application has tests that operate the interface without a window. They type text and select buttons.
- Some tests run the complete application without a window. One of them makes an account on a real server with an invite link and exchanges text with a second user.
- The automatic checks measure the coverage of each crate (D-104). The files `view.rs` and `main.rs` do not count.
- Mutation tests examine the protocol crate each night.
