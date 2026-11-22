use std::borrow::Cow;
use std::path::PathBuf;

use iced::keyboard::key::Named;
use iced::{Pixels, Settings, Size};
use iced_test::selector::{Candidate, Selector};
use iced_test::{Simulator, selector};
use jiff::tz::TimeZone;
use post_app::app::start::StartProblem;
use post_app::app::{App, Message, Page, chat, console, profiles, servers, settings};
use post_app::i18n::{Language, Strings};
use post_app::palette::Mode;
use post_app::settings::Clock;
use post_app::view::{self, Window};
use post_client::{
    Direction, Disconnect, Event, Fingerprint, KnownServer, Line, Link, NameProblem, Outgoing,
    ProfileEntry, ProfileId,
};
use post_proto::{
    ChannelKind, ChannelName, ClientMessage, Code, ErrorCode, Invitation, Key, ListedChannel, Name,
    Role, ServerMessage, Timestamp,
};

const SCREENSHOT_VARIABLE: &str = "POST_SCREENSHOTS";

type Outbox = Box<dyn FnMut() -> Option<ClientMessage>>;

fn sent_message(outgoing: Outgoing) -> ClientMessage {
    match outgoing {
        Outgoing::Message(message) => message,
        Outgoing::Line(line) => panic!("a typed line and not a message: {line}"),
    }
}

fn name(text: &str) -> Name {
    Name::parse(text).expect(text)
}

fn time(clock: &str) -> Timestamp {
    Timestamp::parse(&format!("2026-11-21T{clock}.000Z")).expect(clock)
}

fn window(app: App, language: Language) -> Window {
    Window {
        app,
        strings: Strings::new(language),
        zone: TimeZone::UTC,
        settings_file: None,
    }
}

fn simulator(window: &Window) -> Simulator<'_, Message> {
    simulator_of_size(window, Size::new(900.0, 600.0))
}

fn simulator_of_size(window: &Window, size: Size) -> Simulator<'_, Message> {
    let settings = Settings {
        default_font: view::TEXT_FONT,
        default_text_size: Pixels(14.0),
        fonts: view::FONTS
            .iter()
            .map(|font| Cow::Borrowed(*font))
            .collect(),
        ..Settings::default()
    };
    Simulator::with_size(settings, size, view::view(window))
}

fn new_app(mode: Mode) -> App {
    App::new(mode, Vec::new())
}

fn typed(app: &mut App, address: &str, name: &str) {
    let address = servers::Message::AddressChanged(address.to_owned());
    app.update(Message::Servers(address));
    let name = servers::Message::NameChanged(name.to_owned());
    app.update(Message::Servers(name));
}

fn connect_app(mode: Mode) -> App {
    let mut app = new_app(mode);
    typed(&mut app, "localhost:7678", "ana");
    app
}

fn list_app(mode: Mode) -> App {
    let server = |address: &str, tls| KnownServer {
        address: address.to_owned(),
        key: Key::from_bytes([1; 32]),
        name: name("ana"),
        tls,
        certificate: None,
    };
    let known = vec![
        server("chat.example.org:443", true),
        server("localhost:8443", true),
    ];
    let mut app = App::new(mode, known);
    app.update(Message::Servers(servers::Message::Cancel));
    app
}

fn certificate_app(mode: Mode, changed: bool) -> App {
    let mut app = new_app(mode);
    typed(&mut app, "https://home.example.org:7678", "ana");
    app.update(Message::Servers(servers::Message::Connect));
    let attempt = app.target().expect("a connection is in progress").attempt;
    let fingerprint = Fingerprint::of_certificate(b"abc");
    let reason = if changed {
        Disconnect::ChangedCertificate(fingerprint)
    } else {
        Disconnect::UnknownCertificate(fingerprint)
    };
    app.update(Message::Connection(attempt, Event::Disconnected(reason)));
    app
}

fn connecting_app(mode: Mode) -> App {
    let mut app = connect_app(mode);
    app.update(Message::Servers(servers::Message::Connect));
    app
}

fn chat_app(mode: Mode) -> (App, Outbox) {
    chat_app_as(mode, Role::Member)
}

fn signed_in_app(mode: Mode, role: Role) -> (App, Outbox) {
    let mut app = connecting_app(mode);
    let attempt = app.target().expect("a connection is in progress").attempt;
    let (link, mut messages) = Link::channel(name("ana"), role, Key::from_bytes([1; 32]), 8);
    app.update(Message::Connection(attempt, Event::Connected(link)));
    (
        app,
        Box::new(move || messages.try_recv().ok().map(sent_message)),
    )
}

fn no_channel_app(mode: Mode) -> (App, Outbox) {
    let (mut app, outbox) = signed_in_app(mode, Role::Member);
    receive(&mut app, ServerMessage::Ready {});
    app.update(Message::Chat(chat::Message::Close));
    (app, outbox)
}

fn chat_app_as(mode: Mode, role: Role) -> (App, Outbox) {
    let (mut app, outbox) = signed_in_app(mode, role);
    let message = |from: &str, clock: &str, text: &str| ServerMessage::Message {
        reference: None,
        channel: "general".to_owned(),
        from: name(from),
        time: time(clock),
        text: text.to_owned(),
    };
    let received = [
        channel_state("general"),
        ServerMessage::Ready {},
        ServerMessage::Join {
            channel: "general".to_owned(),
            name: name("ben"),
            time: time("14:02:05"),
        },
        message("ana", "14:02:07", "Hello"),
        message("ben", "14:03:10", "Hi. Does it work?"),
        message("ana", "14:03:20", "Yes."),
        message(
            "ana",
            "14:03:31",
            "Two people, one channel, and a server on this computer.",
        ),
        ServerMessage::Leave {
            reference: None,
            channel: "general".to_owned(),
            name: name("ben"),
            time: time("14:05:00"),
        },
    ];
    for message in received {
        receive(&mut app, message);
    }
    (app, outbox)
}

fn receive(app: &mut App, message: ServerMessage) {
    let attempt = app.target().expect("a connection").attempt;
    app.update(Message::Connection(attempt, Event::Received(message)));
}

fn code(text: &str) -> Code {
    Code::parse(text).expect(text)
}

fn invitation(text: &str, end: Option<&str>, limit: Option<u32>, used: u32) -> Invitation {
    Invitation {
        code: code(text),
        link: format!("https://chat.example.org/i/{text}"),
        created: time("14:02:00"),
        end: end.map(|end| Timestamp::parse(end).expect(end)),
        uses: limit,
        used,
    }
}

fn two_invitations() -> Vec<Invitation> {
    vec![
        invitation("k3Wm8sTz2LpQ", Some("2026-11-28T14:02:00.000Z"), Some(5), 2),
        invitation("7Hq2kdX9mPq4", None, None, 0),
    ]
}

fn invitations_app(mode: Mode, list: Option<Vec<Invitation>>) -> (App, Outbox) {
    let (mut app, outbox) = chat_app_as(mode, Role::Owner);
    let open = chat::Message::Open(chat::Panel::Invitations);
    app.update(Message::Chat(open));
    if let Some(list) = list {
        let reference = Some(1);
        receive(&mut app, ServerMessage::Invitations { reference, list });
    }
    (app, outbox)
}

