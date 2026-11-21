# Engineering decisions

These records tell you how we build Post. Read the [guide](README.md) first.

### D-097 Repository layout

**Decision.** The repository is one Cargo workspace with these crates:

| Crate | Content |
|---|---|
| `post-proto` | Message types, codec, and protocol rules |
| `post-crypto` | Identity keys and sealed chats |
| `post-server` | The server program |
| `post-client` | The client logic without an interface, and the local store |
| `post-app` | The iced application |

**Reason.** A protocol change goes into the client and the server in one commit. One set of automatic checks examines all code.

**Not selected.**

- A separate repository for the server.
- One crate for all code.

### D-098 Build sequence

**Decision.** We first build a thin slice that works from end to end: server, client logic, and a simple window. Then we add one function at a time through all layers, each with its tests.

**Reason.** There is always an application that runs. We find problems between the layers early.

**Not selected.**

- Protocol and server first, interface last.
- Interface first, with false data.

### D-099 First slice

**Decision.** Version 0.1 is the thinnest slice. A user types a temporary name and a server address. The user arrives in one fixed channel, `#lobby`, and exchanges text with a different user in real time. The connection is a WebSocket without TLS, to the local computer only. The server keeps its state in memory.

**Reason.** The first demonstration comes early. Accounts, TLS, and storage come in the next versions, each one done fully.

**Not selected.**

- A slice with storage.
- A slice with TLS and keys.

### D-100 Roadmap

**Decision.** The versions come in this sequence:

| Version | Content |
|---|---|
| 0.1 | Slice: two users chat in one channel |
| 0.2 | Identity, handles, invitations, TLS, application profiles, protocol console |
| 0.3 | Channels, members, presence, split panes, sidebar favorites |
| 0.4 | History, unread counts, read position, drafts |
| 0.5 | Direct messages |
| 0.6 | Replies, reactions, edit, delete, mentions, keyword highlights, mentions view, text format |
| 0.7 | Files and images |
| 0.8 | Search, notifications, message links, saved messages, export |
| 0.9 | Moderation tools, administration in the application, server settings and limits |
| 0.10 | Sealed chats |
| 0.11 | Recovery phrase, device link, keychain, passphrase, application lock, account proof |
| 0.12 | Voice: transport, relay, and sound path |
| 0.13 | Voice: interface, moderation, and sealed voice |
| 0.14 | Bots with slash commands, webhooks, link previews |
| 0.15 | Custom themes, key bindings |
| 0.16 | Automatic certificates, proxy, packages, server directory, operator tools |
| 1.0 | Final corrections, code signature, release |

The public web view is an open decision (D-032). If the project selects it, it goes into version 0.15.

**Reason.** The base comes first. Voice is a primary function and the largest risk, thus it comes directly after the text functions and the sealed chats. If time becomes short, a smaller function moves, not voice.

**Not selected.**

- The smaller server functions before voice.
- A thin voice slice directly after direct messages.

### D-101 Tests first

**Decision.** We write a test before the code that it examines. A commit contains the test and the code together, and all tests pass.

**Reason.** Each function has a test from its first day. The `main` branch always works.

### D-102 Automatic checks

**Decision.** These checks protect the quality:

- `rustfmt` examines the format.
- `clippy` runs with the `all` and `pedantic` groups. Each warning is an error. A short list of exceptions gives a reason for each one.
- Our crates forbid `unsafe` code.
- All tests run on Windows, macOS, and Linux.
- `cargo deny` examines licenses and security advisories.
- A coverage minimum applies (D-104).
- Mutation tests examine the protocol and cryptography crates.
- Each parser has a fuzz target.
- A git hook in the repository runs the format check, clippy, and the tests before each commit.

**Reason.** The checks stop a decrease in quality before it gets into the `main` branch.

**Not selected.**

- A smaller set without coverage, mutation tests, and fuzz targets.

### D-103 When the checks run

**Decision.** Each push runs the fast checks: format, clippy, tests on three systems, `cargo deny`, and coverage. Mutation tests and long fuzz runs start each night and before each minor version.

**Reason.** A push gets a result quickly. The slow checks still run each day.

**Not selected.**

- All checks on each push.
- Slow checks only before a release.

### D-104 Coverage

**Decision.** The automatic checks fail if line coverage is below 90% in the protocol, cryptography, server, and client crates. In the application crate, the state and update logic count. Pure view code does not count.

**Reason.** Tests of widget layout have little value. All logic has tests.

**Not selected.**

- A minimum of 95%.
- A minimum of 100% with a list of exceptions.

### D-105 Performance

**Decision.** We write performance budgets, for example:

- The window shows in less than 300 ms.
- The timeline scrolls smoothly through 100,000 messages.
- The application uses little processor time and memory when idle.
- The server sends a message to all members of a channel quickly.

Benchmarks run in the automatic checks, and we record the results. A clear decrease in performance stops the next version.

