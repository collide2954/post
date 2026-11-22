# Product decisions

These records tell you what Post does for a user. Read the [guide](README.md) first.

### D-001 Independent servers

**Decision.** Each Post server is independent. Any person can operate a server. The application can connect to many servers at the same time. Servers do not exchange messages with each other.

**Reason.** This model is simple and fast, like an IRC network. The operator has full control of the community.

**Not selected.**

- Federated servers. They add much complexity: shared state, trust, and spam control between servers.
- One central service. It prevents self-hosted servers.
- A peer-to-peer network. It makes history and offline delivery difficult.

### D-002 Channels and direct messages

**Decision.** A server has a flat list of channels and direct messages. A private group is an invite-only channel. A reply shows a quote of the earlier message in the same timeline. Post has no side threads and no groups of channels.

**Reason.** Few concepts are easy to learn. A conversation stays in one place.

**Not selected.**

- Side threads. They hide a part of the conversation and add a second place to look.
- Channel categories. They add structure that an operator must maintain.

### D-003 Scope of version 1.0

**Decision.** Version 1.0 contains these functions. This list is the full scope of Post (D-122).

Text chat:

- Channels, direct messages, and message history with a retention rule.
- Replies, reactions, edit, delete, mentions, keyword highlights, and a mentions view.
- A small text format, code blocks with syntax colors, files, images, and link previews.
- Search, desktop notifications, message links, saved messages, drafts, and export.

Voice:

- Voice channels, and voice in direct messages.

Security:

- Sealed chats, key verification, the recovery phrase, and the device link.
- The application passphrase, the application lock, the proxy, and account proof.

Community:

- Roles, moderation tools, reports, and a moderation log.
- Administration in the application, server settings, and operator tools.
- Bots with slash commands, webhooks, and the server directory.

Application:

- Split panes, application profiles, custom themes, key bindings, and the protocol console.
- English and Latin American Spanish text.

One decision about the scope is open: the public web view (D-032).

**Reason.** The first scope was a small core. Two reviews of function options on 2026-11-20 added the other functions. Each function has its own record.

**Not selected.**

- A first release with text only.
- A wide scope with threads, polls, and custom emoji.

### D-004 History and retention

**Decision.** The server keeps the message history. Each channel has a retention rule: keep all messages, keep them for a set time, or keep no messages. The operator sets the default rule and the permitted range. A channel owner selects a rule in this range, and a direct message uses the default rule. The set time can be as short as some hours. The application removes its local copies by the same rule.

**Reason.** A user can read earlier messages on each device. Each community selects how long its messages stay.

**Not selected.**

- Permanent history only. Short-lived chats are then not possible.
- A short buffer only.
- No server history. It is weak for a user with more than one device.
- A rule that each channel owner selects freely. The operator then cannot limit the storage.
- One rule for the full server.

### D-005 No separate timer for messages

**Decision.** Post has no separate function that removes a message after a timer. The retention rule (D-004) does this job.

**Reason.** One concept is easier to learn than two. The documents tell the user that a reader can always copy a message before its removal.

**Not selected.**

- A timer that the sender sets on one message.

### D-006 Kinds of channel

**Decision.** A text channel is public or private:

- A public channel is in the channel list. Each member of the server can join it.
- A private channel is not in the list. An invitation is necessary to join it.

A channel can also have one of these modes:

- Announcement mode: only the owner and the moderators can write. The other members read and react.
- Archive mode: no person can write. The history stays readable.

A voice channel is a different kind of channel (D-038). Announcement mode also applies to a voice channel (D-044).

**Reason.** These kinds and modes are sufficient for most communities. Announcement mode is the present form of the moderated mode of IRC.

### D-007 Channel topic

**Decision.** Each channel has a topic line below its name. The owner or a moderator sets the topic. The topic can contain a message link (D-015). Post has no pinned messages.

**Reason.** The topic is an IRC tradition. It tells a user the purpose of the channel.

**Not selected.**

- A list of pinned messages. It adds a panel and more protocol.
- A header with the name only.

### D-008 Direct messages

**Decision.** A member can start a direct message with each member of the same server. A user can block a person (D-020). A setting can limit new direct messages to persons who share a channel with the user. The operator sets the default of this setting.

