# Post design philosophy

This document gives the ideas behind Post. Each design decision must agree with these ideas. The [decision records](decisions/README.md) give each decision and its reason.

## What Post is

> Post is a modern chat app in the spirit of IRC: simple, open, self-hosted, with optional end-to-end encryption.

Post is between IRC and Telegram.

- IRC is simple and clear, but it is old. IRCv3 does not change this sufficiently.
- Telegram has many functions, but it became too complex.

Post keeps the simplicity of IRC. It adds the functions that most people use each day: history, replies, reactions, files, search, and voice channels.

Post must be fast, good to look at, and easy to use.

## Principles

### 1. Simple first

A simple product is easy to learn, easy to trust, and easy to maintain.

- Post has few concepts: server, channel, direct message, message, and user.
- Each task has one good method, not many methods.
- A function must help most users. If it helps only some users, it is an option, or Post does not have it.
- We do not add a function only because a different application has it.
- If two solutions are equally good, we select the simpler one.
- If a concept that Post already has can do a job, we do not add a second concept.

### 2. Security is always on, and more security is an option

Each user gets good protection without work. A user can select more protection. This selection does not make Post more complex for other users.

- Each connection uses TLS. This is not an option.
- End-to-end encryption is an option for each chat. A chat with end-to-end encryption is a "sealed chat".
- Key verification, the application passphrase, the application lock, and the proxy are options.
- A default must be safe. A default must not ask the user for knowledge or work.
- All cryptography comes from libraries that experts examined. We do not write our own cryptography.

### 3. Tell the user the truth

A user can make a good decision only if the user knows the facts.

- Each chat shows who can read it. In a normal chat, the server can read the messages. In a sealed chat, only the members can read them.
- The documents tell what Post protects and what it does not protect.
- No independent audit of Post exists. The documents say this until an audit is complete.
- Post has no telemetry and no advertisements.
- Post connects only to the servers that the user adds. The server directory and the update check are the only exceptions, and the user starts each of them.

### 4. A community owns its server and its data

- Each server is independent. Any person can operate a server.
- A server is one program with one configuration file.
- An account has no password, no email address, and no telephone number.
- A user can export each chat that the user can read.
- Post does not depend on a central service.

### 5. Open

- The protocol is public and has a full specification.
- Protocol messages are JSON text that a person can read.
- Each program that obeys the protocol can connect. A bot is a normal account.
- The source code has the Apache-2.0 license.

### 6. Fast and calm

- Post starts quickly, scrolls smoothly, and uses little memory. Written budgets and benchmarks protect this.
- The interface puts text first. It has few colors, few icons, and no decoration.
- By default, only a direct message or a mention causes a notification.
- Post has no read receipts and no "last seen" times, because they put pressure on people.
- A timeline is always in time sequence. Post does not sort or promote messages.
- Voice does not ring. A user goes into a voice channel when the user is ready.

### 7. Easy for a new user, powerful for an expert

- A new user sends the first message after two steps.
- Each function has a button or a menu item.
- An expert can do the same work with slash commands and the keyboard.
- A settings screen changes the settings. A text file holds them, and an expert can edit the file.

### 8. The same quality on each system

- Windows, macOS, and Linux get equal support, on x86-64 and ARM64 processors.
- Post contains its own fonts and emoji, thus it looks the same on each system.
- Post obeys the conventions of each system where users expect them.

### 9. For all people

- A user can do each task with the keyboard only.
- Colors have sufficient contrast, and the user can change the text size.
- The interface is ready for translation. It has English and Latin American Spanish text.

### 10. Made to last

- We write each test before the code that it examines.
- Automatic checks stop a change that decreases the quality.
- We make small changes, and we record each design decision with its reason.
- We prefer technology that is simple and well known: SQLite, JSON, WebSocket, TLS, and QUIC.
- We use few dependencies, and we examine each one.

### 11. Version 1.0 is complete

An application becomes too complex when new functions come without end. Post has an end.

- Version 1.0 contains all the functions of Post.
- Each function is in version 1.0, or it is not in Post. There is no list for a later time.
- After version 1.0, we correct errors, keep Post secure, and keep it compatible with new systems.
- After version 1.0, we add a function only if it is strongly necessary.

## How we examine a new function

Before we add a function, we ask these questions:

1. Does the function help most users?
2. Can a concept that Post already has do the job?
3. Is the default safe, and can a new user ignore the function?
4. Does the function respect deletion, retention, and the permission to read a chat?
5. Can we tell the user the truth about the function in one sentence?
6. Can we build, test, and maintain it at the quality of the other parts?

If an answer is "no", we change the design, or we do not add the function.

After version 1.0, a new function must also be strongly necessary (principle 11).

Two examples:

- The retention rule already removes old messages. Thus Post has no separate timer for messages.
- A message link respects deletion and retention. A copy of a message does not. Thus Post has message links and no forward function.

## What Post is not

Post will not have these functions:

- Stickers, GIF search, and stories.
- Payments, mini applications, and advertisements.
- A feed that an algorithm sorts.

Post also does not have the functions in the left column. A simpler concept does the job, or the function puts pressure on people.

| Post does not have | Post has |
|---|---|
| A forward function | Message links |
| A timer for messages | The retention rule |
| Calls that ring | Voice channels |
| Read receipts and "last seen" times | Presence and a private read position |
| Custom roles | Fixed roles |
| A web panel for administration | Administration in the application |
| Passwords, email addresses, and telephone numbers | An identity from keys |
| Pinned messages | A channel topic that can contain a message link |
| Avatars and display names | A handle with a stable color |
| Automatic moderation | Moderators, reports, and rate limits |

These functions are also not in Post:

- Threads, polls, custom server emoji, and spoiler text.
- Recorded voice messages, video, screen share, and a record function for voice.
- Spell check.
- A web client and a mobile application. The open protocol lets other people build them.
- An IRC gateway.
- Federation between servers.

After version 1.0, principle 11 applies to each of these functions.

## How to change this document

A change to a principle is a design decision. Write a decision record for it, and change this document in the same commit.