**Reason.** Speed is a property that the project must protect, as it protects correct operation.

**Not selected.**

- A check that fails on each push. Shared test computers give unstable results.
- No written budgets.

### D-106 Comments

**Decision.** The code has few comments. A comment is correct only where the code cannot give the information. Doc comments are necessary on the message types of the protocol crate, because the build puts them into the JSON Schemas. Source files have no license header.

**Reason.** Good names and types give the meaning. A comment that repeats the code becomes wrong with time.

**Not selected.**

- A doc comment on each public item.
- No doc comments at all.

### D-107 Documents

**Decision.** The repository contains these documents:

- README, CONTRIBUTING, SECURITY, and the code of conduct.
- The protocol specification in `docs/protocol/`.
- An architecture description in `docs/architecture.md`.
- The design philosophy and the decision records.

All documents obey ASD-STE100 Simplified Technical English. A change to the code and the related change to a document go into the same commit.

**Reason.** Simple English is clear for all readers, also for a reader with a different first language. Documents that change with the code stay correct.

**Not selected.**

- A README and a protocol specification only.
- A website with guides from the start.

### D-108 Commits

**Decision.** We make many small commits. Each commit message is one line in the imperative, for example "Add hello and welcome messages". A commit message has no co-author line. Each commit goes directly to the `main` branch, and we push it immediately.

**Reason.** A small commit is easy to examine and to undo. One branch is the simplest procedure for one developer.

**Not selected.**

- Conventional Commits.
- A branch and a pull request for each function.

### D-109 Version numbers

**Decision.** All crates have the same version number. The patch number increases when a correction or a small improvement is complete. The minor number increases when a function is complete. Version 1.0.0 comes when the scope of D-003 is complete. Each increase is a separate commit and adds an entry to `CHANGELOG.md`. Each minor version gets a git tag.

**Reason.** One number tells the state of the full project.

**Not selected.**

- An increase in each commit.
- A different number for each crate.

### D-110 Dependencies

**Decision.** We use few dependencies and examine each new one. Before each minor version, we update the dependencies manually and examine the changes. `cargo deny` tells us about security advisories each night. No bot makes commits.

**Reason.** Each dependency is a risk. With a manual update, each commit comes from the developer.

**Not selected.**

- Pull requests from an update bot.
- Updates only when a problem occurs.

### D-111 Toolchain

**Decision.** The file `rust-toolchain.toml` sets one stable Rust release. We change it in a separate commit soon after each new stable release.

**Reason.** The local computer and the automatic checks use the same compiler and the same lints. A new Rust release cannot break the `main` branch.

### D-112 Work procedure

**Decision.** Before each minor version, a round of questions sets the details of that version. Each question gives a recommended option. Then we build the version from end to end, increase the version number, add the tag, and make a report.

**Reason.** Decisions come before the work, and each version gets a review.

**Not selected.**

- A review after each function.
- Work without a stop until a problem occurs.

### D-157 Network libraries

**Date.** 2026-11-21.

**Decision.** Post uses these libraries for its network code:

- `tokio` is the async runtime of the server and of the application.
- `axum` gives the server HTTP and WebSocket on one port.
- `tokio-tungstenite` gives the client its WebSocket connection.

**Reason.** The libraries are well known, and the tokio project maintains `tokio` and `axum`. Later versions need HTTP and WebSocket on one port for files, invite pages, and webhooks (D-047).

**Not selected.**

- `tokio-tungstenite` on the two sides, with HTTP routes that we write directly on `hyper`. This option has fewer dependencies, but we must test and maintain more HTTP code.

### D-158 Details of the first slice

**Date.** 2026-11-21.

**Decision.** These rules apply to version 0.1 (D-099):

- The temporary name obeys the rules for a handle (D-023). The server refuses a name that a connected user already has.
- The server listens only on the loopback addresses of the computer. The default port is 7678. On a telephone keypad, these digits spell "POST".
- The application refuses a server address that is not the local computer.
- The server sends each message to all people in the channel, also to the sender.
- The timeline shows a line when a person joins or leaves (D-124).
- If the connection stops, the application goes back to the connect screen and tells the user. It does not connect again automatically (D-093 comes later).
- A message has no identifier. Identifiers come with history in version 0.4.
- Emoji come from the fonts of the operating system. The emoji font (D-085) comes in a later version.
- The `post-crypto` crate (D-097) starts in version 0.2, when Post gets its first key.

**Reason.** The slice stays thin. Data without encryption never goes across a network. Each rule is the final rule, or it is clearly temporary.

### D-159 Program names

**Date.** 2026-11-21.

**Decision.** The program of the application has the name `post`. The program of the server has the name `post-server`.

**Reason.** The names are short and clear.

**Open point.** Some systems contain a Perl tool with the name `POST`. We must examine a possible conflict on macOS before the first macOS package.