**Reason.** This is as open as IRC, and the user keeps control.

**Not selected.**

- A shared channel as a condition for all users.
- A request that the receiver must accept first. It adds steps.

### D-009 Edit and delete

**Decision.** A user can edit or delete the user's own messages at any time. An edited message shows the mark "edited". A deleted message goes out of view. A reply that quoted a deleted message shows "deleted message". A moderator can delete each message in the channel. A moderator cannot edit the message of a different person.

**Reason.** People make mistakes and must be able to correct them. The mark keeps the record honest. No person can change the words of a different person.

**Not selected.**

- A time limit for edit and delete.
- A public history of each edit.

### D-010 Text format

**Decision.** A message is plain text in the protocol. The application shows a small part of Markdown: bold, italic, strike, inline code, code blocks, quotes, and links. The specification defines this part exactly. A link always shows its full address. The application never shows HTML from a message.

**Reason.** Plain text is easy for a bot, and a person can read it without the application. An exact definition gives the same result in each client. A link cannot hide its target.

**Not selected.**

- Style ranges that travel separately from the text. Each client and bot must then calculate positions.
- Plain text with code blocks only.
- Spoiler text that stays hidden until a click. It helps only some communities.

### D-011 Message length

**Decision.** A message can have a maximum of 4,000 characters. For longer text, the application offers to send a text file. The operator can change the limit.

**Reason.** The timeline stays easy to read, and the server stays safe from very large messages.

**Not selected.**

- A limit of 1,000 characters.
- A limit of 16,000 characters.

### D-012 Reactions

**Decision.** Each Unicode emoji can be a reaction. The emoji selector shows the most-used emoji of the user first. A message can have only a limited number of different reactions.

**Reason.** The common reaction is one click away, and users can be expressive.

**Not selected.**

- A small fixed set of reactions.
- One "like" reaction only.

### D-013 Code blocks

**Decision.** A code block uses the monospace font and has a copy button. If the author gives the name of a language, the application adds syntax colors. A block without a language name has no colors.

**Reason.** Many users write code. Colors only on request prevent wrong guesses.

**Not selected.**

- No syntax colors.
- An automatic guess of the language for each block.

### D-014 Mentions and keyword highlights

**Decision.** The text `@handle` mentions a user. The text `@all` mentions all members of a channel. Only the channel owner and the moderators can use `@all`. A user can also make a list of words. A message that contains one of these words counts as a mention for that user.

**Reason.** Keyword highlights are an IRC tradition. They use the notification that mentions already have. A limited `@all` lets an important notice get to all members, and it is hard to abuse.

**Not selected.**

- No mention of all members.
- A mention of all members that each member can use.

### D-015 Message links

**Decision.** Each message has an https link. The link opens the application at that message.

**Reason.** A link respects deletion, retention, and the permission to read the chat. It is the honest method to show a message to a different person.

### D-016 No forward function

**Decision.** Post has no function that copies a message into a different chat. A user shares a message link (D-015).

**Reason.** A copy does not respect deletion, retention, or the permission to read the first chat. A link does.

**Not selected.**

- A forward function for normal chats only.

### D-017 Saved messages

**Decision.** A user can save a message. Saved messages show in a private list. The server keeps the list, thus it is the same on each device of the user.

**Reason.** A user can find important messages again quickly.

**Not selected.**

- Saved messages after version 1.0.
- Message links as the only method.

### D-018 Awareness signals

**Decision.** Post shows these signals:

- Presence: a user is online, away, or offline (D-019).
- A typing indicator. A user can turn off the indicator that the user sends.
- A private read position. It is the same on each device of the user. No other person sees it.

Post has no read receipts.

**Reason.** These signals help a conversation. A read receipt puts pressure on people, and it is noise in a channel.

### D-019 Presence

**Decision.** A dot shows that a user is online, away, or offline. The away state starts after idle time, or the user sets it. The user can add a short note to the away state. A user can select to show as offline. Post does not show a "last seen" time.

**Reason.** The dot and the note give useful information. A "last seen" time shows the habits of a person and adds pressure.

