# Verification

This document tells which parts of Post have a formal proof, and which parts have none. Decision record D-177 gives the rule.

## What a proof is

Post uses the tool [Verus](https://github.com/verus-lang/verus). Verus reads a specification and the Rust code. It proves that the code obeys the specification for all inputs, not only for examples. If Verus cannot prove this, the check fails.

A proof is only as good as its specification. Read the specification to know what a proof shows. The specifications are in the Rust files, in the blocks that start with `verus!`.

Post as a whole is not verified. Only the parts in this document have a proof.

## How to run the proofs

1. Get Verus release 0.2026.11.02 from the [releases of Verus](https://github.com/verus-lang/verus/releases).
2. Put the directory of the release in `~/tools/verus`, or put it in your `PATH`.
3. Run `sh tools/proofs.sh`.

The commit hook runs the proofs if the computer has Verus. The automatic checks run them on each push.

Verus uses Rust 1.99.1 for the proofs. The normal build uses the Rust release that the file `rust-toolchain.toml` gives, and it does not use Verus.

## What is proved

| Part | File | What the proof shows |
|---|---|---|
| The date arithmetic of a time | `crates/post-proto/src/time/date.rs` | For each day from 1970-01-01 to 9999-12-31, the arithmetic has no overflow. Each day number gives a valid date, with the correct length of each month. The conversion from a date to a day number and the conversion back are exact inverses. |
| The text form of a time | `crates/post-proto/src/time/text.rs` | The specification gives the 24 characters of each time. The writer makes exactly these characters. The reader accepts a text only if it is the text of a time, and it gives that time. Thus the reader gives back each time that the writer writes, and it refuses all other texts. |
| The time type | `crates/post-proto/src/time.rs` | Each value that the proved code makes is a time from 1970-01-01 to 9999-12-31. |
| The name of a user | `crates/post-proto/src/name.rs` | The rule is 2 to 20 characters from a to z, 0 to 9, hyphen, and underscore. The reader accepts exactly the texts that obey the rule. For each other text, it gives the error that the specification gives. Each value that the proved code makes obeys the rule. |
| The code of an invitation | `crates/post-proto/src/code.rs` | The rule is 12 characters, each a letter or a digit. The reader accepts exactly the texts that obey the rule. Each value that the proved code makes obeys the rule. |
| The word for a part of the protocol | `crates/post-proto/src/part.rs` | The rule is 2 to 20 characters, each a lower-case letter or a hyphen, with no hyphen at the start or the end. The reader accepts exactly the texts that obey the rule. Each value that the proved code makes obeys the rule. |

## Holes

A hole is code or an assumption that has no proof. Each hole is in this list.

### Holes that the project can close

| Hole | State |
|---|---|
| The rules for names, text, codes, and addresses in the protocol crate. | Open |
| The rules of the server for accounts, handles, and invitations. | Open |
| The steps of the sign-in in the server and in the client. | Open |

### Holes that stay

Verus cannot work on these parts, or the parts are not our code:

- The network code, because Verus does not support async code.
- TLS, SQLite, the window toolkit, and the JSON library.
- Files, the clock, and other input and output.
- The connection code. Small functions connect the proved code to the libraries, for example the `Display` function and the serde functions of a type. They have tests, but no proof.
- The cryptographic algorithms. They come from libraries (D-168).
- The security of the sign-in against an attacker in the network. Verus does not analyze cryptographic protocols. The [threat model](threat-model.md) gives the argument in words.

## What a proof depends on

A proof is correct only if these parts are correct:

- Verus and the Z3 solver that it uses.
- The Rust compiler.
- The specifications that Verus gets for functions of the Rust standard library.