fn account_app(mode: Mode) -> (App, Outbox) {
    let (mut app, outbox) = chat_app(mode);
    app.update(Message::Chat(chat::Message::Open(chat::Panel::Account)));
    (app, outbox)
}

fn settings_app(mode: Mode) -> App {
    let mut app = new_app(mode);
    app.update(Message::Open(Page::Settings));
    app
}

fn profile(number: u32) -> ProfileId {
    ProfileId::from_number(number)
}

fn profiles_app(mode: Mode) -> App {
    let mut app = new_app(mode);
    app.update(Message::Open(Page::Profiles));
    let entry = |number, name: &str| ProfileEntry {
        id: profile(number),
        name: name.to_owned(),
    };
    let list = vec![entry(1, "main"), entry(2, "work")];
    let listed = profiles::Message::Listed(list, profile(1));
    app.update(Message::Profiles(listed));
    app
}

fn profiles_app_with(mode: Mode, messages: Vec<profiles::Message>) -> App {
    let mut app = profiles_app(mode);
    for message in messages {
        app.update(Message::Profiles(message));
    }
    app
}

fn profile_message(window: &Window, text: &str) -> profiles::Message {
    match clicked(window, text).as_slice() {
        [Message::Profiles(message)] => message.clone(),
        other => panic!("{text}: {other:?}"),
    }
}

const SENT_LINE: &str = r#"{"type":"send","ref":1,"channel":"general","text":"Hello"}"#;
const RECEIVED_LINE: &str = r#"{"type":"message","ref":1,"channel":"general","from":"ana","time":"2026-11-21T14:02:07.120Z","text":"Hello"}"#;

fn with_console(app: &mut App) {
    let on = settings::Message::ConsoleToggled(true);
    app.update(Message::Settings(on));
}

fn console_app(mode: Mode) -> (App, Outbox) {
    let (mut app, outbox) = chat_app(mode);
    with_console(&mut app);
    let attempt = app.target().expect("a connection").attempt;
    let lines = [
        (Direction::Sent, "2026-11-21T14:02:07.004Z", SENT_LINE),
        (
            Direction::Received,
            "2026-11-21T14:02:07.120Z",
            RECEIVED_LINE,
        ),
    ];
    for (direction, time, text) in lines {
        let line = Line {
            time: Timestamp::parse(time).expect(time),
            direction,
            text: text.to_owned(),
        };
        app.update(Message::Connection(attempt, Event::Line(line)));
    }
    (app, outbox)
}

fn settings_file(app: &mut App, text: &str) {
    let changed = settings::Message::FileChanged(text.to_owned());
    app.update(Message::Settings(changed));
}

fn clicked(window: &Window, text: &str) -> Vec<Message> {
    let mut interface = simulator(window);
    interface
        .click(text)
        .unwrap_or_else(|error| panic!("{text}: {error}"));
    interface.into_messages().collect()
}

#[test]
fn the_name_field_gets_the_keys_that_the_user_types() {
    let window = window(new_app(Mode::Light), Language::English);
    let mut interface = simulator(&window);

    interface
        .click(selector::id(view::NAME_FIELD))
        .expect("the connect screen has a name field");
    interface.typewrite("a");

    let messages: Vec<Message> = interface.into_messages().collect();
    assert!(
        matches!(messages.as_slice(), [Message::Servers(servers::Message::NameChanged(name))] if name == "a"),
        "{messages:?}"
    );
}

#[test]
fn the_address_field_gets_the_keys_that_the_user_types() {
    let window = window(new_app(Mode::Light), Language::English);
    let mut interface = simulator(&window);

    interface
        .click(selector::id(view::ADDRESS_FIELD))
        .expect("the servers screen has an address field");
    interface.typewrite("l");
    assert!(interface.find("Servers").is_err());

    let messages: Vec<Message> = interface.into_messages().collect();
    assert!(
        matches!(messages.as_slice(), [Message::Servers(servers::Message::AddressChanged(address))] if address == "l"),
        "{messages:?}"
    );
}

#[test]
fn the_servers_screen_lists_the_servers_of_the_profile() {
    let window = window(list_app(Mode::Light), Language::English);
    let mut interface = simulator(&window);

    for text in [
        "Servers",
        "chat.example.org",
        "https://localhost:8443",
        "Add a server",
    ] {
        interface
            .find(text)
            .unwrap_or_else(|error| panic!("{text}: {error}"));
    }
}

#[test]
fn the_buttons_of_a_server_open_it_and_remove_it() {
    let mut app = list_app(Mode::Light);
    app.update(Message::Servers(servers::Message::Forget(0)));
    let window = window(app, Language::English);

    let mut interface = simulator(&window);
    interface
        .click("Open")
        .expect("a server has an open button");
    let messages: Vec<Message> = interface.into_messages().collect();
    assert!(
        matches!(
            messages.as_slice(),
            [Message::Servers(servers::Message::Open(0))]
        ),
        "{messages:?}"
    );

    let mut interface = simulator(&window);
    interface
        .click("Remove")
        .expect("a server has a remove button");
    let messages: Vec<Message> = interface.into_messages().collect();
    assert!(
        matches!(
            messages.as_slice(),
            [Message::Servers(servers::Message::Forget(0))]
        ),
        "{messages:?}"
    );
}

#[test]
fn the_window_asks_about_a_certificate_that_no_authority_signed() {
    let window = window(certificate_app(Mode::Light, false), Language::English);
    let mut interface = simulator(&window);

    for text in [
        "home.example.org:7678",
        "This server has a certificate that no public authority signed.",
        "Fingerprint",
        "ba78 16bf 8f01 cfea 4141 40de 5dae 2223",
        "b003 61a3 9617 7a9c b410 ff61 f200 15ad",
    ] {
        interface
            .find(text)
            .unwrap_or_else(|error| panic!("{text}: {error}"));
    }
    interface
        .click("Accept the certificate")
        .expect("the question has an accept button");
    interface
        .click("Cancel")
        .expect("the question has a cancel button");

    let messages: Vec<Message> = interface.into_messages().collect();
    assert!(
        matches!(
            messages.as_slice(),
            [
                Message::Servers(servers::Message::AcceptCertificate),
                Message::Servers(servers::Message::Cancel)
            ]
        ),
        "{messages:?}"
    );
}

#[test]
fn the_window_warns_about_a_certificate_that_changed() {
    let window = window(certificate_app(Mode::Light, true), Language::Spanish);
    let mut interface = simulator(&window);

    for text in [
        "Advertencia: el certificado de este servidor cambió.",
        "Huella",
        "Aceptar el certificado",
    ] {
        interface
            .find(text)
            .unwrap_or_else(|error| panic!("{text}: {error}"));
    }
}

#[test]
fn the_connect_button_and_the_enter_key_start_a_connection() {
    let window = window(connect_app(Mode::Light), Language::English);

    let mut interface = simulator(&window);
    interface
        .click("Connect")
        .expect("the connect screen has a button");
    let messages: Vec<Message> = interface.into_messages().collect();
    assert!(
        matches!(
            messages.as_slice(),
            [Message::Servers(servers::Message::Connect)]
        ),
        "{messages:?}"
    );

    let mut interface = simulator(&window);
    interface
        .click(selector::id(view::NAME_FIELD))
        .expect("the connect screen has a name field");
    interface.tap_key(Named::Enter);
    let messages: Vec<Message> = interface.into_messages().collect();
    assert!(
        matches!(
            messages.as_slice(),
            [Message::Servers(servers::Message::Connect)]
        ),
        "{messages:?}"
    );
}

