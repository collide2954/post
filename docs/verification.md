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
| The date arithmetic of a time | `crates/post-proto/src/time.rs` | For each day from 1970-01-01 to 9999-12-31, the arithmetic has no overflow. Each day number gives a valid date, with the correct length of each month. The conversion from a date to a day number and the conversion back are exact inverses. |

## Holes

A hole is code or an assumption that has no proof. Each hole is in this list.

### Holes that the project can close

| Hole | State |
|---|---|
| The text form of a time. The code that reads and writes the 24 characters calls the proved date arithmetic, but has no proof itself. | Open |
| The rules for names, text, codes, and addresses in the protocol crate. | Open |
| The rules of the server for accounts, handles, and invitations. | Open |
| The steps of the sign-in in the server and in the client. | Open |

### Holes that stay

Verus cannot work on these parts, or the parts are not our code:

- The network code, because Verus does not support async code.
- TLS, SQLite, the window toolkit, and the JSON library.
- Files and other input and output.
- The cryptographic algorithms. They come from libraries (D-168).
- The security of the sign-in against an attacker in the network. Verus does not analyze cryptographic protocols. The [threat model](threat-model.md) gives the argument in words.

## What a proof depends on

A proof is correct only if these parts are correct:

- Verus and the Z3 solver that it uses.
- The Rust compiler.
- The specifications that Verus gets for functions of the Rust standard library.
