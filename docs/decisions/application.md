# Application decisions

These records tell you how the desktop application looks and operates. Read the [guide](README.md) first.

### D-077 Toolkit

**Decision.** The application uses the iced toolkit for its interface. All code is in the Rust language.

**Reason.** These are basic requirements of the project. iced is a Rust toolkit for Windows, macOS, and Linux.

### D-078 Message layout

**Decision.** The timeline uses compact lines. The time, the handle, and the text are on one line. Messages in sequence from one person show the handle one time. There are no bubbles and no avatars in the timeline. A setting changes the distance between lines.

**Reason.** Many messages fit on the screen, as in an IRC client. Good typography makes the dense text easy to read.

**Not selected.**

- A layout with avatars and a header row.
- Bubbles.
- Two layouts with a switch.

### D-079 Navigation

**Decision.** One sidebar shows each server as a section that the user can open and close. The channels and direct messages of the server are below it, with unread counts. A member list is on the right side. A quick switcher (Ctrl+K) goes to each chat.

**Reason.** The user sees all communities at the same time, as in a good IRC client.

**Not selected.**

- A narrow column of server icons.
- One list with the chats of all servers mixed.

### D-080 Split panes

**Decision.** A user can split the window and see more than one chat at the same time. Version 1.0 contains this function.

**Reason.** A user who follows many channels does not have to switch all the time.

**Not selected.**

- One chat at a time in version 1.0, with panes later.
- One chat at a time always.

### D-081 Visual style

**Decision.** The style has the name "ink and paper". Light mode has a warm off-white background. Dark mode has a deep ink-gray background. Thin lines divide the areas, and the layout has much space. There are no gradients, no shadows, and few icons. Light and dark mode obey the setting of the operating system.

**Reason.** The style is calm and puts the text first.

**Not selected.**

- Plain gray with the accent color of the operating system.
- A style with rounded cards and many colors.

### D-082 Accent color

**Decision.** The style has one accent color for links, selection, and unread marks. The default is airmail blue. A setting offers a small set of other accent colors.

**Reason.** Blue reads as "link" and keeps red free for errors. A small set of tested colors keeps the contrast good.

**Not selected.**

- One fixed accent color.

### D-083 Custom themes

**Decision.** A user can load a theme file that sets all colors. Version 1.0 contains this function. The built-in themes obey the contrast rule (D-091). A custom theme is the responsibility of its author.

**Reason.** Theme files are a tradition of IRC clients.

**Not selected.**

- Custom themes after version 1.0.
- Built-in themes only.

### D-084 Fonts

**Decision.** The application contains its own open fonts: Inter for text and JetBrains Mono for code.

**Reason.** Post looks the same, and equally good, on each system. Inter is neutral and clear at small sizes.

**Not selected.**

- The fonts of each operating system.
- A monospace font for all text.
- IBM Plex Sans and IBM Plex Mono.
- Atkinson Hyperlegible Next and Atkinson Hyperlegible Mono.

**History.** At first, Inter and JetBrains Mono were only candidates. On 2026-11-21, the project selected them before the work on the first window.

### D-085 Emoji

**Decision.** The application contains one open emoji font, Noto Color Emoji.

**Reason.** Emoji look the same on each system and are always available. Some Linux systems have no color emoji font.

**Not selected.**

- The emoji font of each operating system.

**Open point.** We must make sure that iced can show this font.

### D-086 Commands and keyboard

**Decision.** Post has slash commands with completion, for example `/join`, `/msg`, `/me`, and `/topic`. The user can do each task with the keyboard. Each command also has a button or a menu item. A bot can add commands to the `/` menu (D-128).

**Reason.** An expert works quickly, and a new user never has to type a command.

**Not selected.**

- Buttons and menus only.
- Aliases and scripts that a user defines.

### D-087 Settings

**Decision.** A settings screen changes the settings. One TOML file with documentation holds them. A user can edit the file, copy it to a different computer, or keep it in version control. The application applies a change in the file immediately.

**Reason.** The screen helps each user. The file helps an expert.

**Not selected.**

- A settings screen with a hidden store.
- A file without a settings screen.

### D-088 When the window closes

**Decision.** Post obeys the convention of each system. On macOS, Post continues to run in the Dock when the user closes the window. On Windows and Linux, Post stops when the user closes the window. A setting lets Post continue to run in the system tray.

**Reason.** The behavior does not surprise the user. No hidden process runs unless the user asks for it.

**Not selected.**

- The system tray by default.
- No tray function.

### D-089 First start

**Decision.** At the first start, the user sees one screen. The user pastes an invite link or types a server address, then selects a handle. The application makes the identity silently. It offers the recovery phrase later. The user can also open the server directory (D-060).

**Reason.** A new user sends the first message after two steps.

**Not selected.**

- A setup with many steps that shows the recovery phrase first.

