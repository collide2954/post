# Changelog

This file lists the changes in each version of Post. The newest version is first. Decision record D-109 gives the rules for version numbers.

## 0.1.4 - 2026-11-22

- Add the rules core of the server (D-178). Verus proves the rules for accounts, handles, and invitations, and SQLite only stores the data.
- Replace the library `base64` with our own proved code for the text form of keys (D-179).
- Add proofs for the text form of the seed and for the generator of invitation codes.
- Let only one program use the data of a server. The operator stops the server before the command `post-server owner-link`.

## 0.1.3 - 2026-11-21

- Add the sign-in of an account to the protocol and to the server (D-163). The application does not use it yet.
- Add the fields for client requirements to the sign-in (D-176).
- Add the configuration file, the data directory, and the owner-link command of the server.
- Add the profile store of the client, and the reader for invite links and server addresses. The application does not use them yet.
- Add formal proofs with Verus (D-177). The document `docs/verification.md` tells what each proof shows.

## 0.1.2 - 2026-11-21

- Add the website of the project (D-175).
- Add the first form of the postmark icon (D-095).
- Add the cryptography crate with the seed, the keys, and the proofs for the sign-in (D-163).
- Add the storage and the configuration file of the server. The server program does not use them yet.

## 0.1.1 - 2026-11-21

- Add 12 decision records for version 0.2 (D-163 to D-174).
- Add the threat model.
- Give a version to the handle change, the settings screen, and the appearance settings (D-100).

## 0.1.0 - 2026-11-21

Version 0.1 is the first slice (D-099). Two or more users on one computer exchange text in the channel `#lobby`.

- Add the application window with a connect screen and a chat screen.
- Add light mode and dark mode in the "ink and paper" style, with the fonts Inter and JetBrains Mono.
- Add English and Latin American Spanish text for the interface.
- Add interface tests that run without a window. One test runs the complete application with a real server.
- Add fuzz targets, nightly checks, and release builds for Windows, macOS, and Linux.
- Add the README, the contribution documents, and the architecture document.

This version has no accounts, no encryption, and no stored history. It connects only to a server on the same computer.

## 0.0.5 - 2026-11-21

- Make the repository a Cargo workspace with a pinned toolchain, a commit hook, and automatic checks.
- Add the protocol crate with its specification, JSON Schemas, and example conversations.
- Add the server program for one channel on the local computer.
- Add the client logic: the address rule, the session, and the connection.

## 0.0.4 - 2026-11-21

- Add 8 decision records for version 0.1 (D-155 to D-162).
- Select Inter and JetBrains Mono as the fonts (D-084).

## 0.0.3 - 2026-11-20

- Make the public web view an open decision (D-032).

## 0.0.2 - 2026-11-20

- Make version 1.0 the final function set (D-122).
- Remove the IRC gateway from the design.
- Add 33 decision records for voice, bots, the server, and the application.

## 0.0.1 - 2026-11-20

- Add the design philosophy.
- Add 121 decision records.
- Add the Apache 2.0 license.
