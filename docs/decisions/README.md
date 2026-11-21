# Decision records

This directory contains the design decisions for Post. Each record gives one decision and its reason.

Read the [design philosophy](../philosophy.md) first. The philosophy gives the ideas. The records apply the ideas.

## How to read a record

Each record has an identifier (for example D-012) and a title. A record has these parts:

- **Date** gives the date of the record. Some records do not have this part.
- **Decision** tells you what Post does or what the project does.
- **Reason** tells you why.
- **Not selected** gives the other options. Some records do not have this part.
- **Open point** gives a fact that we must examine. Some records do not have this part.
- **History** tells you about an earlier form of the decision. Some records do not have this part.

If a decision is open, the **Decision** part says so.

A record without a **Date** part has the date 2026-11-20.

## How to change a decision

1. Change the record. Add the date of the change and the reason for it.
2. If the change touches a principle, change the philosophy document too.
3. Put these changes in the same commit as the related code change.

A new record gets the next free number. Do not use a number again.

## Files

| File | Subject | Records |
|---|---|---|
| [product.md](product.md) | What Post does for a user | D-001 to D-036, D-122 to D-130 |
| [voice.md](voice.md) | Voice channels | D-037 to D-044, D-131, D-132 |
| [protocol.md](protocol.md) | The Post protocol | D-045 to D-050, D-155, D-156 |
| [server.md](server.md) | The server and its operation | D-051 to D-060, D-133 to D-146, D-164, D-166, D-172 |
| [identity-and-security.md](identity-and-security.md) | Accounts, keys, and encryption | D-061 to D-076, D-147 to D-149, D-163, D-169 |
| [application.md](application.md) | The desktop application | D-077 to D-096, D-150 to D-154, D-160, D-161, D-165, D-167, D-171 |
| [engineering.md](engineering.md) | How we build Post | D-097 to D-112, D-157 to D-159, D-168, D-170, D-173, D-174 |
| [distribution-and-community.md](distribution-and-community.md) | Releases and the project community | D-113 to D-121, D-162 |

## Index

### Product

- D-001 Independent servers
- D-002 Channels and direct messages
- D-003 Scope of version 1.0
- D-004 History and retention
- D-005 No separate timer for messages
- D-006 Kinds of channel
- D-007 Channel topic
- D-008 Direct messages
- D-009 Edit and delete
- D-010 Text format
- D-011 Message length
- D-012 Reactions
- D-013 Code blocks
- D-014 Mentions and keyword highlights
- D-015 Message links
- D-016 No forward function
- D-017 Saved messages
- D-018 Awareness signals
- D-019 Presence
- D-020 Block a person
- D-021 Notifications
- D-022 Notification text
- D-023 Names
- D-024 Handle change
- D-025 Profile card
- D-026 Welcome text
- D-027 Export
- D-028 Account deletion
- D-029 Bots and other clients
- D-030 Webhooks
- D-031 Link previews
- D-032 Public web view
- D-033 No IRC gateway
- D-034 Things that Post will not have
- D-035 Functions that are not in Post
- D-036 Description of Post
- D-122 Version 1.0 is complete
- D-123 Media in the timeline
- D-124 Timeline comforts
- D-125 Mentions view
- D-126 Drafts
- D-127 Find a person
- D-128 Bot commands
- D-129 Limits for bots
- D-130 Help for bot authors

### Voice

- D-037 Voice is a primary function
- D-038 Voice channels
- D-039 Voice in a direct message
- D-040 Sound only
- D-041 Voice security
- D-042 Voice transport
- D-043 Microphone mode
- D-044 Voice functions
- D-131 Voice quality
- D-132 Voice sounds

### Protocol

- D-045 A new protocol
- D-046 JSON lines
- D-047 Transport
- D-048 Protocol version
- D-049 Specification
- D-050 Invite links
- D-155 Message style
- D-156 Protocol version 0

### Server

- D-051 Storage
- D-052 Operation
- D-053 TLS certificates
- D-054 First operator
- D-055 Who can join a server
- D-056 Who can invite
- D-057 Who can make a channel
- D-058 Roles
- D-059 Server logs
- D-060 Server directory
- D-133 Where the server settings are
- D-134 Server limits
- D-135 Function switches
- D-136 No default channels
- D-137 Limits for new accounts
- D-138 Moderation tools
- D-139 No automatic moderation
- D-140 Server bans
- D-141 What an operator can read
- D-142 Operator power over channels
- D-143 Reserved names
- D-144 Lockdown switch
- D-145 Server status view
- D-146 Operator tools
- D-164 Own certificate
- D-166 Invitation limits
- D-172 New owner link

### Identity and security

- D-061 Security model
- D-062 Identity
- D-063 One seed and one key for each server
- D-064 Recovery and more devices
- D-065 Key storage
- D-066 Application lock
- D-067 Data on the device
- D-068 Encryption for sealed chats
- D-069 How a chat becomes sealed
- D-070 Key verification
- D-071 Earlier history in a sealed chat
- D-072 Label in each chat
- D-073 Private certificates
- D-074 Proxy
- D-075 Assurance
- D-076 No telemetry
- D-147 Application profiles
- D-148 Account proof
- D-149 Operator control of sealed chats
- D-163 Server key
- D-169 Seed file

### Application

- D-077 Toolkit
- D-078 Message layout
- D-079 Navigation
- D-080 Split panes
- D-081 Visual style
- D-082 Accent color
- D-083 Custom themes
- D-084 Fonts
- D-085 Emoji
- D-086 Commands and keyboard
- D-087 Settings
- D-088 When the window closes
- D-089 First start
- D-090 Update check
- D-091 Accessibility
- D-092 Languages
- D-093 Messages without a connection
- D-094 Protocol console
- D-095 Icon
- D-096 Words in the interface
- D-150 Sidebar order
- D-151 Key bindings
- D-152 Appearance settings
- D-153 Sounds and automatic start
- D-154 Links
- D-160 Window of the first slice
- D-161 Time format
- D-165 Profile selection
- D-167 First settings screen
- D-171 Servers screen

### Engineering

- D-097 Repository layout
- D-098 Build sequence
- D-099 First slice
- D-100 Roadmap
- D-101 Tests first
- D-102 Automatic checks
- D-103 When the checks run
- D-104 Coverage
- D-105 Performance
- D-106 Comments
- D-107 Documents
- D-108 Commits
- D-109 Version numbers
- D-110 Dependencies
- D-111 Toolchain
- D-112 Work procedure
- D-157 Network libraries
- D-158 Details of the first slice
- D-159 Program names
- D-168 Libraries for keys, TLS, and storage
- D-170 Performance budgets
- D-173 Content of version 0.2
- D-174 Details of version 0.2

### Distribution and community

- D-113 License
- D-114 Source code host
- D-115 Platforms
- D-116 Packages
- D-117 Code signature
- D-118 Release builds
- D-119 Application identifier
- D-120 Contributions
- D-121 Code of conduct
- D-162 Security reports