#[test]
fn the_connect_screen_shows_a_problem() {
    let mut app = new_app(Mode::Light);
    typed(&mut app, "localhost", "a");
    app.update(Message::Servers(servers::Message::Connect));
    let window = window(app, Language::English);
    let mut interface = simulator(&window);

    interface
        .find("A name must have a minimum of 2 characters.")
        .expect("the problem shows");
}

#[test]
fn the_window_tells_why_post_cannot_start() {
    let app = App::stopped(Mode::Light, StartProblem::Damaged("seed"));
    let window = window(app, Language::English);
    let mut interface = simulator(&window);

    interface
        .find("The seed file of this profile is not valid. Post cannot open the profile.")
        .expect("the problem shows");
    assert!(interface.find("Connect").is_err());
}

#[test]
fn the_fields_do_not_change_while_the_application_connects() {
    let window = window(connecting_app(Mode::Light), Language::English);
    let mut interface = simulator(&window);

    interface
        .find("Connecting to localhost:7678…")
        .expect("the progress text shows");
    interface
        .click(selector::id(view::NAME_FIELD))
        .expect("the connect screen has a name field");
    interface.typewrite("x");
    interface.tap_key(Named::Enter);
    interface
        .click("Cancel")
        .expect("the user can stop the connection");

    let messages: Vec<Message> = interface.into_messages().collect();
    assert!(
        matches!(
            messages.as_slice(),
            [Message::Servers(servers::Message::Cancel)]
        ),
        "{messages:?}"
    );
}

#[test]
fn the_chat_screen_shows_the_timeline_with_local_times() {
    let (app, _outbox) = chat_app(Mode::Light);
    let window = window(app, Language::English);
    let mut interface = simulator(&window);

    for text in [
        "#general",
        "ana · connected",
        "14:02",
        "ben joined",
        "Hello",
        "Hi. Does it work?",
        "14:05",
        "ben left",
    ] {
        interface
            .find(text)
            .unwrap_or_else(|error| panic!("{text}: {error}"));
    }
}

#[test]
fn the_message_field_sends_the_draft_with_the_enter_key() {
    let (app, _outbox) = chat_app(Mode::Light);
    let window = window(app, Language::English);
    let mut interface = simulator(&window);

    interface
        .click(selector::id(view::DRAFT_FIELD))
        .expect("the chat screen has a message field");
    interface.typewrite("h");
    interface.tap_key(Named::Enter);

    let messages: Vec<Message> = interface.into_messages().collect();
    assert!(
        matches!(messages.as_slice(), [Message::Chat(chat::Message::DraftChanged(draft)), Message::Chat(chat::Message::Send)] if draft == "h"),
        "{messages:?}"
    );
}

#[test]
fn the_leave_button_leaves_the_chat() {
    let (app, _outbox) = chat_app(Mode::Light);
    let window = window(app, Language::English);
    let mut interface = simulator(&window);

    interface
        .click("Leave")
        .expect("the chat screen has a leave button");

    let messages: Vec<Message> = interface.into_messages().collect();
    assert!(
        matches!(messages.as_slice(), [Message::Chat(chat::Message::Leave)]),
        "{messages:?}"
    );
}

#[test]
fn the_header_of_the_chat_has_a_link_for_each_panel() {
    let (app, _outbox) = chat_app(Mode::Light);
    let member = window(app, Language::English);
    assert!(simulator(&member).find("Invitations").is_err());
    let messages = clicked(&member, "Change name");
    assert!(
        matches!(
            messages.as_slice(),
            [Message::Chat(chat::Message::Open(chat::Panel::Account))]
        ),
        "{messages:?}"
    );

    let (app, _outbox) = chat_app_as(Mode::Light, Role::Owner);
    let owner = window(app, Language::English);
    let messages = clicked(&owner, "Invitations");
    assert!(
        matches!(
            messages.as_slice(),
            [Message::Chat(chat::Message::Open(chat::Panel::Invitations))]
        ),
        "{messages:?}"
    );
}

fn channel_state(title: &str) -> ServerMessage {
    ServerMessage::Channel {
        reference: None,
        channel: channel(title),
        kind: ChannelKind::Public,
        topic: String::new(),
        owner: name("ana"),
        created: time("14:00:00"),
    }
}

fn channels_app(mode: Mode, role: Role) -> (App, Outbox) {
    let (mut app, outbox) = chat_app_as(mode, role);
    let received = [
        channel_state("dev"),
        ServerMessage::Message {
            reference: None,
            channel: "dev".to_owned(),
            from: name("ben"),
            time: time("14:06:00"),
            text: "This is the channel for the code.".to_owned(),
        },
    ];
    for message in received {
        receive(&mut app, message);
    }
    (app, outbox)
}

fn listed(title: &str, topic: &str, members: u32) -> ListedChannel {
    ListedChannel {
        channel: channel(title),
        kind: ChannelKind::Public,
        topic: topic.to_owned(),
        members,
    }
}

fn three_channels() -> Vec<ListedChannel> {
    vec![
        listed("garden", "Plants and flowers", 12),
        listed("general", "", 2),
        listed("solo", "", 1),
    ]
}

fn channels_panel_app(mode: Mode, list: Option<Vec<ListedChannel>>) -> (App, Outbox) {
    let (mut app, mut outbox) = channels_app(mode, Role::Member);
    app.update(Message::Chat(chat::Message::Open(chat::Panel::Channels)));
    let Some(ClientMessage::Channels { reference }) = outbox() else {
        panic!("the panel asks the server for the channels");
    };
    if let Some(list) = list {
        let attempt = app.target().expect("the chat has a connection").attempt;
        let answer = ServerMessage::Channels {
            reference,
            list,
            more: false,
        };
        app.update(Message::Connection(attempt, Event::Received(answer)));
    }
    (app, outbox)
}

fn channel(title: &str) -> ChannelName {
    ChannelName::parse(title).expect(title)
}

#[test]
fn the_sidebar_shows_the_server_the_user_and_each_channel() {
    let (app, _outbox) = channels_app(Mode::Light, Role::Member);
    let window = window(app, Language::English);
    let mut interface = simulator(&window);

    for text in [
        "localhost:7678",
        "ana · connected",
        "#dev",
        "#general",
        "Change name",
        "Settings",
        "Leave",
    ] {
        let bounds = interface
            .find(text)
            .unwrap_or_else(|error| panic!("{text}: {error}"))
            .bounds();
        assert!(bounds.x + bounds.width <= 200.0, "{text}: {bounds:?}");
    }
    assert!(interface.find("Menu").is_err());
    assert!(interface.find("Invitations").is_err());

    for title in ["dev", "general"] {
        let messages = clicked(&window, &format!("#{title}"));
        assert!(
            matches!(
                messages.as_slice(),
                [Message::Chat(chat::Message::Select(name))] if *name == channel(title)
            ),
            "{messages:?}"
        );
    }
}

