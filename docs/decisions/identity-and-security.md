# Identity and security decisions

These records tell you how Post handles accounts, keys, and encryption. Read the [guide](README.md) first.

### D-061 Security model

**Decision.** Each connection uses TLS 1.3. This is not an option. In a normal chat, the server can read the messages, thus history, search, and moderation work. A user can make a direct message or a channel a sealed chat. A sealed chat has end-to-end encryption, and the server stores only ciphertext.

**Reason.** Each user gets good protection without work. A user who selects more protection gets it. A large public channel gets little from end-to-end encryption.

**Not selected.**

- End-to-end encryption for all direct messages by default.
- End-to-end encryption for all chats.
- TLS only in the first release.

### D-062 Identity

**Decision.** The application makes a key pair for the user. On each server, the user selects a handle, and the server binds the handle to the public key. An account has no password, no email address, and no telephone number.

**Reason.** A new user starts as quickly as on IRC, and the handle stays the property of the user. Keys make end-to-end encryption possible.

**Not selected.**

- A user name and a password.
- IRC nicknames without an account.
- Sign-in with an email address or a telephone number.

### D-063 One seed and one key for each server

**Decision.** The application keeps one secret seed. From the seed, it calculates a different key pair for each server. A user can prove later that two accounts belong to the same person.

**Reason.** Servers cannot connect the accounts of a user with each other. One backup of the seed brings all accounts back.

**Not selected.**

- One key for all servers. Servers can then connect the accounts.
- An independent key for each server. Each key then makes its own backup necessary.

### D-064 Recovery and more devices

**Decision.** The application offers a recovery phrase of 24 words. The phrase is optional, and we recommend it. To add a device, the user scans a QR code or types a short code from a device that the user already has. If a user loses all devices and the phrase, only the operator can make the handle free.

**Reason.** A user gets a second device easily. A careful user can always get the accounts back.

**Not selected.**

- A device link only, without a phrase.
- A backup of the seed on the server with a passphrase. A weak passphrase is open to attack.

### D-065 Key storage

**Decision.** The keychain of the operating system protects the seed. The user types nothing. A setting adds an application passphrase for a user who selects more protection. If a system has no keychain, a file with a passphrase protects the seed.

**Reason.** The default is safe and has no steps. More protection is an option.

**Not selected.**

- The keychain only.
- A passphrase at each start for all users.

### D-066 Application lock

**Decision.** If the user sets the application passphrase, Post can lock itself. The lock starts after idle time or when the user asks for it. The passphrase opens Post again.

**Reason.** The lock protects a computer that is not locked. A user without a passphrase never sees the function.

### D-067 Data on the device

**Decision.** The application keeps a local copy of the chats. With this copy, Post starts quickly, and the user can read without a connection.

- The application always encrypts the content of sealed chats on the disk, with a key from the keychain.
- The application does not encrypt normal chats on the disk.
- If the user sets the application passphrase, the application encrypts the full local database.

**Reason.** Sealed content always has protection. Normal content has the same protection as the data of other applications, and the user can select more.

**Not selected.**

- Encryption of all local data for all users.
- No local copy.

### D-068 Encryption for sealed chats

**Decision.** Sealed chats use Messaging Layer Security (MLS, RFC 9420) from the OpenMLS library.

**Reason.** MLS is a public standard. One mechanism protects a direct message and a channel with thousands of members. It gives forward secrecy and recovery after the loss of a key.

**Not selected.**

- Ratchets in the Signal style. Groups scale less well.
- One shared key for each chat. It has no forward secrecy.

### D-069 How a chat becomes sealed

**Decision.** A user can select "sealed" when the user makes a chat. A user can also seal a chat later. A chat cannot go back from sealed to normal. A line in the timeline shows where the sealed part starts. Earlier messages do not change. An operator can make "sealed" the default for new direct messages.

**Reason.** Protection never goes away silently. A user can seal a conversation that already exists.

**Not selected.**

- Sealed at creation only. A user then has two chats with the same person.
- A change in the two directions. The state then becomes unclear.

### D-070 Key verification

**Decision.** A sealed chat works immediately. The application remembers the key of each person and gives a clear warning if a key changes. Two users can also compare a short safety code or scan a QR code. The application then shows the contact as verified.

**Reason.** Protection has no first step. Verification is an option for a user who selects more protection.

**Not selected.**

- Verification as a condition for each sealed chat.
- A warning without a function to compare keys.

### D-071 Earlier history in a sealed chat

**Decision.** A new member of a sealed chat sees only the messages after the join. The new member can ask for the earlier history. Only the owner or a moderator of the chat can give it. A new device of a user gets the sealed history from a different device of the same user, at the time of the device link.

**Reason.** Earlier messages stay safe unless a person with authority shares them. A user does not lose the user's own history.

**Not selected.**

- No earlier history in all cases.
- History that each member can share.

### D-072 Label in each chat

**Decision.** The header of each chat has a small label. The label says "Normal: the server can read this chat" or "Sealed: only members can read this chat". One click gives more information.

**Reason.** The user always knows who can read a chat.

**Not selected.**

- An icon on sealed chats only.
- One explanation at the first join only.

### D-073 Private certificates

**Decision.** Some servers have a certificate that no public authority signed. For such a server, the application shows the certificate fingerprint one time and asks the user to accept it. The application remembers the fingerprint and gives a clear warning if it changes. A connection without TLS is possible only to the local computer, for development.

**Reason.** A server at home or in a private network works, and TLS stays on.

**Not selected.**

- Public certificates only.
- Connections without TLS after a warning.

### D-074 Proxy

**Decision.** The application can connect through a SOCKS5 proxy. The setting applies to all servers or to one server. This also makes Tor possible.

**Reason.** A user can hide the network address from a server. The work is small.

### D-075 Assurance

**Decision.** We write a threat model before the work on identity starts. All cryptography comes from libraries that experts examined: rustls and OpenMLS. Tests examine the security functions. The project does not plan an independent audit.

The documents must tell the reader that no independent audit exists. They must also tell what a server operator can see.

**Reason.** An independent audit has a high cost. The project accepts this limit and tells users the truth about it.

**Not selected.**

- An independent audit before version 1.0.
- An independent audit after version 1.0.

### D-076 No telemetry

**Decision.** Post collects no usage data and sends no crash reports. The application connects only to the servers that the user adds. The server directory (D-060) and the update check (D-090) are the only exceptions, and the user starts each of them.

**Reason.** The user controls each connection that the application makes.
