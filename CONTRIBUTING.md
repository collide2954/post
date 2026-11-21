# How to contribute to Post

Thank you for your interest in Post.

## What the project accepts

The design of Post changes quickly during the 0.x versions (decision record D-120).

- A bug report is welcome. Open an issue.
- An idea is welcome. Open an issue. Read the [design philosophy](docs/philosophy.md) first, because it tells which functions Post will not have.
- For a code change, open an issue first and wait for an answer. Do not start with a pull request.
- Do not open a public issue for a security problem. Read [SECURITY.md](SECURITY.md).

Obey the [code of conduct](CODE_OF_CONDUCT.md).

## Sign-off

Each commit from an external contributor must have a sign-off line. With this line, you agree to the [Developer Certificate of Origin](https://developercertificate.org). The command `git commit -s` adds the line.

## Prepare your computer

1. Install [rustup](https://rustup.rs). It reads `rust-toolchain.toml` and installs the correct Rust release.
2. Activate the commit hook of the repository:

   ```
   git config core.hooksPath .githooks
   ```

The hook runs the format check, clippy, and the tests before each commit.

## Rules for code

- Write the test before the code that it examines (D-101).
- A commit contains the test and the code together, and all tests pass.
- Make small commits. A commit message is one line in the imperative, for example "Add hello and welcome messages" (D-108).
- Write few comments. A comment is correct only where the code cannot give the information (D-106).
- `unsafe` code is not permitted.
- Each interface text comes from the catalogs in `crates/post-app/i18n`. A change to an English text also changes the Spanish text (D-092).
- Examine each new dependency. Post uses few dependencies (D-110).

## Rules for documents

All documents obey ASD-STE100 Simplified Technical English (D-107).

- Write short sentences: a maximum of 20 words in a procedure and 25 words in a description.
- Use the active voice.
- Give one instruction in each sentence.
- Use one word for one thing. For example, write "function" and not "feature", and write "the application" and not "app".

A change to the code and the related change to a document go into the same commit.

A design decision gets a [decision record](docs/decisions/README.md).

## Commands

| Task | Command |
|---|---|
| Run all checks of the hook | `sh .githooks/pre-commit` |
| Examine licenses and security advisories | `cargo deny check` |
| Measure the coverage | `sh tools/coverage.sh` |
| Write the JSON Schemas again | Set `POST_UPDATE_SPECIFICATION=1`, then run `cargo test -p post-proto` |
| Write screenshots of each screen | Set `POST_SCREENSHOTS` to a directory, then run `cargo test -p post-app --test window` |
| Start the application in Spanish | Set `POST_LANGUAGE=es-419`, then run `cargo run --bin post` |

The coverage command uses `cargo-llvm-cov`. The license command uses `cargo-deny`.

## Automatic checks

Each push starts these checks on GitHub (D-103):

- The format check and clippy.
- The tests on Windows, macOS, and Linux.
- `cargo deny`.
- The coverage minimum of 90% for each crate (D-104).

Mutation tests and fuzz tests start each night.
