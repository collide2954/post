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

The `tools` directory holds the scripts of the automatic checks and the program that makes the website (D-175).

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
| 0.2 | Identity, handles, handle change, invitations, TLS, application profiles, first settings screen, text size, protocol console |
| 0.3 | Channels, members, presence, split panes, sidebar favorites |
| 0.4 | History, unread counts, read position, drafts |
| 0.5 | Direct messages |
| 0.6 | Replies, reactions, edit, delete, mentions, keyword highlights, mentions view, text format |
| 0.7 | Files and images |
| 0.8 | Search, notifications, message links, saved messages, export |
| 0.9 | Moderation tools, administration in the application, server settings and limits, join policies, client requirements |
| 0.10 | Sealed chats |
| 0.11 | Recovery phrase, device link, keychain, passphrase, application lock, account proof |
| 0.12 | Voice: transport, relay, and sound path |
| 0.13 | Voice: interface, moderation, and sealed voice |
| 0.14 | Bots with slash commands, webhooks, link previews |
| 0.15 | Custom themes, key bindings, appearance settings |
| 0.16 | Automatic certificates, proxy, packages, links that open Post, server directory, operator tools |
| 1.0 | Final corrections, code signature, release |

The public web view is an open decision (D-032). If the project selects it, it goes into version 0.15.

**Reason.** The base comes first. Voice is a primary function and the largest risk, thus it comes directly after the text functions and the sealed chats. If time becomes short, a smaller function moves, not voice.

**Not selected.**

- The smaller server functions before voice.
- A thin voice slice directly after direct messages.

**History.** On 2026-11-21, the question round for version 0.2 gave a version to functions that had none (D-167, D-173). The handle change, the first settings screen, and the text size went into version 0.2. The join policies went into version 0.9, the other appearance settings into version 0.15, and the links that open Post into version 0.16. On the same day, the client requirements went into version 0.9 (D-176).

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
- Verus proves the code that has a specification (D-177).

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
- A threat model in `docs/threat-model.md` (D-075).
- The design philosophy and the decision records.

All documents obey ASD-STE100 Simplified Technical English. A change to the code and the related change to a document go into the same commit. The website shows the documents as web pages (D-175).

**Reason.** Simple English is clear for all readers, also for a reader with a different first language. Documents that change with the code stay correct.

**Not selected.**

- A README and a protocol specification only.
- A website with guides from the start.

**History.** On 2026-11-21, the project added a website that shows the documents (D-175). The website has no separate guides.

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

**Decision.** The file `rust-toolchain.toml` sets one stable Rust release. We change it in a separate commit soon after each new stable release. The proofs use the Rust release that Verus sets (D-177), thus the proved code must also build with that release.

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

### D-168 Libraries for keys, TLS, and storage

**Date.** 2026-11-21.

**Decision.** Post uses these libraries:

- `rustls` gives TLS to the server and to the application. It uses its default provider, `aws-lc-rs`. This provider adds a post-quantum key exchange.
- `rustls-platform-verifier` lets the application examine a certificate with the certificate check of the operating system.
- `rcgen` makes the own certificate of the server (D-164).
- `ed25519-dalek` and `hkdf` from the RustCrypto project make and examine the keys of accounts and servers.
- `rusqlite` gives the server its SQLite storage (D-051). SQLite is a part of the server program.

**Reason.** Experts examined these libraries, and many projects use them (D-075). The standard provider of OpenMLS uses the same RustCrypto family (D-068). The certificate check of the operating system also accepts a certificate authority that a company installed.

**Not selected.**

- `ring` for TLS and for the keys. It has fewer crates and a simpler build, but TLS then has no post-quantum key exchange.
- `sqlx` for storage. It brings many more crates. SQLite has one writer, thus async queries help little.

### D-170 Performance budgets

**Date.** 2026-11-21.

**Decision.** These budgets apply (D-105):

| Subject | Budget |
|---|---|
| Start of the application | The window shows in 300 ms or less after a warm start. |
| Memory of the application | The application uses 150 MB or less when idle. |
| Processor time of the application | The application uses 1% of one processor core or less when idle. |
| Timeline, from version 0.4 | A frame takes 16 ms or less with 100,000 messages. |
| Message delivery | The server sends a message to 1,000 members of a channel in 50 ms or less. |
| Server memory | 10,000 idle connections use 1 GB or less. |

