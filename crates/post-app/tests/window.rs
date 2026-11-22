use std::borrow::Cow;
use std::path::PathBuf;

use iced::keyboard::key::Named;
use iced::{Pixels, Settings, Size};
use iced_test::{Simulator, selector};
use jiff::tz::TimeZone;
use post_app::app::start::StartProblem;
use post_app::app::{App, Message, chat, servers};
use post_app::i18n::{Language, Strings};
use post_app::palette::Mode;
use post_app::view::{self, Window};
use post_client::{Event, Link};
use post_proto::{ClientMessage, ErrorCode, Key, Name, Role, ServerMessage, Timestamp};

const SCREENSHOT_VARIABLE: &str = "POST_SCREENSHOTS";

type Outbox = Box<dyn FnMut() -> Option<ClientMessage>>;

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
    }
}

fn simulator(window: &Window) -> Simulator<'_, Message> {
    let settings = Settings {
        default_font: view::TEXT_FONT,
        default_text_size: Pixels(14.0),
        fonts: view::FONTS
            .iter()
            .map(|font| Cow::Borrowed(*font))
            .collect(),
        ..Settings::default()
    };
    Simulator::with_size(settings, Size::new(900.0, 600.0), view::view(window))
}

fn new_app(mode: Mode) -> App {
    App::new(mode, Vec::new())
}

fn connect_app(mode: Mode) -> App {
    let mut app = new_app(mode);
    app.update(Message::Servers(servers::Message::NameChanged(
        "ana".to_owned(),
    )));
    app
}

fn connecting_app(mode: Mode) -> App {
    let mut app = connect_app(mode);
    app.update(Message::Servers(servers::Message::Connect));
    app
}

fn chat_app(mode: Mode) -> (App, Outbox) {
    let mut app = connecting_app(mode);
    let attempt = app.target().expect("a connection is in progress").attempt;
    let (link, mut messages) =
        Link::channel(name("ana"), Role::Member, Key::from_bytes([1; 32]), 8);
    app.update(Message::Connection(attempt, Event::Connected(link)));

    let join = |who: &str, clock: &str| ServerMessage::Join {
        channel: "lobby".to_owned(),
        name: name(who),
        time: time(clock),
    };
    let message = |from: &str, clock: &str, text: &str| ServerMessage::Message {
        reference: None,
        channel: "lobby".to_owned(),
        from: name(from),
        time: time(clock),
        text: text.to_owned(),
    };
    let received = [
        join("ana", "14:02:00"),
        join("ben", "14:02:05"),
        message("ana", "14:02:07", "Hello"),
        message("ben", "14:03:10", "Hi. Does it work?"),
        message("ana", "14:03:20", "Yes."),
        message(
            "ana",
            "14:03:31",
            "Two people, one channel, and a server on this computer.",
        ),
        ServerMessage::Leave {
            channel: "lobby".to_owned(),
            name: name("ben"),
            time: time("14:05:00"),
        },
    ];
    for message in received {
        app.update(Message::Connection(attempt, Event::Received(message)));
    }
    (app, Box::new(move || messages.try_recv().ok()))
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
fn the_server_field_shows_the_local_address() {
    let window = window(new_app(Mode::Light), Language::English);
    let mut interface = simulator(&window);

    interface
        .click("localhost:7678")
        .expect("the server field shows the default address");
    interface.typewrite("0");

    let messages: Vec<Message> = interface.into_messages().collect();
    assert!(
        matches!(messages.as_slice(), [Message::Servers(servers::Message::AddressChanged(address))] if address == "localhost:76780"),
        "{messages:?}"
    );
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
    app.update(Message::Servers(servers::Message::NameChanged(
        "a".to_owned(),
    )));
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
        "#lobby",
        "ana · connected",
        "14:02",
        "ana joined",
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
    for text in ["ana · conectado", "ben entró", "ben salió", "Salir"] {
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
            ("connecting", connecting_app(mode), Language::English),
            ("chat", chat_app(mode).0, Language::English),
            ("chat-es", chat_app(mode).0, Language::Spanish),
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
