# Server decisions

These records tell you how a Post server works and how an operator uses it. Read the [guide](README.md) first.

### D-051 Storage

**Decision.** The server stores its data in SQLite. SQLite is a part of the server program. Uploaded files go into a directory. The code has a storage boundary, thus PostgreSQL stays possible.

**Reason.** An operator installs nothing more. One file and one directory are easy to back up.

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

**Decision.** The operator selects the join policy. A new server accepts new members by invitation only. The operator can change the server to open sign-up.

**Reason.** A new server is safe from spam from the first minute.

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