The reference computer is a laptop with an Intel Core i5-8365U processor and built-in graphics. Before each minor version, we measure on the reference computer and record the results in `docs/performance.md`. The nightly checks run the benchmarks that need no window.

**Reason.** The budgets are near the measured values of version 0.1: a window in 50 ms to 80 ms, 120 MB of memory, and less than 1% of one core. Thus a decrease in performance shows early. A reference computer of average speed gives stable numbers.

**Not selected.**

- Strict budgets of 150 ms and 80 MB. The GPU renderer of the toolkit uses more than 80 MB today.
- Benchmarks with no budgets until version 0.4.

### D-173 Content of version 0.2

**Date.** 2026-11-21.

**Decision.** Version 0.2 contains the functions that the roadmap gives (D-100). These rules also apply:

- The handle change (D-024) is in version 0.2.
- The first Settings screen and the text size are in version 0.2 (D-167).
- The sign-in messages carry the parts that a client supports and the parts that a server requires (D-176). The two lists are empty in version 0.2.
- A server has only the join policy "invitation". The other two policies (D-055) come in version 0.9 with the server settings.
- In a browser, the page of an invite link shows the link and a Copy button. It tells the person to paste the link into the application, and it has a link to the downloads. The page has English and Latin American Spanish text.
- The button "Open in Post" (D-050) comes with the packages in version 0.16. Only an installed package can register the application for links on all three systems.

**Reason.** A function that comes early is a part of each later function from its first day. A function that depends on a later version waits for that version.

**Not selected.**

- The handle change in version 0.4 or in version 0.9.
- Open sign-up in version 0.2, with one line in the configuration file.
- No page for an invite link before version 0.16.
- A registration for links in version 0.2. It is not possible on macOS without a package.

### D-174 Details of version 0.2

**Date.** 2026-11-21.

**Decision.** These rules apply to version 0.2.

Keys and sign-in (D-163):

- A key is an Ed25519 key. The seed has 32 random bytes. HKDF with SHA-256 calculates an account key from the seed and the public server key.
- The protocol writes keys, signatures, and random values as base64url text. The text has no `=` characters at its end.
- A sign-in has four messages: `hello`, `challenge`, `identify` or `register`, and `welcome`. The protocol specification gives them.
- One account can have more than one connection at the same time.

Addresses:

- An address without a port means port 443 with TLS (D-047).
- An address of the local computer without a port means port 7678 without TLS, for development (D-073). The prefix `https://` selects TLS for such an address.

Server:

- Without a configuration file, the server operates in development mode. It listens only on the local computer, without TLS.
- The configuration file has the name `post-server.toml`. It gives the public addresses, the listen addresses, the data directory, and the TLS mode. The TLS modes are the own certificate, certificate files, and a reverse proxy.
- The server accepts only TLS 1.3. The own certificate is valid for 10 years. The application identifies a private certificate by its SHA-256 fingerprint.
- The code of an invitation has 12 random letters and digits. An owner link is an invitation that makes its user the server owner.
- Only the server owner makes invitations. Operators and the setting for members (D-056) come in version 0.9.
- A request for a new invitation gives the time to its end in seconds. The server calculates the end time, thus the clock of the client has no effect.
- The server answers each invitation request with the full list of the open invitations.
- The limits of the handle change (D-024) are fixed. The operator settings for them come in version 0.9.
- The server tells each user in the channel about a handle change. The application then shows the new handle in the earlier lines of that user.
- A line in the timeline for a person who joins or leaves still shows the start or the end of a connection. Channels and members come in version 0.3.
- The server stores accounts, handles, and invitations. It stores no messages before version 0.4.

Application:

