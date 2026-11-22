# Changelog

This file lists the changes in each version of Post. The newest version is first. Decision record D-109 gives the rules for version numbers.

## 0.2.0 - 2026-11-22

Version 0.2 adds identity and the connection across a network (D-100, D-173). The versions 0.1.3 to 0.1.6 added the accounts, the invitations, the proved rules of the server, TLS, the Servers screen, the handle change in the protocol, and the page for an invite link.

- Add the Invitations panel. The server owner makes, copies, and cancels invite links (D-166).
- Add the panel for the handle change (D-024).
- Add the settings file and the Settings page with the language, the time format, the text size, and the protocol console (D-087, D-167). The application applies a change in the file.
- Add the Profiles page and the command `post --profile <name>` (D-165).
- Add the protocol console. It shows each line of the connection, and it sends a line that the user types (D-094).
- Add the measurements for the performance budgets (D-170). The document `docs/performance.md` gives the values.
- Decrease the memory of the server for each idle connection from 137 kB to 10 kB.
- Make the text of a message one time for all the connections that get the same message.

The application keeps the seed of a profile in a file that only your user account can read. The seed has no other protection in this version (D-169).

This version has one channel and no stored history. The server can read each message.

## 0.1.6 - 2026-11-22

- Add TLS 1.3 to the server and to the application (D-061). A server can make its own certificate (D-164) or use certificate files.
- Ask the user to accept the fingerprint of a private certificate, and give a warning if the certificate changes (D-073).
- Add the Servers screen (D-171). The application connects at the start to the server that was last in use.
- Add the handle change with the `rename` messages (D-024).
- Show a page for an invite link in a browser (D-173).

## 0.1.5 - 2026-11-22

- Let the application sign in with an account (D-163). The application makes the first profile and remembers the key of each server.
- Add the messages `invite`, `invitations`, and `revoke` for invitations. Only the server owner can use them.
- Remove the temporary `guest` message. Each connection now signs in with an account.
- Prove that only the server owner cancels an invitation.

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