### D-020 Block a person

**Decision.** A blocked person cannot send direct messages to the user. In a channel, each message of that person shows as a thin line. The user can open the line to read the message. Post does not tell the blocked person.

**Reason.** The user gets quiet, and the conversation in a channel stays complete. The operator uses the server tools against abuse.

**Not selected.**

- A block for direct messages only.
- A block that hides all messages fully. A conversation with only one side is hard to follow.

### D-021 Notifications

**Decision.** By default, a direct message or a mention causes a desktop notification. Other messages only increase the unread count. For each channel, a user can select: all messages, mentions only, or no notifications. Post obeys the do-not-disturb mode of the operating system.

**Reason.** The default is calm, and the user does not miss a personal message.

### D-022 Notification text

**Decision.** A setting controls the content of a notification. It has three levels: name and text, name only, or the words "new message" only. By default, a normal chat shows name and text. A sealed chat shows the name only.

**Reason.** The text of a sealed chat must not show on a lock screen unless the user selects that.

### D-023 Names

**Decision.** A handle or a channel name contains only these characters: the letters a to z, the digits 0 to 9, the hyphen, and the underscore. A handle has 2 to 20 characters.

**Reason.** The rule is simple. It prevents a name that looks almost the same as a different name.

**Not selected.**

- Letters of all languages, with a check for look-alike names.
- Almost all visible characters.

### D-024 Handle change

**Decision.** A user can change the handle. The old handle stays reserved for 30 days. A user can change the handle one time in 30 days. Earlier messages show the new handle. The operator can change these limits.

**Reason.** A user can correct a mistake without a new account. The limits prevent confusion and abuse.

**Not selected.**

- A permanent handle.
- A change at any time.

### D-025 Profile card

**Decision.** An account has one handle on a server. The application gives each handle a stable color. A profile card can have a short "about" line and the time zone of the user. If the time zone is there, the card shows the local time of the user. The card can also show account proofs (D-148). Post has no avatars and no display names.

**Reason.** Post puts text first. A stable color helps a reader to recognize a person. Without pictures, there is no upload and no moderation of pictures.

**Not selected.**

- An avatar for each account.
- A display name together with the handle. Look-alike names then become possible.

### D-026 Welcome text

**Decision.** The operator can write a welcome and rules text in the message text format. A new member sees it at the time of the join. All members can read it in the server information view.

**Reason.** This is the message of the day of IRC in a calm form. A new member gets the rules immediately.

**Not selected.**

- The text at each connection. It becomes noise.
- No welcome text.

### D-027 Export

**Decision.** A user can export each chat that the user can read. The result is a plain-text log or a file of JSON lines, together with the files of the chat. Post does not write log files automatically.

**Reason.** The data belongs to the users. An archive does not depend on the retention rule of the server.

**Not selected.**

- Automatic plain-text logs. They are a privacy risk for sealed chats.

### D-028 Account deletion

**Decision.** When a user deletes the account, the user selects one of two results. The messages stay with the name "deleted account", or the server erases all of them. The handle becomes free after a time that the operator sets.

**Reason.** This respects the conversation record and the right of a person to go away fully.

### D-029 Bots and other clients

**Decision.** Each program that obeys the protocol can connect. A bot is an account with a visible "bot" mark.

- A member makes a bot in the application and selects its handle. The member is the owner of the bot.
- The profile card of a bot shows its owner. The owner can turn the bot off.
- A bot signs in with a key file, as each account does (D-062).
- The operator can make approval necessary for a new bot. The operator can also set a maximum number of bots for each member.

**Reason.** An open protocol lets a bot culture grow, as on IRC. Each bot has a responsible person. One method to sign in is sufficient for all accounts.

**Not selected.**

- Bots that only an operator can make.
- Bots that join as a person does. No person is then clearly responsible.
- Access tokens for bots. They are a second method to sign in.
- A server that accepts the official application only.

### D-030 Webhooks

**Decision.** A channel owner can make a secret address for the channel. An external service sends an HTTPS request to this address. The server puts the text of the request in the channel, from a sender with a "bot" mark. The request contains simple JSON, for example `{"text": "Build 214 is complete"}`. Many services can already send this form. The text uses the message text format.