### D-090 Update check

**Decision.** A setting lets the application ask GitHub for the latest version. The setting is off by default. When it is on and a new version exists, the application shows a quiet note. Post does not install an update by itself.

**Reason.** The application contacts no third party unless the user selects that. A user of a portable version can still learn about an update.

**Not selected.**

- Package managers only.
- A check that is on by default.
- Automatic installation.

### D-091 Accessibility

**Decision.** Version 1.0 obeys these rules:

- The user can do each task with the keyboard.
- Colors agree with the WCAG AA contrast level.
- The user can set the text size from 80% to 200%.
- The user can decrease motion.

Screen reader support depends on the iced toolkit. We add it when the toolkit makes it possible.

**Reason.** Post must be usable for as many people as possible.

**Not selected.**

- Screen reader support as a condition for version 1.0.
- Basic keyboard support only.

**Open point.** We must examine the state of screen reader support in iced.

### D-092 Languages

**Decision.** Each interface text comes from a Fluent message catalog. Post has English and Latin American Spanish (es-419) text. A commit that adds or changes an English text also adds or changes the Spanish text. The application uses the language of the operating system, and a setting changes it. The repository documents stay in English.

**Reason.** A translation is a new file, not a change to each screen. Two languages from the start prove that the catalog works.

**Not selected.**

- Text in the source code.
- Spanish documents. ASD-STE100 is an English standard, and two copies can disagree.

### D-093 Messages without a connection

**Decision.** When the connection is down, the user can still write. A new message shows with the mark "pending". The application sends the pending messages in sequence when the connection comes back. The user can cancel a pending message. The application connects again automatically.

**Reason.** The user can continue to work on a bad network.

**Not selected.**

- An input field that is not available without a connection.
- An error with a button to try again.

### D-094 Protocol console

**Decision.** A developer setting opens a console. The console is a panel at the bottom of the window. It shows the JSON lines that the application sends and receives, each with its time and its direction. A field in the console sends a JSON line that the developer types, in the connection that is already open. The server examines this line as it examines each line. The console does not show the content of sealed chats. The setting is off by default.

**Reason.** The protocol is easy to learn, and a programmer of a bot or a client can find errors. A sign-in needs a signature from the account key (D-163), thus a person cannot try the protocol with a simple tool.

**Not selected.**

- A console that only shows the lines.
- A console in a separate window.

**History.** At first, the console only showed the lines. On 2026-11-21, the project added the field that sends a line.

### D-095 Icon

**Decision.** The icon of the application is a round postmark with wave lines, in ink on paper.

**Reason.** A postmark is different from the many envelope and speech-bubble icons. It agrees with the visual style.

**Not selected.**

- An envelope.
- The letter P.
- A postage stamp.

**Open point.** The first form of the icon is on the website (D-175). The project must examine it before the application uses it.

### D-096 Words in the interface

**Decision.** The interface uses plain words: server, channel, direct message, operator, moderator. It uses one word from the post office: "sealed".

**Reason.** Plain words are easy to understand and to translate.

### D-150 Sidebar order

**Decision.** In the sidebar, channels sort by name, and direct messages sort by the latest activity. A user can mark a chat as a favorite. Favorites stay at the top. A user can also mute or hide a chat.

**Reason.** The user maintains nothing, and no chat moves by surprise. The chats that the user uses most are easy to reach.

**Not selected.**

- A manual order by drag.
- An automatic order with no favorites.

### D-151 Key bindings

**Decision.** Each keyboard shortcut has a default, and a user can change it. The settings screen and the settings file show the shortcuts.

**Reason.** IRC users expect this function. It also solves a conflict with a keyboard layout or with a shortcut of the operating system.

**Not selected.**

- Fixed shortcuts.

### D-152 Appearance settings

**Decision.** A user can change these parts of the appearance:

- The font for text and the font for code. The fonts that Post contains are the default.
- The time format: one setting selects the 12-hour format (D-161), and one setting adds the seconds.
- The handle colors: a setting turns them off.
- The images: a setting shows each image as a small card until the user clicks it.

**Reason.** A different font helps some people to read. The other settings let a user make the timeline as plain as the user wants.

### D-153 Sounds and automatic start

**Decision.** Post can play one soft sound for a notification. Post can start when the user signs in to the computer. These two settings are off by default.

**Reason.** The functions help a user who keeps Post open all day. The defaults stay quiet.

### D-154 Links

**Decision.** A click on a link opens the link in the default browser. The application does not ask first.

**Reason.** A message always shows the full address of a link (D-010), thus the user sees the target before the click.

**Not selected.**

- A question at the first click for each site.
- A question at each click.

### D-160 Window of the first slice

**Date.** 2026-11-21.

