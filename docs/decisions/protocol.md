# Protocol decisions

These records tell you how the Post protocol works. Read the [guide](README.md) first.

### D-045 A new protocol

**Decision.** Post has its own protocol. The protocol is small and public, and it has a version number. History, replies, reactions, edits, files, and optional end-to-end encryption are basic parts of it. The project also builds the server.

**Reason.** An IRC extension keeps the old limits of IRC. With Matrix or XMPP, Post loses control of its simplicity and speed.

**Not selected.**

- A superset of IRCv3.
- Matrix.
- XMPP.

### D-046 JSON lines

**Decision.** Each protocol message is one JSON object on one line. Files travel on a separate HTTPS path. Encrypted data of a sealed chat travels as base64 text.

**Reason.** A person can read the messages and find errors easily. A programmer can write a bot in any language in a short time.

**Not selected.**

- Text lines in the IRC style. Nested data makes a custom grammar necessary, and custom grammars cause parser errors.
- A binary format. Special tools are necessary to read it.

### D-047 Transport

**Decision.** The application connects to the server with a WebSocket over TLS on port 443. File transfer uses HTTPS on the same port. The protocol does not depend on the transport.

**Reason.** This connection passes firewalls and proxies. A server behind a reverse proxy is easy to operate.

**Not selected.**

- A TLS socket without WebSocket. Networks frequently block other ports.
- QUIC as the only transport. Some networks block UDP.

### D-048 Protocol version

**Decision.** The protocol has its own version number. This number is separate from the version of the application.

**Reason.** The application can change frequently while the protocol stays stable for other clients.

### D-049 Specification

**Decision.** The specification has three parts:

- A description in ASD-STE100.
- A JSON Schema for each message. The build makes the schemas from the Rust types.
- Example conversations.

The automatic checks fail if the code, the schemas, and the examples do not agree.

**Reason.** Other people can build a client with confidence. The specification cannot become different from the code.

**Not selected.**

- A description and examples that we check manually.
- The Rust code as the only reference.

### D-050 Invite links

**Decision.** An invite link is a normal https link, for example `https://chat.example.org/i/7Hq2kd`. In a browser, the server shows a small page. The page has the button "Open in Post" and tells where to get the application. A user can also paste the link into the application.

**Reason.** The link works in each program. It also helps a person who does not have the application.

**Not selected.**

- A custom `post://` link.
- A server address together with a code that the user types.

### D-155 Message style

**Date.** 2026-11-21.

**Decision.** A protocol message is a flat JSON object. The `type` field gives the kind of message as a word in lower case, for example `hello`, `send`, or `message`.

A client can put a `ref` number in a request. The answer of the server contains the same number.

An `error` message has a `code` for programs and a `text` for people. A code is in lower case. If a code has more than one word, hyphens join the words, for example `no-such-channel`.

**Reason.** A person can read a flat object easily. A JSON Schema describes it easily. The style keeps no habit of an older protocol.

**Not selected.**

- Verbs in the IRC style, for example `MSG` and `JOIN`.
- JSON-RPC 2.0. Each line has more levels and more fixed fields.

### D-156 Protocol version 0

**Date.** 2026-11-21.

**Decision.** The protocol version is 0 while the version of the application is below 1.0. Version 0 can change in each release, and it makes no promise to other clients. Protocol version 1 starts with version 1.0 of the application.

A client gives its protocol version in the `hello` message. The server refuses a version that it does not support.

**Reason.** The protocol must be free to change while we build the functions. An honest number tells the author of a different client that the protocol is not stable.