#[test]
fn the_window_shows_the_timeline_of_the_channel_that_the_user_selects() {
    let (mut app, _outbox) = channels_app(Mode::Light, Role::Member);
    app.update(Message::Chat(chat::Message::Select(channel("dev"))));
    let window = window(app, Language::English);
    let mut interface = simulator(&window);

    interface
        .find("This is the channel for the code.")
        .expect("the timeline of the channel");
    assert!(interface.find("Hi. Does it work?").is_err());
    interface
        .find(InMainArea("#dev"))
        .expect("the bar shows the name of the channel");
    assert!(interface.find(InMainArea("#general")).is_err());
}

#[test]
fn the_sidebar_opens_the_channels_panel_and_the_bar_leaves_a_channel() {
    let (app, _outbox) = channels_app(Mode::Light, Role::Member);
    let window = window(app, Language::English);

    let messages = clicked(&window, "Channels");
    assert!(
        matches!(
            messages.as_slice(),
            [Message::Chat(chat::Message::Open(chat::Panel::Channels))]
        ),
        "{messages:?}"
    );
    let messages = clicked(&window, "Leave channel");
    assert!(
        matches!(
            messages.as_slice(),
            [Message::Chat(chat::Message::LeaveChannel)]
        ),
        "{messages:?}"
    );
}

#[test]
fn the_chat_opens_the_channels_panel_for_a_user_with_no_channel() {
    let (mut app, mut outbox) = signed_in_app(Mode::Light, Role::Member);
    receive(&mut app, ServerMessage::Ready {});

    assert_eq!(
        outbox(),
        Some(ClientMessage::Channels { reference: Some(1) })
    );
    let window = window(app, Language::English);
    let mut interface = simulator(&window);
    for text in ["localhost:7678", "Name of a new channel", "Back"] {
        interface
            .find(InMainArea(text))
            .unwrap_or_else(|error| panic!("{text}: {error}"));
    }
}

#[test]
fn the_chat_tells_a_user_with_no_channel_what_to_do() {
    let (app, _outbox) = no_channel_app(Mode::Light);
    let window = window(app, Language::English);
    let mut interface = simulator(&window);

    for text in [
        "localhost:7678",
        "No channel is open",
        "You are not a member of a channel. Join a channel or make one.",
        "Channels",
    ] {
        interface
            .find(InMainArea(text))
            .unwrap_or_else(|error| panic!("{text}: {error}"));
    }
    assert!(interface.find("Leave channel").is_err());
    assert!(interface.find(selector::id(view::DRAFT_FIELD)).is_err());

    interface
        .click(InMainArea("Channels"))
        .expect("the main area has a button for the channels");
    let messages: Vec<Message> = interface.into_messages().collect();
    assert!(
        matches!(
            messages.as_slice(),
            [Message::Chat(chat::Message::Open(chat::Panel::Channels))]
        ),
        "{messages:?}"
    );
}

#[test]
fn the_chat_shows_a_notice_to_a_user_with_no_channel() {
    let (mut app, _outbox) = no_channel_app(Mode::Light);
    app.update(Message::ProfileNotSaved);
    let window = window(app, Language::English);

    simulator(&window)
        .find("Post could not save this server in your profile.")
        .expect("the notice shows below the hint");
}

#[test]
fn the_chat_shows_no_hint_before_the_sign_in_is_complete() {
    let (app, _outbox) = signed_in_app(Mode::Light, Role::Member);
    let window = window(app, Language::English);
    let mut interface = simulator(&window);

    interface
        .find(InMainArea("localhost:7678"))
        .expect("the bar shows the address of the server");
    assert!(interface.find("No channel is open").is_err());
}

#[test]
fn the_channels_panel_lists_the_public_channels() {
    let (app, _outbox) = channels_panel_app(Mode::Light, Some(three_channels()));
    let window = window(app, Language::English);
    let mut interface = simulator(&window);

    for text in [
        "Channels",
        "Name of a new channel",
        "Public",
        "Private",
        "Make a channel",
        "#garden",
        "Plants and flowers · 12 members",
        "#general",
        "2 members",
        "#solo",
        "1 member",
        "Back",
    ] {
        interface
            .find(InMainArea(text))
            .unwrap_or_else(|error| panic!("{text}: {error}"));
    }
    assert!(interface.find("Hello").is_err());

    let messages = clicked(&window, "Join");
    assert!(
        matches!(
            messages.as_slice(),
            [Message::Chat(chat::Message::Join(name))] if name.as_str() == "garden"
        ),
        "{messages:?}"
    );
    let messages = clicked(&window, "Open");
    assert!(
        matches!(
            messages.as_slice(),
            [Message::Chat(chat::Message::Select(name))] if *name == channel("general")
        ),
        "{messages:?}"
    );
}

#[test]
fn the_channels_panel_tells_the_state_of_the_list() {
    let (app, _outbox) = channels_panel_app(Mode::Light, None);
    let window = self::window(app, Language::English);
    simulator(&window)
        .find("Post asks the server for the channels.")
        .expect("the panel tells that the list is not there");

    let (app, _outbox) = channels_panel_app(Mode::Light, Some(Vec::new()));
    let window = self::window(app, Language::Spanish);
    simulator(&window)
        .find("El servidor no tiene canales públicos.")
        .expect("the panel tells that the list is empty");
}

#[test]
fn the_channels_panel_makes_a_channel() {
    let (app, _outbox) = channels_panel_app(Mode::Light, Some(Vec::new()));
    let window = window(app, Language::English);
    let mut interface = simulator(&window);

    interface
        .click(selector::id(view::PANEL_FIELD))
        .expect("the channels panel has a name field");
    interface.typewrite("x");
    interface.tap_key(Named::Enter);
    interface
        .click("Private")
        .expect("the panel has a choice of the kind");
    interface
        .click("Make a channel")
        .expect("the panel has a button");

    let messages: Vec<Message> = interface.into_messages().collect();
    assert!(
        matches!(
            messages.as_slice(),
            [
                Message::Chat(chat::Message::ChannelNameChanged(name)),
                Message::Chat(chat::Message::MakeChannel),
                Message::Chat(chat::Message::KindSelected(ChannelKind::Private)),
                Message::Chat(chat::Message::MakeChannel)
            ] if name == "x"
        ),
        "{messages:?}"
    );
}

#[test]
fn the_channels_panel_shows_a_problem_with_the_new_channel() {
    let (mut app, _outbox) = channels_panel_app(Mode::Light, Some(Vec::new()));
    let name = chat::Message::ChannelNameChanged("Garden".to_owned());
    app.update(Message::Chat(name));
    app.update(Message::Chat(chat::Message::MakeChannel));
    let window = self::window(app, Language::English);
    simulator(&window)
        .find("A channel name can contain only a to z, 0 to 9, hyphen, and underscore.")
        .expect("the panel shows the problem of the name");

    let (mut app, _outbox) = channels_panel_app(Mode::Light, Some(Vec::new()));
    let attempt = app.target().expect("the chat has a connection").attempt;
    let refused = ServerMessage::error(Some(9), ErrorCode::NameTaken);
    app.update(Message::Connection(attempt, Event::Received(refused)));
    let window = self::window(app, Language::Spanish);
    simulator(&window)
        .find("Un canal ya usa este nombre, o el nombre está reservado.")
        .expect("the panel shows the answer of the server");
}

