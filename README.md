# Post

> Post is a modern chat app in the spirit of IRC: simple, open, self-hosted, with optional end-to-end encryption.

Post is between IRC and Telegram. It keeps the simplicity of IRC, and it adds the functions that most people use each day. The [design philosophy](docs/philosophy.md) gives the ideas behind Post.

![The chat screen of Post 0.1 in light mode](docs/images/chat-light.png)

## State of the project

Post is at the start of its development. Do not use it for real conversations.

Version 0.1 is the first slice. It shows that all the layers work together:

- Two or more people exchange text in one channel, `#lobby`.
- The server and the applications run on one computer.
- The application has English and Latin American Spanish text.
- The application has light mode and dark mode.

Version 0.1 has no accounts, no encryption, and no stored history. The connection does not go across a network. The [roadmap](docs/decisions/engineering.md) gives the sequence of the next versions (record D-100).

## Try version 0.1

You must have [rustup](https://rustup.rs). It installs the Rust release that the file `rust-toolchain.toml` gives.

1. Start the server:

   ```
   cargo run --bin post-server
   ```

2. In a second terminal, start the application:

   ```
   cargo run --bin post
   ```

3. Type a name, and select **Connect**.
4. Start the application again in a third terminal. Use a different name.
5. Write a message in one window. The message shows in the two windows.

The server listens on port 7678. To use a different port, give the number: `cargo run --bin post-server -- 9000`.

## Documents

| Document | Content |
|---|---|
| [Design philosophy](docs/philosophy.md) | The ideas behind Post |
| [Decision records](docs/decisions/README.md) | Each design decision and its reason |
| [Protocol specification](docs/protocol/README.md) | The messages between a client and a server |
| [Architecture](docs/architecture.md) | The parts of the code |
| [Changelog](CHANGELOG.md) | The changes in each version |

## Repository layout

| Directory | Content |
|---|---|
| `crates/post-proto` | Message types, codec, and protocol rules |
| `crates/post-server` | The server program, `post-server` |
| `crates/post-client` | The client logic without an interface |
| `crates/post-app` | The application, `post` |
| `docs` | The documents |
| `tools` | Scripts for the automatic checks |

## Development

Read [CONTRIBUTING.md](CONTRIBUTING.md) before you change the code. It gives the rules and the commands.

## Security

Do not report a security problem in a public issue. [SECURITY.md](SECURITY.md) tells you how to make a private report.

## License

Post has the [Apache License, version 2.0](LICENSE).

The application contains the fonts Inter and JetBrains Mono. They have the SIL Open Font License, version 1.1. The license texts are in `crates/post-app/fonts`.
