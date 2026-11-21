# Voice decisions

These records tell you how voice works in Post. Read the [guide](README.md) first.

A separate round of design questions sets more details before the voice work starts.

### D-037 Voice is a primary function

**Decision.** Post has voice in the manner of TeamSpeak and Mumble. Voice is a primary function of Post, not an addition. Version 1.0 contains it. We build voice after the text functions and the sealed chats.

**Reason.** A community talks and writes. Voice of low quality is worse than no voice, thus we design it carefully.

**Not selected.**

- Voice as the primary subject of version 2.0.
- Voice directly after the first slice.

### D-038 Voice channels

**Decision.** A voice channel is a kind of channel. The sidebar shows the users in each voice channel and shows who talks. One click lets a user go in, and one click lets the user go out. No device rings. Each voice channel has a small text chat.

**Reason.** A user sees immediately where people talk. The user goes in when the user is ready.

**Not selected.**

- A voice button in each text channel.
- A separate area for voice rooms.

### D-039 Voice in a direct message

**Decision.** Each of the two persons can open voice in a direct message. The other person sees this and can go in at any time. There is no ring and no list of missed calls.

**Reason.** A private talk is quick to start, and it does not interrupt a person.

**Not selected.**

- Calls that ring.
- Voice in channels only.

### D-040 Sound only

**Decision.** A voice channel carries sound only. Post has no video and no screen share.

**Reason.** All the work goes into clear sound with a small delay. A person who must share a screen uses a tool for meetings.

**Not selected.**

- Screen share in version 1.0.
- Screen share and camera video.

### D-041 Voice security

**Decision.** Voice obeys the same rule as text (D-061). Post always encrypts voice between the application and the server. In a normal channel, the server relays the sound and can listen. In a sealed channel, voice has end-to-end encryption with keys from the MLS group of the channel. The server then relays only ciphertext. The label in the chat header shows the state (D-072).

**Reason.** One rule for text and voice is easy to understand.

**Not selected.**

- End-to-end encryption for all voice.
- Encryption to the server only.

### D-042 Voice transport

**Decision.** Voice travels as QUIC datagrams on UDP port 443. The connection uses the certificate of the server. The server sends the sound of each speaker to the listeners and does not mix it. If the network blocks UDP, voice uses the normal connection of the application.

**Reason.** The encryption comes from a TLS 1.3 library that experts examined. We do not design packet encryption.

**Not selected.**

- WebRTC. It is a very large system for a simple server.
- A custom UDP packet format. Its encryption then becomes our own design.

### D-043 Microphone mode

**Decision.** When a user goes into a voice channel for the first time, a short setup starts. The user selects voice activation or push-to-talk and does a sound test.

**Reason.** The user makes a clear selection, and no sound goes out by accident.

**Not selected.**

- Voice activation as a silent default.
- Push-to-talk as a silent default.

### D-044 Voice functions

**Decision.** Voice has these functions:

- Go in and go out with one click.
- An indicator that shows who talks.
- Mute the microphone, and stop all sound.
- A volume control for each person.
- Voice activation or push-to-talk.
- Noise suppression and echo cancellation.
- Selection of the sound devices, with a test.
- Moderator tools: mute a person, or remove a person from the channel.
- Announcement mode: only the owner and the moderators talk, and the other members listen.
- Priority speaker: a moderator holds a key, and the volume of all other persons decreases.
- An away room: a person who is idle for a set time moves to a quiet channel automatically.

Post does not record voice. The documents tell the user that a listener can record with other tools.

**Reason.** Clear sound and a small delay come first. Announcement mode uses a concept that text channels already have. Without a record function, people talk more freely, and the server holds no recordings.

**Not selected.**

- Whisper to selected persons. It makes lists, more keys, and more interface necessary.
- Temporary channels.
- A record function with a clear sign.

### D-131 Voice quality

**Decision.** Voice uses a default quality that is good for speech on a normal connection. The operator sets the maximum bitrate and the maximum number of persons in one voice channel. A channel owner can select "high quality" for a channel in these limits, for example for music. Users get no codec controls.

**Reason.** Most people do not know which codec values to select. An operator can still control the load on the server.

**Not selected.**

- Detailed codec settings for each channel.
- One fixed quality for all servers.

### D-132 Voice sounds

**Decision.** Post plays a short, soft sound when a person goes into or out of the voice channel of the user. It also plays a sound when the user mutes the microphone. A setting turns each sound off.

**Reason.** The user knows what occurs while the user looks at a different window.

**Not selected.**

- No sounds.
- Spoken notices, as in TeamSpeak.
