# Post

> Post is a modern chat app in the spirit of IRC: simple, open, self-hosted, with optional end-to-end encryption.

Post is between IRC and Telegram. It keeps the simplicity of IRC, and it adds the functions that most people use each day. The [design philosophy](docs/philosophy.md) gives the ideas behind Post.

The website of the project is [collide2954.github.io/post](https://collide2954.github.io/post/).

![The chat screen of Post 0.2 in light mode](docs/images/chat-light.png)

## State of the project

Post is at the start of its development. Do not use it for real conversations.

Version 0.2 adds identity and the connection across a network:

- Each user has an account on each server. The account is a key, and it has no password. The application calculates the key from a secret seed in a profile.
- A person joins a server with an invite link. The first person uses the owner link and becomes the server owner.
- A connection across a network uses TLS 1.3. A server can make its own certificate.
- A user can change the handle.
- The application has profiles, a Settings page, a text size from 80% to 200%, and a protocol console for developers.
- The application has English and Latin American Spanish text, light mode, and dark mode.

Version 0.2 has one channel, `#lobby`, and no stored history. The server can read each message. The [roadmap](docs/decisions/engineering.md) gives the sequence of the next versions (record D-100).

## Try version 0.2

You must have [rustup](https://rustup.rs). It installs the Rust release that the file `rust-toolchain.toml` gives.

1. Start the server:

   ```
   cargo run --bin post-server
   ```

   At the first start, the server prints an owner link.

2. In a second terminal, start the application:

   ```
   cargo run --bin post
   ```

3. Paste the owner link into the first field. Type a name, and select **Connect**. You are now the owner of the server.
4. Write a message.

At the next start, the application connects to this server again. Without a configuration file, the server listens only on your computer and uses no TLS.

The application keeps a secret seed in a file in your user directory. Only your user account can read the file, and the seed has no other protection at this time (record D-169).

The server listens on port 7678. To use a different port, give the number: `cargo run --bin post-server -- --port 9000`. The server keeps its data in the directory `post-server-data`.

### A second user on the same computer

One profile has one account on a server. Thus a second user needs a second profile.

1. In the chat, select **Invitations**, then **New invite link**, then **Copy**.
2. Select **Back**, then **Leave**, then **Profiles**. Make a profile with the name `second`.
3. Select **Back**, and open your server again.
4. In a third terminal, start a second window with the new profile:

   ```
   cargo run --bin post -- --profile second
   ```

5. Paste the invite link into the first field. Type a name, and select **Connect**.

### A server for other computers

Make a file with the name `post-server.toml` in the directory where you start the server:

```toml
# The address that the users type. An address without a port uses port 443.
address = "chat.example.org"

[tls]
# "own": the server makes its own certificate.
# "files": the server uses the two files below.
# "proxy": a reverse proxy does the TLS work.
mode = "own"
# certificate = "certificate.pem"
# key = "certificate-key.pem"
```

With the mode `own`, the server prints the fingerprint of its certificate. Give the fingerprint to your users. The application shows the fingerprint at the first connection, and the user compares the two (record D-073).

The command `post-server owner-link` makes a new owner link. Stop the server before you use this command.

## Documents

| Document | Content |
|---|---|
| [Design philosophy](docs/philosophy.md) | The ideas behind Post |
| [Decision records](docs/decisions/README.md) | Each design decision and its reason |
| [Protocol specification](docs/protocol/README.md) | The messages between a client and a server |
| [Architecture](docs/architecture.md) | The parts of the code |
| [Threat model](docs/threat-model.md) | What Post protects, and what it does not protect |
| [Verification](docs/verification.md) | Which parts of the code have a formal proof, and which parts have none |
| [Performance](docs/performance.md) | The performance budgets and the measured values |
| [Changelog](CHANGELOG.md) | The changes in each version |

## Repository layout

| Directory | Content |
|---|---|
| `crates/post-proto` | Message types, codec, and protocol rules |
| `crates/post-crypto` | Keys and proofs of accounts and servers |
| `crates/post-rules` | The proved rules of the server for accounts, handles, and invitations |
| `crates/post-server` | The server program, `post-server` |
| `crates/post-client` | The client logic without an interface |
| `crates/post-app` | The application, `post` |
| `docs` | The documents |
| `site` | The pages and the styles of the website |
| `tools` | Scripts for the automatic checks, and the program that makes the website |

## Development

Read [CONTRIBUTING.md](CONTRIBUTING.md) before you change the code. It gives the rules and the commands.

## Security

Do not report a security problem in a public issue. [SECURITY.md](SECURITY.md) tells you how to make a private report.

## License

Post has the [Apache License, version 2.0](LICENSE).

The application contains the fonts Inter and JetBrains Mono. They have the SIL Open Font License, version 1.1. The license texts are in `crates/post-app/fonts`.