**Decision.** The window of version 0.1 has the real visual style (D-081), the fonts (D-084), and compact lines (D-078). It has two screens: a connect screen and one chat screen. The chat screen has a header, the timeline, and the input field. The sidebar and the member list (D-079) come in version 0.3.

**Reason.** The first demonstration already looks like Post. The slice has no empty parts of the layout.

**Not selected.**

- The full layout with a sidebar and a member list from the start.
- A plain window with the default look of the toolkit.

### D-161 Time format

**Date.** 2026-11-21.

**Decision.** The timeline shows the local time of the user in the 24-hour format, for example 14:02. One setting selects the 12-hour format. A different setting adds the seconds (D-152).

**Reason.** A time with a constant width keeps the compact lines in a straight column. The 24-hour format is an IRC tradition.

**Not selected.**

- The convention of the operating system. Separate code for each system is necessary.
- The 24-hour format with no setting.

### D-165 Profile selection

**Date.** 2026-11-21.

**Decision.** One window shows one profile (D-147). The application makes the first profile silently. At the start, it opens the profile that was last in use. The command `post --profile <name>` opens a specific profile, thus two windows can show two profiles at the same time.

A Profiles screen makes, renames, deletes, and switches profiles. Before the application deletes a profile, it tells the user what the user loses, and the user must type the name of the profile. A user cannot delete the profile that is open.

**Reason.** A user who wants one profile never sees the function. The parts of a life stay fully apart. A deleted seed cannot come back, thus a deletion must not occur by mistake.

**Not selected.**

- All profiles in one window, with a label on each server.
- A selection on the command line only.
- One confirmation question before a deletion.
- No deletion in the application before the recovery phrase exists.

### D-167 First settings screen

**Date.** 2026-11-21.

**Decision.** Version 0.2 contains the settings file and the first Settings screen (D-087). Each profile has its own settings file (D-147). The screen has these settings:

- The language (D-092).
- The time format (D-161).
- The text size, from 80% to 200% (D-091).
- The protocol console (D-094).

Each later version adds its own settings. The other appearance settings come in version 0.15: the distance between lines (D-078), the handle colors (D-152), and the accent color (D-082).

**Reason.** Each function must have a button (D-086). The toolkit can scale the full interface, thus the text size costs little, and the tests of each later screen can use it.

**Not selected.**

- A settings file with no screen in version 0.2.
- All appearance settings in version 0.2.
- All appearance settings in version 0.15.

### D-171 Servers screen

**Date.** 2026-11-21.

**Decision.** Until the sidebar comes in version 0.3 (D-079), the window shows one server at a time. At the start, the application connects to the server that was last in use and opens its chat. A Servers screen lists the servers of the profile. On this screen, the user adds a server with an invite link or an address, and removes a server. A new profile starts on this screen.

**Reason.** A user who comes back sees the chat immediately.

**Not selected.**

- The Servers screen at each start.

**History.** In version 0.3, the sidebar replaces the Servers screen (D-079). The application then connects to each server of the profile at the start (D-187).

### D-182 Model of the split panes

**Date.** 2026-11-22.

**Decision.** A user can split a pane to the right or down (D-080). A new pane can split again. The user moves the line between two panes to change their sizes. Each pane shows one chat.

**Reason.** This is the model of a code editor and of a terminal multiplexer, and many users know it. The toolkit has a widget for it.

**Not selected.**

- Columns only.
- A maximum of two panes.

### D-183 Order of the member list

**Date.** 2026-11-22.

**Decision.** The member list (D-079) shows the owner of the channel first. The moderators come next, from version 0.9. The other members come last. In each group, the members sort by name. A dot shows the presence of each member (D-019).

**Reason.** This is the order of an IRC client. The roles in a channel are easy to see.

**Not selected.**

- An order by presence: online, away, and offline.
- One list by name only.

### D-186 Place of the favorites

**Date.** 2026-11-22.

**Decision.** The application keeps the favorites (D-150) in a file in the profile, on the computer of the user. The server does not get them.

**Reason.** The function needs no protocol and no data on the server. The operator cannot see which chats a person marked.

**Not selected.**

- The favorites on the server, the same on each device of the user.

### D-187 Connections at the start

**Date.** 2026-11-22.

**Decision.** At the start, the application connects to each server of the profile. If the user disconnects a server by hand, that server stays disconnected until the user connects it again. This rule also applies after the next start.

**Reason.** The sidebar shows all communities immediately, as an IRC client with automatic connect does.

**Not selected.**

- A connection only to the server that was last in use.
- No connection at the start.

### D-188 Title of a server

**Date.** 2026-11-22.

**Decision.** The address of a server is its name. The sidebar shows the address as the title of the section of the server (D-079). A server has no other name.

**Reason.** No person can give a server a false name. People have no display names for the same reason (D-025).

**Not selected.**

- A name that the operator sets.
- A name that each user gives to a server in the application.