**Reason.** A project community gets messages from its tools, and no program has to run all the time.

**Not selected.**

- Parsers for the formats of specific services. Each parser is code that we must maintain.
- Bot accounts as the only method.

### D-031 Link previews

**Decision.** Below a message with a link, the application shows the page title and a small image. The server gets this data, not the application. The operator can turn the function off. A sealed chat has no link previews.

**Reason.** The network address of a reader never goes to the linked site. The server cannot read a sealed chat, thus it cannot make a preview there.

**Not selected.**

- Link previews after version 1.0.
- No link previews.

### D-032 Public web view

**Decision.** This decision is open. The project examines the public web view again at a later time, before the work on version 0.15 starts. Until then, the function is not in the scope of version 1.0 (D-003).

The function is this: a channel owner can publish a public channel as read-only web pages. The function is off until the owner turns it on. A published channel shows the label "public on the web".

**Reason.** The function helps an open project, because answers stay easy to find. But it adds web pages to the server that we must protect. The project takes more time for this decision.

**History.** On 2026-11-20, this record first put the public web view into version 1.0. On the same day, the project made the decision open again.

### D-033 No IRC gateway

**Decision.** The server does not speak the IRC protocol. A person uses Post with the application, or with a different client that obeys the Post protocol.

**Reason.** A gateway is a second protocol to build, protect, and maintain. This does not agree with the principle "Simple first".

**History.** On 2026-11-20, this record first put an IRC gateway into version 1.0. The project removed the gateway on the same day.

### D-034 Things that Post will not have

**Decision.** Post will not have these functions:

- Stickers, GIF search, and stories.
- Payments, mini applications, and advertisements.
- A feed that an algorithm sorts.

**Reason.** These functions add noise or make Post a platform. They do not help a conversation.

### D-035 Functions that are not in Post

**Decision.** These functions are not in Post:

- Threads, polls, custom server emoji, and spoiler text.
- Recorded voice messages.
- Spell check.
- A web client. Chat in a browser is not possible.
- A mobile application. The protocol must not depend on a desktop computer, and the public specification lets other people build a mobile client.
- Federation between servers.

Other records give more functions that are not in Post:

- D-007: pinned messages.
- D-016: a forward function.
- D-025: avatars and display names.
- D-033: an IRC gateway.
- D-040: video and screen share.
- D-044: a record function for voice.

**Reason.** Each function adds weight, and each one fails a question of the design philosophy. Version 1.0 is complete (D-122), thus these functions do not come later.

### D-036 Description of Post

**Decision.** The short description of Post is: "Post is a modern chat app in the spirit of IRC: simple, open, self-hosted, with optional end-to-end encryption."

**Reason.** The sentence gives the origin of Post and its four primary properties.

### D-122 Version 1.0 is complete

**Decision.** Version 1.0 contains all the functions of Post. Each function is in version 1.0, or it is not in Post. After version 1.0, we correct errors, keep Post secure, and keep it compatible with new systems. We add a function after version 1.0 only if it is strongly necessary. A decision record must then give the necessity.

**Reason.** An application becomes too complex when new functions come without end. A product that is complete stays simple.

**Not selected.**

- A list of functions for a time after version 1.0.

### D-123 Media in the timeline

**Decision.** PNG, JPEG, GIF, and WebP images show in the timeline. A click opens an image viewer. A message with more than one image shows the images as a grid. Each other file shows as a card with its name, its size, and a button to save or open it. A setting can show each image as a small card until the user clicks it (D-152).

**Reason.** Most shared media are images. Post contains no audio or video codecs for files, thus there is less code to protect.

**Not selected.**

- Audio and video players in the timeline.
- File cards only.

### D-124 Timeline comforts

**Decision.** The timeline has these functions:

- A small gray line shows when a person joins or leaves a channel. These lines are on by default, and a setting hides them.
- A very long message or code block shows only its first lines, with a "show more" control.
- A user can select a part of a message and reply to only that part.
- A user can select more than one message and copy them as a text log, with time and handle.

**Reason.** The join and leave lines are an IRC tradition and show the life of a channel. The other functions keep the timeline easy to read and to quote.

