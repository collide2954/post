# Changelog

This file lists the changes in each version of Post. The newest version is first. Decision record D-109 gives the rules for version numbers.

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