- The profiles are in the data directory of the user: `%APPDATA%\Post` on Windows, `~/Library/Application Support/Post` on macOS, and the XDG directories on Linux. The environment variable `POST_HOME` selects a different directory.
- A profile name has 1 to 30 characters. The first profile has the name "main", or "principal" in Spanish.
- The user types a name only for a new account. For an account that exists, the application ignores the name.
- The application remembers the key of each server. If a known address shows a different key, the application does not sign in.
- If the application cannot open the profile, the window shows the cause, and the application does nothing more.
- When the application changes a value in the settings file, it keeps the comments and the sequence of the file.
- The F12 key shows and hides the protocol console when the developer setting is on.

**Reason.** Each rule is the final rule, or it is clearly temporary. The rules keep the version as small as the decisions permit.

### D-177 Formal verification

**Date.** 2026-11-21.

**Decision.** The project proves parts of the code with Verus. Verus is the only tool for formal verification in the project.

- We take the proofs as far as the tool permits. A proof applies to all inputs, not to examples.
- New code that has no input and no output gets a specification and a proof.
- Where Verus cannot work, the code has a hole. A hole is code or an assumption that has no proof.
- The document `docs/verification.md` tells what each proof shows, and it names each hole.
- The proofs run before each commit, if the computer has Verus. They also run in the automatic checks on each push.
- The documents tell exactly what a proof shows. They never say that Post as a whole is verified.
- The version of Verus and the version of its library `vstd` are fixed. We change them together.

These holes stay: the async network code, TLS, SQLite, the window toolkit, JSON, files, and the cryptographic algorithms. The security of the sign-in against an attacker in the network also has no proof, because Verus does not analyze cryptographic protocols.

**Reason.** A test examines some inputs. A proof examines all inputs. Verus proves the Rust code itself, thus a proof cannot disagree with the program. It operates on the three systems of the project.

Verus is under active development, and its makers say that functions can be broken or missing. The project accepts this risk. The normal build does not use Verus, thus the program can always be built without it.

**History.** On 2026-11-21, the project selected the sequence of the work. The build of version 0.2 continues, and each new part with no input and no output gets its proof in the same step. The rules of the server get a proved core (D-178). The text form of keys gets its own proved code (D-179).

**Not selected.**

- ProVerif for the sign-in, Kani for the parsers, and TLA+ for the server rules.
- A trial of Verus before the decision.
- No formal verification.

### D-178 Proved rules core of the server

**Date.** 2026-11-21.

**Decision.** The server keeps its accounts, handles, and invitations in memory, in a part that has no input and no output. This part is the rules core. The core makes each decision about an account, a handle, or an invitation. Verus proves its rules (D-177).

- A change has three steps. The core calculates the new state and the list of changes. The server writes the changes to SQLite in one transaction. Only then does the new state replace the old state.
- SQLite stores the data (D-051). It makes no decisions.
- At the start, the server loads the data and examines it against the rules. If the data does not obey the rules, the server does not start.
- Only one program uses the data of a server at one time. A lock file stops a second program.
- The first form of the core keeps its rows in lists and examines them in sequence. An index comes when a measurement shows the need.
- The messages of the channels are not a part of the core.

As a minimum, Verus proves these rules:

- A key has a maximum of one account.
- A handle has a maximum of one owner.
- A reserved handle goes only to the account that had it, until the reserved time ends.
- An invitation is not used more times than its limit, and it is not used after its end.
- A handle change obeys the minimum interval.

**Reason.** Verus cannot prove SQL. Rules in Rust with no input and no output can have a proof. They are also faster, because a sign-in needs no query. The server writes a change before it uses the change, thus the memory and the file agree after a failure.

**Not selected.**

- Proved functions for the small decisions only. SQLite then keeps the rules that connect many rows, and these rules have no proof.
- Tests only for the rules of the server.

### D-179 Own code for the text form of keys

**Date.** 2026-11-21.

**Decision.** The protocol crate contains its own code for the text form of keys, random values, and signatures. Verus proves this code (D-177). The project does not use the library `base64`.

The proof shows two properties. The reader gives back each value that the writer writes. Each value has only one text.

**Reason.** The text form is small, and each sign-in uses it. With our own code, the text form has a proof, and the project has one dependency less. The values are public, thus the code needs no protection against an attacker who measures time.

**Not selected.**

- The library `base64`. Many projects use it, but the text form of keys then has no proof.