# Server decisions

These records tell you how a Post server works and how an operator uses it. Read the [guide](README.md) first.

### D-051 Storage

**Decision.** The server stores its data in SQLite. SQLite is a part of the server program. Uploaded files go into a directory. The code has a storage boundary, thus PostgreSQL stays possible.

**Reason.** An operator installs nothing more. One file and one directory are easy to back up.

**History.** On 2026-11-21, the project gave the rules for accounts, handles, and invitations to a proved core in memory (D-178). SQLite stores the data of the core, and it makes no decisions about these rules.

**Not selected.**

- PostgreSQL only.
- SQLite and PostgreSQL from the start. Each query then has two implementations.

### D-052 Operation

**Decision.** The server is one program with one TOML configuration file and safe defaults. The operator does the daily administration in the Post application, with screens and slash commands. The server has no web panel.

**Reason.** A server is easy to start. The operator uses one interface for chat and administration.

**Not selected.**

- A web panel. It is a second interface to build and protect.
- Administration on the command line only.

### D-053 TLS certificates

**Decision.** The operator gives the server a domain name. The server then gets a free certificate from Let's Encrypt and renews it automatically. The server can also use certificate files, or it can operate behind a reverse proxy.

**Reason.** The easiest method is the default. An operator does not have to learn certificate tools.

**Not selected.**

- Certificate files or a proxy as the only methods.

### D-054 First operator

**Decision.** At the first start, the server prints an owner link that works one time. The person who opens the link in the application selects a handle and becomes the server owner.

**Reason.** The operator does not copy keys and does not edit a file.

**Not selected.**

- A public key in the configuration file.
- The first account that joins. A stranger can be first.

### D-055 Who can join a server

**Decision.** The operator selects one of three join policies:

- Invitation: a person joins with an invite link. A new server has this policy.
- Open sign-up: each person who knows the address can join.
- Request: a person asks to join with a short note, and an operator approves or declines.

**Reason.** A new server is safe from spam from the first minute. An operator can open the server gradually.

**Not selected.**

- Open sign-up as the default.
- Invitation as the only policy.

### D-056 Who can invite

**Decision.** By default, only operators make invitations to the server. An operator can let all members invite. An invitation can have an end date and a limit of uses. The server records who invited each member.

**Reason.** The operator controls growth and can find the source of abuse.

**Not selected.**

- Invitations from each member by default.
- Invitations from operators only, without a setting.

### D-057 Who can make a channel

**Decision.** Each member can make a channel and becomes its owner. The operator can limit this function to operators. The operator can also set a maximum number of channels for each member.

**Reason.** A community can grow by itself, as on IRC.

**Not selected.**

- Operators only by default.
- No limit setting.

### D-058 Roles

**Decision.** Post has fixed roles:

- On a server: the server owner and the operators.
- In a channel: the channel owner, the moderators, and the members.

A moderator can delete messages, mute a member, remove a member, and ban a member. A channel owner can also set the topic, the retention rule, and the invitations. There are no custom roles.

**Reason.** Fixed roles are as clear as the operators of an IRC channel. A permission matrix is hard to manage.

**Not selected.**

- Custom roles with a permission matrix.
- One owner with all powers and no moderators.

### D-059 Server logs

**Decision.** The server logs never contain message content. The server keeps network addresses for 7 days to help against abuse, then it removes them. The operator can change this time or turn off the address log. The server information view shows the policy to the members.

**Reason.** The default protects privacy, and the operator can still stop abuse. Members can read the policy.

**Not selected.**

- No address log.
- Logs without a policy in the server.

### D-060 Server directory

**Decision.** The Post repository contains a file with a list of public servers. An operator asks for an entry with a pull request, and the project examines each change. The application downloads the list only when the user opens the directory.

**Reason.** A new user finds a community. The project does not have to operate a service.

**Not selected.**

- A list inside each release.
- A directory service that operators update themselves.

### D-133 Where the server settings are

**Decision.** The TOML file holds the settings that the server must have to start: the address, the domain name, the certificate, and the data directories. An operator sets all policy in the Post application: limits, the join policy, function switches, and the retention rule. A policy change applies immediately, and the server does not have to start again. The file can give the first values of the policy for an installation by script.

**Reason.** The operator changes policy in the same place where the operator sees its effect. An installation by script stays possible.

**Not selected.**

- All settings in the file.
- All settings in the application.

### D-134 Server limits

**Decision.** An operator can set these limits:

- The message length.
- The upload size.
- The storage for each member, and the total storage.
- The number of channels for each member.
- The message rate.

Each limit has a safe default. The same limits apply to all members.

**Reason.** A new operator has to change nothing. A small set of limits is easy to understand.

**Not selected.**

- Different limits for each role and each channel, and rules for file types.
- Limits for upload size and total storage only.

### D-135 Function switches

**Decision.** An operator can turn off each of these functions for a server: voice, file upload, link previews, and webhooks. The application hides a function that is off.

**Reason.** A small server operates only the functions that it can protect and pay for.

**Not selected.**

- All functions always on.

### D-136 No default channels

**Decision.** A server has no default channels. A new member starts with the channel list and selects the channels to join.

