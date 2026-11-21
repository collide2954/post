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