struct InMainArea(&'static str);

impl Selector for InMainArea {
    type Output = selector::Text;

    fn select(&mut self, candidate: Candidate<'_>) -> Option<selector::Text> {
        match candidate {
            Candidate::Text {
                id,
                bounds,
                visible_bounds,
                content,
            } if content == self.0 && bounds.x > 200.0 => Some(selector::Text::Raw {
                id: id.cloned(),
                bounds,
                visible_bounds,
            }),
            _ => None,
        }
    }

    fn description(&self) -> String {
        format!("the text {} at the right of the sidebar", self.0)
    }
}

#[test]
fn the_smallest_window_has_a_menu_in_the_place_of_the_sidebar() {
    let smallest = Size::new(520.0, 320.0);
    let (app, _outbox) = channels_app(Mode::Light, Role::Owner);
    let narrow = window(app, Language::Spanish);
    let mut interface = simulator_of_size(&narrow, smallest);

    for hidden in [
        "ana · conectado",
        "localhost:7678",
        "Cambiar nombre",
        "#dev",
    ] {
        assert!(interface.find(hidden).is_err(), "{hidden}");
    }
    for text in ["#general", "Menú"] {
        let bounds = interface
            .find(text)
            .unwrap_or_else(|error| panic!("{text}: {error}"))
            .bounds();
        assert!(
            bounds.x >= 16.0 && bounds.x + bounds.width <= smallest.width - 16.0,
            "{text}: {bounds:?}"
        );
    }
    interface.click("Menú").expect("the bar has a menu link");
    let messages: Vec<Message> = interface.into_messages().collect();
    assert!(
        matches!(
            messages.as_slice(),
            [Message::Chat(chat::Message::Open(chat::Panel::Menu))]
        ),
        "{messages:?}"
    );

    let (mut app, _outbox) = channels_app(Mode::Light, Role::Owner);
    app.update(Message::Chat(chat::Message::Open(chat::Panel::Menu)));
    let menu = window(app, Language::Spanish);
    let mut interface = simulator_of_size(&menu, smallest);
    for text in [
        "Volver",
        "localhost:7678",
        "ana · conectado",
        "#dev",
        "#general",
        "Invitaciones",
        "Cambiar nombre",
        "Configuración",
        "Salir",
    ] {
        let bounds = interface
            .find(text)
            .unwrap_or_else(|error| panic!("{text}: {error}"))
            .bounds();
        assert!(
            bounds.x >= 16.0 && bounds.x + bounds.width <= smallest.width - 16.0,
            "{text}: {bounds:?}"
        );
    }
    assert!(interface.find("Menú").is_err());

    let mut wide = simulator_of_size(&menu, Size::new(700.0, 320.0));
    wide.find("ana · conectado")
        .expect("a wide window shows the sidebar");
}

#[test]
fn the_invitations_panel_lists_the_open_invitations() {
    let (app, _outbox) = invitations_app(Mode::Light, Some(two_invitations()));
    let window = window(app, Language::English);
    let mut interface = simulator(&window);

    for text in [
        "#general",
        "Invitations",
        "Ends after",
        "7 days",
        "No limit",
        "https://chat.example.org/i/k3Wm8sTz2LpQ",
        "Ends 2026-11-28 14:02 · Uses: 2 of 5",
        "https://chat.example.org/i/7Hq2kdX9mPq4",
        "No end · Uses: 0",
    ] {
        interface
            .find(text)
            .unwrap_or_else(|error| panic!("{text}: {error}"));
    }
    interface.find("Back").expect("the bar has a back link");
    for text in ["Leave", "Change name", "Hello"] {
        assert!(interface.find(InMainArea(text)).is_err(), "{text}");
    }
}

#[test]
fn the_invitations_panel_tells_the_state_of_the_list() {
    let (app, _outbox) = invitations_app(Mode::Light, None);
    let window = self::window(app, Language::English);
    simulator(&window)
        .find("Post asks the server for the invitations.")
        .expect("the panel tells that the list is not there");

    let (app, _outbox) = invitations_app(Mode::Light, Some(Vec::new()));
    let window = self::window(app, Language::English);
    simulator(&window)
        .find("The server has no open invitations.")
        .expect("the panel tells that the list is empty");

    let (mut app, _outbox) = invitations_app(Mode::Light, Some(Vec::new()));
    receive(
        &mut app,
        ServerMessage::error(Some(1), ErrorCode::NotPermitted),
    );
    let window = self::window(app, Language::English);
    simulator(&window)
        .find("Your account does not have the permission for this task.")
        .expect("the panel shows the error");
}

#[test]
fn the_buttons_of_the_invitations_panel_make_copy_and_cancel_invitations() {
    let list = vec![invitation("7Hq2kdX9mPq4", None, None, 0)];
    let (app, _outbox) = invitations_app(Mode::Light, Some(list));
    let window = window(app, Language::English);

    let messages = clicked(&window, "1 day");
    assert!(
        matches!(
            messages.as_slice(),
            [Message::Chat(chat::Message::LifetimeSelected(
                chat::Lifetime::Day
            ))]
        ),
        "{messages:?}"
    );
    let messages = clicked(&window, "25");
    assert!(
        matches!(
            messages.as_slice(),
            [Message::Chat(chat::Message::UsesSelected(
                chat::Uses::TwentyFive
            ))]
        ),
        "{messages:?}"
    );
    assert!(clicked(&window, "7 days").is_empty());
    assert!(clicked(&window, "No limit").is_empty());
    let messages = clicked(&window, "New invite link");
    assert!(
        matches!(messages.as_slice(), [Message::Chat(chat::Message::Invite)]),
        "{messages:?}"
    );
    let messages = clicked(&window, "Copy");
    assert!(
        matches!(messages.as_slice(), [Message::Chat(chat::Message::Copy(copied))] if *copied == code("7Hq2kdX9mPq4")),
        "{messages:?}"
    );
    let messages = clicked(&window, "Cancel invitation");
    assert!(
        matches!(messages.as_slice(), [Message::Chat(chat::Message::Revoke(revoked))] if *revoked == code("7Hq2kdX9mPq4")),
        "{messages:?}"
    );
    let messages = clicked(&window, "Back");
    assert!(
        matches!(messages.as_slice(), [Message::Chat(chat::Message::Close)]),
        "{messages:?}"
    );
}

#[test]
fn the_invitations_panel_tells_which_link_is_in_the_clipboard() {
    let (mut app, _outbox) = invitations_app(Mode::Light, Some(two_invitations()));
    let copy = chat::Message::Copy(code("7Hq2kdX9mPq4"));
    app.update(Message::Chat(copy));
    let window = window(app, Language::English);
    let mut interface = simulator(&window);

    interface
        .find("Copied")
        .expect("one link is in the clipboard");
    interface.find("Copy").expect("one link is not there");
}

#[test]
fn the_account_panel_changes_the_name() {
    let (app, _outbox) = account_app(Mode::Light);
    let window = window(app, Language::English);
    let mut interface = simulator(&window);

    for text in [
        "Your name on this server",
        "New name",
        "You can change your name one time in 30 days. Your earlier name stays reserved for you for 30 days.",
    ] {
        interface
            .find(text)
            .unwrap_or_else(|error| panic!("{text}: {error}"));
    }
    interface
        .click(selector::id(view::PANEL_FIELD))
        .expect("the account panel has a name field");
    interface.typewrite("x");
    interface.tap_key(Named::Enter);
    interface
        .click(InMainArea("Change name"))
        .expect("the account panel has a button");

    let messages: Vec<Message> = interface.into_messages().collect();
    assert!(
        matches!(
            messages.as_slice(),
            [
                Message::Chat(chat::Message::NameChanged(name)),
                Message::Chat(chat::Message::Rename),
                Message::Chat(chat::Message::Rename)
            ] if name.contains('x') && name.contains("ana")
        ),
        "{messages:?}"
    );
}

#[test]
fn the_account_panel_shows_a_problem_with_the_new_name() {
    let (mut app, _outbox) = account_app(Mode::Light);
    let short = chat::Message::NameChanged("a".to_owned());
    app.update(Message::Chat(short));
    app.update(Message::Chat(chat::Message::Rename));
    let window = self::window(app, Language::English);
    simulator(&window)
        .find("A name must have a minimum of 2 characters.")
        .expect("the problem shows");

    let (mut app, _outbox) = account_app(Mode::Light);
    receive(&mut app, ServerMessage::error(Some(1), ErrorCode::TooSoon));
    let window = self::window(app, Language::English);
    simulator(&window)
        .find("You changed your name a short time ago. Try again later.")
        .expect("the error shows");
}

#[test]
fn the_servers_screen_and_the_chat_have_a_link_to_the_settings() {
    let servers = window(new_app(Mode::Light), Language::English);
    let (app, _outbox) = chat_app(Mode::Light);
    let chat = window(app, Language::English);

    for window in [&servers, &chat] {
        let messages = clicked(window, "Settings");
        assert!(
            matches!(messages.as_slice(), [Message::Open(Page::Settings)]),
            "{messages:?}"
        );
    }
}

#[test]
fn only_the_servers_screen_has_a_link_to_the_profiles() {
    let servers = window(new_app(Mode::Light), Language::English);
    let messages = clicked(&servers, "Profiles");
    assert!(
        matches!(messages.as_slice(), [Message::Open(Page::Profiles)]),
        "{messages:?}"
    );

    let (app, _outbox) = chat_app(Mode::Light);
    let chat = window(app, Language::English);
    assert!(simulator(&chat).find("Profiles").is_err());
}

#[test]
fn the_profiles_page_lists_the_profiles() {
    let window = window(profiles_app(Mode::Light), Language::English);
    let mut interface = simulator(&window);

    for text in [
        "Profiles",
        "Each profile has its own identity, servers, and settings.",
        "main",
        "Open in this window",
        "work",
        "Open",
        "Rename",
        "Delete",
        "Name of a new profile",
        "Make a profile",
        "Back",
    ] {
        interface
            .find(text)
            .unwrap_or_else(|error| panic!("{text}: {error}"));
    }
    interface
        .click(selector::id(view::PROFILE_FIELD))
        .expect("the page has a field for a new name");
    interface.typewrite("g");
    interface.tap_key(Named::Enter);
    let messages: Vec<Message> = interface.into_messages().collect();
    assert!(
        matches!(
            messages.as_slice(),
            [
                Message::Profiles(profiles::Message::NewNameChanged(name)),
                Message::Profiles(profiles::Message::Create)
            ] if name == "g"
        ),
        "{messages:?}"
    );

    assert!(
        matches!(profile_message(&window, "Open"), profiles::Message::Switch(id) if id == profile(2))
    );
    assert!(
        matches!(profile_message(&window, "Rename"), profiles::Message::Rename(id) if id == profile(1))
    );
    assert!(
        matches!(profile_message(&window, "Delete"), profiles::Message::Delete(id) if id == profile(2))
    );
    assert!(matches!(
        profile_message(&window, "Make a profile"),
        profiles::Message::Create
    ));
    let messages = clicked(&window, "Back");
    assert!(
        matches!(messages.as_slice(), [Message::Back]),
        "{messages:?}"
    );
}

#[test]
fn the_profiles_page_changes_the_name_of_a_profile() {
    let rename = vec![profiles::Message::Rename(profile(2))];
    let window = window(profiles_app_with(Mode::Light, rename), Language::English);
    let mut interface = simulator(&window);

    interface
        .click(selector::id(view::PANEL_FIELD))
        .expect("the row has a name field");
    interface.typewrite("x");
    interface.tap_key(Named::Enter);
    let messages: Vec<Message> = interface.into_messages().collect();
    assert!(
        matches!(
            messages.as_slice(),
            [
                Message::Profiles(profiles::Message::EditChanged(name)),
                Message::Profiles(profiles::Message::Confirm)
            ] if name.contains('x') && name.contains("work")
        ),
        "{messages:?}"
    );

    assert!(matches!(
        profile_message(&window, "Save"),
        profiles::Message::Confirm
    ));
    assert!(matches!(
        profile_message(&window, "Cancel"),
        profiles::Message::Cancel
    ));
    assert!(clicked(&window, "Rename").is_empty());
    let mut interface = simulator(&window);
    assert!(interface.find("Open").is_err());
    assert!(interface.find("Delete").is_err());
}

#[test]
fn the_profiles_page_asks_for_the_name_before_it_deletes_a_profile() {
    let delete = vec![profiles::Message::Delete(profile(2))];
    let window = window(
        profiles_app_with(Mode::Light, delete.clone()),
        Language::English,
    );
    let mut interface = simulator(&window);
    for text in [
        "If you delete the profile \"work\", you lose its accounts on all servers. No person can get them back.",
        "Type the name of the profile to delete it.",
        "Delete this profile",
        "Cancel",
    ] {
        interface
            .find(text)
            .unwrap_or_else(|error| panic!("{text}: {error}"));
    }
    assert!(clicked(&window, "Delete this profile").is_empty());

    let mut typed = delete;
    typed.push(profiles::Message::EditChanged("work".to_owned()));
    let window = self::window(profiles_app_with(Mode::Light, typed), Language::English);
    assert!(matches!(
        profile_message(&window, "Delete this profile"),
        profiles::Message::Confirm
    ));
}

#[test]
fn the_profiles_page_shows_a_problem_at_the_place_of_the_change() {
    let taken = profiles::Message::Failed(profiles::Problem::Name(NameProblem::Taken));
    let window = window(
        profiles_app_with(Mode::Light, vec![taken.clone()]),
        Language::English,
    );
    simulator(&window)
        .find("A different profile has this name.")
        .expect("the problem shows below the field for a new profile");

    let in_use = profiles::Message::Failed(profiles::Problem::InUse);
    let messages = vec![profiles::Message::Delete(profile(2)), in_use];
    let window = self::window(profiles_app_with(Mode::Light, messages), Language::Spanish);
    let mut interface = simulator(&window);
    for text in [
        "Este perfil está abierto en otra ventana.",
        "Eliminar este perfil",
        "Escribe el nombre del perfil para eliminarlo.",
        "Perfiles",
        "Abierto en esta ventana",
        "Nombre de un perfil nuevo",
        "Crear perfil",
    ] {
        interface
            .find(text)
            .unwrap_or_else(|error| panic!("{text}: {error}"));
    }
}

#[test]
fn the_console_shows_each_line_with_its_time_and_direction() {
    let (app, _outbox) = console_app(Mode::Light);
    let window = window(app, Language::English);
    let mut interface = simulator(&window);

    for text in [
        "Protocol console",
        "14:02:07.004",
        "→",
        SENT_LINE,
        "14:02:07.120",
        "←",
        RECEIVED_LINE,
        "Clear",
        "Close",
        "Send",
        "#general",
        "ben left",
    ] {
        interface
            .find(text)
            .unwrap_or_else(|error| panic!("{text}: {error}"));
    }
}

#[test]
fn the_console_sends_a_line_and_has_its_own_links() {
    let (app, _outbox) = console_app(Mode::Light);
    let window = window(app, Language::English);
    let mut interface = simulator(&window);

    interface
        .click(selector::id(view::CONSOLE_FIELD))
        .expect("the console has a field");
    interface.typewrite("{");
    interface.tap_key(Named::Enter);
    let messages: Vec<Message> = interface.into_messages().collect();
    assert!(
        matches!(
            messages.as_slice(),
            [
                Message::Console(console::Message::DraftChanged(draft)),
                Message::Console(console::Message::Send)
            ] if draft == "{"
        ),
        "{messages:?}"
    );

    let messages = clicked(&window, "Send");
    assert!(
        matches!(
            messages.as_slice(),
            [Message::Console(console::Message::Send)]
        ),
        "{messages:?}"
    );
    let messages = clicked(&window, "Clear");
    assert!(
        matches!(
            messages.as_slice(),
            [Message::Console(console::Message::Clear)]
        ),
        "{messages:?}"
    );
    let messages = clicked(&window, "Close");
    assert!(
        matches!(
            messages.as_slice(),
            [Message::Settings(settings::Message::ConsoleToggled(false))]
        ),
        "{messages:?}"
    );
}

#[test]
fn the_console_shows_on_each_screen_only_when_the_setting_is_on() {
    let window = window(new_app(Mode::Light), Language::English);
    assert!(simulator(&window).find("Protocol console").is_err());

    let mut app = new_app(Mode::Light);
    with_console(&mut app);
    let draft = console::Message::DraftChanged("{}".to_owned());
    app.update(Message::Console(draft));
    app.update(Message::Console(console::Message::Send));
    let window = self::window(app, Language::English);
    let mut interface = simulator(&window);
    for text in [
        "Protocol console",
        "The console shows the lines of a connection.",
        "Post did not send the line. A line needs an open connection.",
        "Connect",
    ] {
        interface
            .find(text)
            .unwrap_or_else(|error| panic!("{text}: {error}"));
    }

    let mut app = settings_app(Mode::Light);
    with_console(&mut app);
    let window = self::window(app, Language::Spanish);
    let mut interface = simulator(&window);
    for text in [
        "Consola del protocolo",
        "La consola muestra las líneas de una conexión.",
        "Limpiar",
        "Cerrar",
        "Enviar",
        "Idioma",
    ] {
        interface
            .find(text)
            .unwrap_or_else(|error| panic!("{text}: {error}"));
    }
}

#[test]
fn the_settings_page_shows_each_setting() {
    let mut window = window(settings_app(Mode::Light), Language::English);
    window.settings_file = Some("/home/ana/.config/post/profiles/1/settings.toml".to_owned());
    let mut interface = simulator(&window);

    for text in [
        "Settings",
        "Language",
        "System language",
        "English",
        "Español (Latinoamérica)",
        "Clock",
        "24 hours",
        "12 hours",
        "Show seconds",
        "Text size",
        "Smaller",
        "100%",
        "Larger",
        "For developers",
        "Protocol console (F12 key)",
        "Post keeps these settings in this file: /home/ana/.config/post/profiles/1/settings.toml",
        "Back",
    ] {
        interface
            .find(text)
            .unwrap_or_else(|error| panic!("{text}: {error}"));
    }
    assert!(interface.find("Connect").is_err());
}

#[test]
fn the_controls_of_the_settings_page_change_the_settings() {
    let window = window(settings_app(Mode::Light), Language::English);
    let changed = |text: &str| match clicked(&window, text).as_slice() {
        [Message::Settings(message)] => message.clone(),
        other => panic!("{text}: {other:?}"),
    };

    assert!(matches!(
        changed("Español (Latinoamérica)"),
        settings::Message::LanguageSelected(Some(Language::Spanish))
    ));
    assert!(matches!(
        changed("English"),
        settings::Message::LanguageSelected(Some(Language::English))
    ));
    assert!(matches!(
        changed("12 hours"),
        settings::Message::ClockSelected(Clock::Hours12)
    ));
    assert!(matches!(
        changed("Show seconds"),
        settings::Message::SecondsToggled(true)
    ));
    assert!(matches!(changed("Larger"), settings::Message::Larger));
    assert!(matches!(changed("Smaller"), settings::Message::Smaller));
    assert!(matches!(
        changed("Protocol console (F12 key)"),
        settings::Message::ConsoleToggled(true)
    ));
    assert!(clicked(&window, "System language").is_empty());
    assert!(clicked(&window, "24 hours").is_empty());
    let messages = clicked(&window, "Back");
    assert!(
        matches!(messages.as_slice(), [Message::Back]),
        "{messages:?}"
    );
}

#[test]
fn the_settings_page_shows_the_values_of_the_file() {
    let mut app = settings_app(Mode::Light);
    settings_file(
        &mut app,
        "language = \"en\"\nclock = 12\nseconds = true\ntext-size = 200\nconsole = true\n",
    );
    let window = window(app, Language::English);

    simulator(&window).find("200%").expect("the size shows");
    assert!(clicked(&window, "English").is_empty());
    assert!(clicked(&window, "12 hours").is_empty());
    assert!(clicked(&window, "Larger").is_empty());
    let messages = clicked(&window, "Show seconds");
    assert!(
        matches!(
            messages.as_slice(),
            [Message::Settings(settings::Message::SecondsToggled(false))]
        ),
        "{messages:?}"
    );
    let messages = clicked(&window, "Protocol console (F12 key)");
    assert!(
        matches!(
            messages.as_slice(),
            [Message::Settings(settings::Message::ConsoleToggled(false))]
        ),
        "{messages:?}"
    );

    let mut app = settings_app(Mode::Light);
    settings_file(&mut app, "text-size = 80\n");
    let window = self::window(app, Language::English);
    assert!(clicked(&window, "Smaller").is_empty());
}

#[test]
fn the_settings_page_shows_a_problem_of_the_file() {
    let problems = [
        (
            "clock = = 12\n",
            "The settings file is not valid TOML. Post ignores the file until you correct it.",
        ),
        (
            "clock = 13\n",
            "The value of \"clock\" in the settings file is not valid. Post uses the default value.",
        ),
    ];
    for (file, problem) in problems {
        let mut app = settings_app(Mode::Light);
        settings_file(&mut app, file);
        let window = window(app, Language::English);
        simulator(&window)
            .find(problem)
            .unwrap_or_else(|error| panic!("{problem}: {error}"));
    }

    let mut app = settings_app(Mode::Light);
    app.update(Message::Settings(settings::Message::NotSaved));
    let window = window(app, Language::English);
    simulator(&window)
        .find("Post could not write the settings file.")
        .expect("the problem shows");
}

#[test]
fn the_times_obey_the_clock_settings() {
    let (mut app, _outbox) = chat_app(Mode::Light);
    settings_file(&mut app, "clock = 12\nseconds = true\n");
    let english = window(app, Language::English);
    let mut interface = simulator(&english);
    for text in [" 2:02:05 PM", " 2:02:07 PM", " 2:05:00 PM"] {
        interface
            .find(text)
            .unwrap_or_else(|error| panic!("{text}: {error}"));
    }
    assert!(interface.find("14:02").is_err());

    let (mut app, _outbox) = invitations_app(Mode::Light, Some(two_invitations()));
    settings_file(&mut app, "clock = 12\n");
    let spanish = window(app, Language::Spanish);
    simulator(&spanish)
        .find("Termina 2026-11-28 2:02 p. m. · Usos: 2 de 5")
        .expect("the end of the invitation shows in the 12-hour format");
}

#[test]
fn the_chat_screen_shows_an_error_from_the_server() {
    let (mut app, _outbox) = chat_app(Mode::Light);
    let attempt = app.target().expect("a connection").attempt;
    let refused = ServerMessage::error(Some(1), ErrorCode::TextTooLong);
    app.update(Message::Connection(attempt, Event::Received(refused)));
    let window = window(app, Language::English);
    let mut interface = simulator(&window);

    interface
        .find("A message can have a maximum of 4,000 characters.")
        .expect("the error shows");
}

#[test]
fn the_interface_has_spanish_text() {
    let window = window(connect_app(Mode::Light), Language::Spanish);
    let mut interface = simulator(&window);
    for text in [
        "Enlace de invitación o dirección del servidor",
        "Nombre para una cuenta nueva",
        "Conectar",
    ] {
        interface
            .find(text)
            .unwrap_or_else(|error| panic!("{text}: {error}"));
    }

    let (app, _outbox) = chat_app(Mode::Light);
    let window = self::window(app, Language::Spanish);
    let mut interface = simulator(&window);
    for text in [
        "ana · conectado",
        "ben entró",
        "ben salió",
        "Cambiar nombre",
        "Salir",
    ] {
        interface
            .find(text)
            .unwrap_or_else(|error| panic!("{text}: {error}"));
    }

    let (app, _outbox) = invitations_app(Mode::Light, Some(two_invitations()));
    let window = self::window(app, Language::Spanish);
    let mut interface = simulator(&window);
    for text in [
        "Invitaciones",
        "Termina después de",
        "7 días",
        "Sin límite",
        "Nuevo enlace de invitación",
        "Termina 2026-11-28 14:02 · Usos: 2 de 5",
        "Sin fin · Usos: 0",
        "Copiar",
        "Cancelar invitación",
        "Volver",
    ] {
        interface
            .find(text)
            .unwrap_or_else(|error| panic!("{text}: {error}"));
    }

    let (app, _outbox) = account_app(Mode::Light);
    let window = self::window(app, Language::Spanish);
    let mut interface = simulator(&window);
    for text in ["Tu nombre en este servidor", "Nombre nuevo"] {
        interface
            .find(text)
            .unwrap_or_else(|error| panic!("{text}: {error}"));
    }

    let (app, _outbox) = no_channel_app(Mode::Light);
    let window = self::window(app, Language::Spanish);
    let mut interface = simulator(&window);
    for text in [
        "No hay un canal abierto",
        "No eres miembro de ningún canal. Únete a un canal o crea uno.",
    ] {
        interface
            .find(text)
            .unwrap_or_else(|error| panic!("{text}: {error}"));
    }

    let window = self::window(settings_app(Mode::Light), Language::Spanish);
    let mut interface = simulator(&window);
    for text in [
        "Configuración",
        "Idioma del sistema",
        "24 horas",
        "Mostrar segundos",
        "Más pequeño",
        "Consola del protocolo (tecla F12)",
        "Volver",
    ] {
        interface
            .find(text)
            .unwrap_or_else(|error| panic!("{text}: {error}"));
    }
}

#[test]
fn each_screen_draws_in_light_and_dark_mode() {
    let directory = std::env::var_os(SCREENSHOT_VARIABLE).map(PathBuf::from);

    for (mode, mode_name) in [(Mode::Light, "light"), (Mode::Dark, "dark")] {
        let screens = [
            (
                "stopped",
                App::stopped(mode, StartProblem::InUse),
                Language::English,
            ),
            ("connect", connect_app(mode), Language::English),
            ("servers", list_app(mode), Language::English),
            (
                "certificate",
                certificate_app(mode, false),
                Language::English,
            ),
            (
                "certificate-changed",
                certificate_app(mode, true),
                Language::English,
            ),
            ("connecting", connecting_app(mode), Language::English),
            ("chat", chat_app(mode).0, Language::English),
            ("chat-es", chat_app(mode).0, Language::Spanish),
            (
                "chat-owner",
                chat_app_as(mode, Role::Owner).0,
                Language::English,
            ),
            (
                "chat-channels",
                channels_app(mode, Role::Owner).0,
                Language::English,
            ),
            ("no-channel", no_channel_app(mode).0, Language::English),
            ("no-channel-es", no_channel_app(mode).0, Language::Spanish),
            (
                "channels",
                channels_panel_app(mode, Some(three_channels())).0,
                Language::English,
            ),
            (
                "channels-es",
                channels_panel_app(mode, None).0,
                Language::Spanish,
            ),
            (
                "invitations",
                invitations_app(mode, Some(two_invitations())).0,
                Language::English,
            ),
            (
                "invitations-empty",
                invitations_app(mode, Some(Vec::new())).0,
                Language::Spanish,
            ),
            ("account", account_app(mode).0, Language::English),
            ("settings", settings_app(mode), Language::English),
            ("settings-es", settings_app(mode), Language::Spanish),
            ("console", console_app(mode).0, Language::English),
            ("profiles", profiles_app(mode), Language::English),
            (
                "profiles-rename",
                profiles_app_with(mode, vec![profiles::Message::Rename(profile(2))]),
                Language::English,
            ),
            (
                "profiles-delete",
                profiles_app_with(mode, vec![profiles::Message::Delete(profile(2))]),
                Language::Spanish,
            ),
        ];
        for (screen, app, language) in screens {
            let window = window(app, language);
            let mut interface = simulator(&window);
            let snapshot = interface
                .snapshot(&view::theme(mode))
                .unwrap_or_else(|error| panic!("{screen} {mode_name}: {error}"));
            if let Some(directory) = &directory {
                let path = directory.join(format!("{screen}-{mode_name}.png"));
                snapshot
                    .matches_image(path)
                    .expect("the screenshot can be written");
            }
        }
    }
}