**Not selected.**

- No join and leave lines.
- Join and leave lines that are off by default.

### D-125 Mentions view

**Decision.** One view lists the recent mentions and keyword highlights of the user from all servers. A click goes to the message.

**Reason.** After time away, the user sees in one place where people spoke to the user. The view shows data that Post already has.

### D-126 Drafts

**Decision.** Text that a user wrote but did not send is a draft. The server keeps the drafts of the user, thus each device of the user shows them. The draft of a sealed chat stays on the device where the user wrote it.

**Reason.** A user can start a message on one computer and complete it on a different one. The server must not get the text of a sealed chat.

**Not selected.**

- Drafts that always stay on one device.

### D-127 Find a person

**Decision.** A user finds a person in the member list of a shared channel, or the user types the exact handle. Members get no list of all accounts on a server.

**Reason.** A member cannot collect the names of all accounts. This makes spam and unwanted contact more difficult.

**Not selected.**

- A list of all members that the operator can turn off.

### D-128 Bot commands

**Decision.** A bot can register slash commands with the server. The application shows these commands in the `/` menu, with a description from the bot. A later round of design questions sets the details.

**Reason.** A user finds the commands of a bot easily.

**Not selected.**

- Commands as plain text by convention, for example `!help`, without registration.

### D-129 Limits for bots

**Decision.** A bot cannot do these things:

- Have the moderator role.
- Go into a voice channel.
- Be a member of a sealed chat.
- Start a direct message with a member. A bot can answer in a direct message that a member started.

**Reason.** A bot is a helper in normal text channels. Moderation, voice, and sealed content stay with persons.

### D-130 Help for bot authors

**Decision.** The client crate of Post (`post-client`) is a Rust library that a bot author can use. The repository contains two or three small example bots. An author who uses a different language works from the specification and the JSON Schemas.

**Reason.** The crate exists in all cases, thus the cost is small.

**Not selected.**

- The specification as the only help.
- Official libraries for more languages.

### D-181 Invitation to a private channel

**Date.** 2026-11-22.

**Decision.** The owner of a private channel invites a member of the server by handle. The channel then shows in the channel list of that member, with the mark "invited". The member joins the channel or ignores the invitation.

**Reason.** No person is put into a channel. A member is only in the channels that the member selected (D-136). An invitation by handle goes only to the person that the owner selected.

**Not selected.**

- The owner adds a member directly.
- An invite link for the channel. A link can go to people that the owner did not select.

### D-184 Idle time for the away state

**Date.** 2026-11-22.

**Decision.** The away state (D-019) starts automatically after 10 minutes with no keyboard input and no mouse input on the computer. The application asks the operating system for the idle time. If the operating system does not give the idle time, the application uses the time with no input in its own window.

**Reason.** A person who works in a different program is at the computer and can answer. The dot must tell the truth about that.

**Not selected.**

- Only the input in the window of Post. A person who works in a different program then shows as away.

### D-185 Owner of a channel

**Date.** 2026-11-22.

**Decision.** A channel always has one owner, and the owner is a member of the channel. The owner cannot leave the channel while the owner has that role. The owner first gives the channel to a different member, or the owner deletes the channel.

**Reason.** A channel never stays without control, also before the operator tools come (D-142). A member never gets a channel by surprise.

**Not selected.**

- A channel with no owner until an operator gives it to a member.
- The member who is longest in the channel becomes the owner automatically.

### D-189 One presence for a profile

**Date.** 2026-11-22.

**Decision.** The presence settings of a user apply to all servers of the profile. These settings are the away state, the away note, and the selection to show as offline (D-019). The application has one control for them.

**Reason.** One control is simple. A profile is already one part of the life of a user (D-147).

**Not selected.**

- One presence control for each server.

### D-190 Fixed kind of a channel

**Date.** 2026-11-22.

**Decision.** The owner selects the kind of a channel, public or private (D-006), when the owner makes the channel. The kind does not change later.

**Reason.** The messages of a private channel never show to people who got no invitation. Each chat tells the truth about who can read it.

**Not selected.**

- A kind that the owner can change in the two directions.
- A change from public to private only.
