# Threat model

This document tells what Post protects, against whom, and what Post does not protect. It describes version 0.2. The project wrote it before the work on that version (decision record D-075), and examined it again when the version was complete. A version that changes a security function also changes this document.

No independent audit of Post exists. The project wrote this document itself.

## What Post protects

- **The account.** No other person can sign in as the user.
- **The messages on the network.** No person between the application and the server can read or change them.
- **The privacy between servers.** Two servers cannot find that two of their accounts belong to one person, unless the user shows it.
- **The membership.** Only a person with an invitation can make an account on a server.
- **The handle.** No other account can take the handle of a user.

## The parts and who controls them

| Part | Who controls it | What that person can do |
|---|---|---|
| The application and the profile directory | The user | All that the user can do |
| The server and its data | The operator, and each person with access to the server computer | Read the normal chats, see the handles and the keys of the members, see network addresses and connection times |
| The network | No one that Post trusts | See which computers connect, and how much data they exchange |

In version 0.2, a server has one channel and stores no messages. The server still sees each message when it sends the message to the members.

## Keys

- Each profile has one secret **seed** of 32 random bytes (D-063).
- Each server has its own **server key** (D-163).
- For each server, the application calculates an **account key** from the seed and the public server key. The server binds the handle to the public account key (D-062).

A person who has the seed has all accounts of the profile. A person who has one account key has only that account.

## The attackers

### An attacker who reads the network traffic

Each connection uses TLS 1.3 (D-061). The application refuses an earlier TLS version. The key exchange also resists an attacker who records traffic now and gets a quantum computer later (D-168).

One exception exists. A connection to the local computer can operate without TLS, for development (D-073). This traffic does not go out of the computer.

### An attacker who changes the network traffic

The application examines the certificate of the server.

- For a public certificate, the application uses the certificate check of the operating system.
- For a private certificate, the application shows the fingerprint one time and asks the user to accept it (D-073, D-164). The application remembers the fingerprint. If the certificate changes later, the application gives a warning and does not connect until the user decides.

The first connection to a server with a private certificate is the weak point. An attacker in the network can show a false certificate at that moment. The user is safe only if the user compares the fingerprint with the fingerprint that the operator gives. The interface tells the user to do this.

### A false server

A user can connect to a server that an attacker operates. The attacker wants one of these results:

- To sign in to a different server as the user.
- To find the account of the user on a different server.

Post prevents the two results with the sign-in procedure (D-163):

1. The application sends a random value and the address that it used.
2. The server signs the address and the random value with the server key. A server signs only its own addresses.
3. The application examines the signature. Only then does it calculate the account key from the seed and the server key.
4. The application signs the address, the server key, and the random values of the two sides with the account key.

The false server cannot show the key of a different server, because it cannot make the signature of step 2 for its own address. Thus the application calculates a key that only the false server knows. The signature of step 4 has no value for a different server, because it contains the address and the random values of this connection.

### An attacker who steals the server key

A person who copies the data of a server gets the server key. With that key, a false server at a different address can pass step 2. The application then calculates the account key of the real server and shows the public key to the false server. The attacker learns that this visitor has an account on the real server.

The attacker still cannot sign in to the real server as the user. The signature of step 4 contains the address of the false server, and the real server refuses it.

### An attacker who gets an invite link

A person with an invite link can make an account until the invitation stops. Post limits the damage:

- In the application, a new invitation stops after 7 days. The server owner can set a limit of uses and can cancel an invitation at any time (D-166).
- The server records which invitation each account used, and who made it (D-056).
- An owner link works one time (D-054).

The code in an invite link has 12 random characters. An attacker cannot guess it in a practical time.

### A member who wants the handle of a different person

The server binds each handle to one account key. After a handle change, the old handle stays reserved for 30 days (D-024). A handle contains only lower-case letters, digits, the hyphen, and the underscore (D-023), thus no handle looks almost the same as a different handle.

### A person with access to the server computer

This person controls the server fully. The person can do these things:

- Read each normal chat.
- Change or delete the data of the server.
- Make a different person the server owner (D-172).
- Operate a false server with the real server key and the real address.

This person cannot do these things:

- Get the seed of a user, or the account key of a user.
- Sign in to a different server as a user.

Sealed chats (version 0.10) add protection against this person for the content of a chat.

### A program or a person with access to the user account on the computer

In versions 0.2 to 0.10, the seed is in a file that only the user account can read (D-169). A program that runs as the user can read this file and take all accounts of the profile. So can a person who gets the computer without a lock, or a copy of the profile directory.

The keychain, the application passphrase, and the application lock come in version 0.11 (D-065, D-066). Until then, the lock and the disk encryption of the operating system are the protection.

### A user who loses the profile directory

No recovery phrase exists before version 0.11 (D-064). A user who loses the profile directory loses the accounts of that profile. The operator can then make the handle free. A server owner who loses the key can get the ownership back with a command on the server computer (D-172).

### An attacker who wants to stop the service

The server limits the length of a line and the number of lines that wait for a slow connection. It closes a connection that does not read. In version 0.2, the server does not limit the number of connections, and a connection has no time limit for the sign-in. Rate limits and bans come in version 0.9 (D-134, D-140). Post gives no full protection against an attacker who can send much traffic.

## What Post does not protect

- **The content of normal chats from the operator.** The server can read them.
- **Information about use.** The server sees who sends a message, when, and from which network address. The proxy function comes in version 0.16 (D-074).
- **A user who accepts a false fingerprint.**
- **The seed on a computer that an attacker controls.**
- **The service against a strong attack.**

## Cryptography

Post does not contain its own cryptographic algorithms (D-075). It uses these parts:

| Function | Algorithm | Library |
|---|---|---|
| Connection | TLS 1.3 with the X25519MLKEM768 or the X25519 key exchange | `rustls` with `aws-lc-rs` |
| Certificate check | The rules of the operating system | `rustls-platform-verifier` |
| Signatures of servers and accounts | Ed25519 | `ed25519-dalek` |
| Account key from the seed | HKDF with SHA-256 | `hkdf`, `sha2` |
| Random values | The generator of the operating system | `getrandom` |

The sign-in procedure is a design of the project. It uses only signatures, with a different label for each purpose. The [protocol specification](protocol/README.md) gives the exact bytes.

Verus proves two properties of these bytes (D-177):

- Two signed messages of the same type are equal only if the address, the keys, and the random values in them are equal.
- A message for the signature of the server is never equal to a message for the signature of the account.

The security of the procedure against an attacker has no formal proof. The [verification](verification.md) document gives the limits.

## Conditions

The protection depends on these conditions:

- The random generator of the operating system is good.
- The libraries are correct.
- The computer of the user does not run a program of an attacker.
- The user compares the fingerprint of a private certificate with the operator.