**Reason.** A member is only in channels that the member selected.

**Not selected.**

- Default channels that each new member joins automatically.

### D-137 Limits for new accounts

**Decision.** An operator can set limits for a new account, for a set number of hours or days. In this time, the account cannot upload files and cannot invite. It can start a direct message only with a person who shares a channel with it. These limits are off by default.

**Reason.** An open server gets a simple and clear defense against spam accounts.

**Not selected.**

- No limits for new accounts.

### D-138 Moderation tools

**Decision.** In addition to the powers in D-058, moderators have these tools:

- A mute or a ban can end automatically after a set time.
- A member can report a message with a short reason. The owner and the moderators of the channel see the report.
- A moderation log records each moderation action, the person who did it, and the reason. Owners and operators can read the log.
- Slow mode: a channel setting that lets each member send only one message in a set time.

**Reason.** Moderators get tools that are in proportion to the problem. The log makes the use of power visible.

### D-139 No automatic moderation

**Decision.** The server does not moderate content automatically. It has no word filters and no spam scores. Rate limits protect the server from a flood of messages. A bot can tell the moderators about a problem, but it cannot moderate (D-129).

**Reason.** Automatic moderation makes errors that are hard to explain. Decisions about people stay with people.

**Not selected.**

- A word filter for each channel.
- Spam detection in the server.

### D-140 Server bans

**Decision.** An operator can ban an account from the server. An operator can also ban a network address or a range of addresses. Each ban can have an end time and a reason. Each ban goes into the moderation log.

**Reason.** An address ban stops a person who makes new accounts after a ban.

**Not selected.**

- Bans for accounts only.

### D-141 What an operator can read

**Decision.** The application does not let an operator open a private channel or a direct message of other persons. A report shows the reported message with some messages before and after it. The documents tell the user that a person with access to the server computer can read normal chats.

**Reason.** An operator gets what is necessary to act on a report, and no more. The truth about the server computer stays visible.

**Not selected.**

- Access to each normal chat, with an entry in the moderation log.
- Free access to each normal chat.

### D-142 Operator power over channels

**Decision.** An operator can manage each channel on the server. The operator can moderate it, change its owner, put it into archive mode, or delete it. Each of these actions goes into the moderation log. For a private channel, the operator does these actions without access to its messages (D-141).

**Reason.** The server stays manageable, as an IRC network does. A channel whose owner went away does not stay without control.

**Not selected.**

- Operators who can only delete a channel or ban a person.

### D-143 Reserved names

**Decision.** An operator keeps a list of handles and channel names that no person can take, for example `admin` or `support`.

**Reason.** This prevents an account that pretends to be the staff of the server.

### D-144 Lockdown switch

**Decision.** One switch stops new joins and new invitations and puts slow mode on all channels. An operator uses it for the time of a spam attack.

**Reason.** In an attack, an operator must be able to act with one step.

### D-145 Server status view

**Decision.** Operators see the status of the server in the application. The view shows the members online, the messages for each day, the storage in use, and the voice use.

**Reason.** An operator sees problems early, with no tools outside Post.

### D-146 Operator tools

**Decision.** The server has these tools:

- A backup command makes a consistent copy of the database and the files while the server runs. A restore command puts the copy back.
- A member can get a copy of the data that the server holds about the account.
- An optional metrics endpoint gives numbers to tools such as Prometheus. It is off by default.
- An operator setting can make the handle of an account free after a set time without use, as IRC networks do.

**Reason.** A self-hosted server must be easy to keep safe. The data copy helps an operator who must obey a privacy law.

### D-164 Own certificate

**Date.** 2026-11-21.

**Decision.** If the operator gives the server no certificate files, the server makes its own certificate at the first start. The application shows the fingerprint of this certificate one time and asks the user to accept it (D-073). Certificate files replace the own certificate at any time. The automatic certificate from Let's Encrypt (D-053) comes in version 0.16.

**Reason.** A server at home or in a private network starts with no certificate steps, and TLS stays on.

**Not selected.**

- A server that listens only on the local computer until the operator gives it certificate files.

### D-166 Invitation limits

**Date.** 2026-11-21.

**Decision.** In the application, a new invitation stops after 7 days and has no limit of uses. The person who makes the invitation can select a different end time and a limit of uses. A person who can make invitations can also cancel each invitation at any time.

**Reason.** A link that leaks does not stay open for a long time. A community can still use one link for many people.

**Not selected.**

- One use for each link by default.
- No end time by default.

### D-172 New owner link

**Date.** 2026-11-21.

**Decision.** The command `post-server owner-link` makes a new owner link that works one time (D-054). The person who opens the link in the application becomes the server owner, with a new account or with an account that already exists. The earlier server owner becomes a normal member.

**Reason.** A server owner can lose the key, and no recovery phrase exists before version 0.11. Only a person with access to the server computer can run the command, and that person already controls the server.

**History.** On 2026-11-21, the rules of the server moved into memory (D-178). From that day, the operator stops the server before the command, because only one program can use the data of a server.

**Not selected.**

- No such command before the operator tools in version 0.16. The operator must then delete the server data and start again.
