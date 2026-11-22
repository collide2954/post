pub mod chat;
pub mod servers;
pub mod start;

use post_client::{Event, KnownServer, Link};
use post_proto::ErrorCode;

use self::chat::Chat;
use self::servers::{Problem, Servers, Target};
use self::start::StartProblem;
use crate::i18n::Text;
use crate::palette::Mode;

#[derive(Debug, Clone)]
pub enum Message {
    Servers(servers::Message),
    Chat(chat::Message),
    Connection(u64, Event),
    ServerNotSaved,
    ModeChanged(Mode),
    FocusNext,
    FocusPrevious,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    None,
    FocusName,
    FocusNext,
    FocusPrevious,
    Connected(KnownServer),
}

#[derive(Debug)]
pub struct App {
    mode: Mode,
    servers: Vec<KnownServer>,
    attempts: u64,
    screen: Screen,
}

#[derive(Debug)]
pub enum Screen {
    Stopped(StartProblem),
    Servers(Box<Servers>),
    Chat(Box<Chat>),
}

impl App {
    #[must_use]
    pub fn new(mode: Mode, servers: Vec<KnownServer>) -> Self {
        Self {
            mode,
            servers,
            attempts: 0,
            screen: Screen::Servers(Box::default()),
        }
    }

    #[must_use]
    pub fn stopped(mode: Mode, problem: StartProblem) -> Self {
        Self {
            mode,
            servers: Vec::new(),
            attempts: 0,
            screen: Screen::Stopped(problem),
        }
    }

    #[must_use]
    pub fn mode(&self) -> Mode {
        self.mode
    }

    #[must_use]
    pub fn screen(&self) -> &Screen {
        &self.screen
    }

    #[must_use]
    pub fn servers(&self) -> &[KnownServer] {
        &self.servers
    }

    #[must_use]
    pub fn target(&self) -> Option<&Target> {
        match &self.screen {
            Screen::Stopped(_) => None,
            Screen::Servers(servers) => servers.connecting(),
            Screen::Chat(chat) => Some(chat.target()),
        }
    }

    pub fn update(&mut self, message: Message) -> Effect {
        match message {
            Message::Servers(message) => match &mut self.screen {
                Screen::Servers(servers) => {
                    servers.update(message, &self.servers, &mut self.attempts)
                }
                Screen::Stopped(_) | Screen::Chat(_) => Effect::None,
            },
            Message::Chat(message) => self.chat(message),
            Message::Connection(attempt, event) => self.connection(attempt, event),
            Message::ServerNotSaved => {
                if let Screen::Chat(chat) = &mut self.screen {
                    chat.warn_that_the_server_is_not_saved();
                }
                Effect::None
            }
            Message::ModeChanged(mode) => {
                self.mode = mode;
                Effect::None
            }
            Message::FocusNext => Effect::FocusNext,
            Message::FocusPrevious => Effect::FocusPrevious,
        }
    }

    fn chat(&mut self, message: chat::Message) -> Effect {
        let Screen::Chat(chat) = &mut self.screen else {
            return Effect::None;
        };
        match message {
            chat::Message::DraftChanged(draft) => chat.edit(draft),
            chat::Message::Send => chat.send(),
            chat::Message::Leave => return self.leave(None),
        }
        Effect::None
    }

    fn connection(&mut self, attempt: u64, event: Event) -> Effect {
        if self.target().is_none_or(|target| target.attempt != attempt) {
            return Effect::None;
        }
        match event {
            Event::Connected(link) => self.open_chat(link),
            Event::Received(message) => {
                if let Screen::Chat(chat) = &mut self.screen {
                    chat.receive(message);
                }
                Effect::None
            }
            Event::Disconnected(reason) => {
                let problem = Some(Problem::Disconnected(reason));
                if let Screen::Servers(servers) = &mut self.screen {
                    return servers.stop(problem);
                }
                self.leave(problem)
            }
        }
    }

    fn open_chat(&mut self, link: Link) -> Effect {
        let Screen::Servers(servers) = &mut self.screen else {
            return Effect::None;
        };
        let Some((form, target)) = servers.connected() else {
            return Effect::None;
        };
        let server = KnownServer {
            address: target.destination.address().to_string(),
            key: *link.server(),
            name: link.name().clone(),
            certificate: link
                .certificate()
                .map(|fingerprint| fingerprint.to_string()),
        };
        self.screen = Screen::Chat(Box::new(Chat::new(form, target, link)));
        server.clone().remember_in(&mut self.servers);
        Effect::Connected(server)
    }

    fn leave(&mut self, problem: Option<Problem>) -> Effect {
        let screen = std::mem::replace(&mut self.screen, Screen::Servers(Box::default()));
        let Screen::Chat(chat) = screen else {
            self.screen = screen;
            return Effect::None;
        };
        let mut form = chat.into_form();
        form.problem = problem;
        self.screen = Screen::Servers(Box::new(Servers::with(form)));
        Effect::FocusName
    }
}

#[must_use]
pub fn error_text(code: ErrorCode) -> Text {
    match code {
        ErrorCode::UnsupportedProtocol => Text::ErrorUnsupportedProtocol,
        ErrorCode::InvalidName => Text::ErrorInvalidName,
        ErrorCode::NameTaken => Text::ErrorNameTaken,
        ErrorCode::NoSuchChannel => Text::ErrorNoSuchChannel,
        ErrorCode::EmptyText => Text::ErrorEmptyText,
        ErrorCode::TextTooLong => Text::ErrorTextTooLong,
        ErrorCode::WrongAddress => Text::ErrorWrongAddress,
        ErrorCode::AccountExists => Text::ErrorAccountExists,
        ErrorCode::InvalidInvitation => Text::ErrorInvalidInvitation,
        ErrorCode::MissingPart => Text::ProblemMissingParts,
        ErrorCode::ServerError => Text::ErrorServer,
        ErrorCode::NotPermitted => Text::ErrorNotPermitted,
        ErrorCode::TooSoon => Text::ErrorTooSoon,
        ErrorCode::BadMessage
        | ErrorCode::HelloRequired
        | ErrorCode::DuplicateHello
        | ErrorCode::IdentifyRequired
        | ErrorCode::DuplicateIdentify
        | ErrorCode::InvalidProof
        | ErrorCode::UnknownAccount => Text::ErrorOther,
    }
}

#[cfg(test)]
mod tests {
    use post_client::{Disconnect, Entry, Event, Fingerprint, KnownServer, Link};
    use post_proto::{ClientMessage, ErrorCode, Key, Name, Role, ServerMessage, Timestamp};

    use super::servers::Problem;
    use super::start::StartProblem;
    use super::{App, Effect, Message, Screen, chat, error_text, servers};
    use crate::i18n::Text;
    use crate::palette::Mode;

    type Outbox = Box<dyn FnMut() -> Option<ClientMessage>>;

    fn name(text: &str) -> Name {
        Name::parse(text).expect(text)
    }

    fn key(number: u8) -> Key {
        Key::from_bytes([number; 32])
    }

    fn server(address: &str, number: u8, who: &str) -> KnownServer {
        KnownServer {
            address: address.to_owned(),
            key: key(number),
            name: name(who),
            certificate: None,
        }
    }

    fn app() -> App {
        App::new(Mode::Light, Vec::new())
    }

    fn typed(app: &mut App, address: &str, name: &str) {
        let address = servers::Message::AddressChanged(address.to_owned());
        app.update(Message::Servers(address));
        let name = servers::Message::NameChanged(name.to_owned());
        app.update(Message::Servers(name));
    }

    fn connect(app: &mut App) -> Effect {
        app.update(Message::Servers(servers::Message::Connect))
    }

    fn leave(app: &mut App) -> Effect {
        app.update(Message::Chat(chat::Message::Leave))
    }

    fn connecting(who: &str) -> App {
        let mut app = app();
        typed(&mut app, "localhost:7678", who);
        assert_eq!(connect(&mut app), Effect::None);
        assert!(app.target().is_some());
        app
    }

    fn connection(app: &App, event: Event) -> Message {
        let attempt = app.target().expect("a connection is in progress").attempt;
        Message::Connection(attempt, event)
    }

    fn chatting(who: &str, capacity: usize) -> (App, Outbox) {
        let mut app = connecting(who);
        let (link, mut messages) = Link::channel(name(who), Role::Member, key(1), capacity);
        let message = connection(&app, Event::Connected(link));
        assert_eq!(
            app.update(message),
            Effect::Connected(server("localhost:7678", 1, who))
        );
        assert!(matches!(app.screen(), Screen::Chat(_)));
        (app, Box::new(move || messages.try_recv().ok()))
    }

    fn form_problem(app: &App) -> Option<Problem> {
        match app.screen() {
            Screen::Servers(servers) => servers.form().problem,
            _ => panic!("the application is not on the servers screen"),
        }
    }

    fn entries(app: &App) -> Vec<Entry> {
        match app.screen() {
            Screen::Chat(chat) => chat.session().entries().to_vec(),
            _ => panic!("the application is not on the chat screen"),
        }
    }

    fn notice(app: &App) -> Option<Text> {
        match app.screen() {
            Screen::Chat(chat) => chat.notice(),
            _ => panic!("the application is not on the chat screen"),
        }
    }

    fn joined(who: &str) -> ServerMessage {
        ServerMessage::Join {
            channel: "lobby".to_owned(),
            name: name(who),
            time: Timestamp::parse("2026-11-21T14:02:00.000Z").expect("valid"),
        }
    }

    #[test]
    fn starts_on_the_servers_screen_with_the_servers_of_the_profile() {
        let known = vec![server("localhost:7678", 1, "ana")];
        let app = App::new(Mode::Dark, known.clone());

        let Screen::Servers(servers) = app.screen() else {
            panic!("the application starts on the servers screen");
        };
        assert_eq!(servers.form().address, "localhost:7678");
        assert_eq!(app.mode(), Mode::Dark);
        assert_eq!(app.target(), None);
        assert_eq!(app.servers(), known);
    }

    #[test]
    fn gives_each_connection_a_new_number() {
        let mut app = connecting("ana");
        assert_eq!(app.target().map(|target| target.attempt), Some(1));
        let cancel = Message::Servers(servers::Message::Cancel);
        assert_eq!(app.update(cancel), Effect::FocusName);
        assert_eq!(app.target(), None);

        connect(&mut app);

        assert_eq!(app.target().expect("a second connection").attempt, 2);
    }

    #[test]
    fn gives_the_key_of_a_known_server_to_the_connection() {
        let mut app = App::new(Mode::Light, vec![server("localhost:7678", 2, "ana")]);

        connect(&mut app);

        let target = app.target().expect("a connection is in progress");
        assert_eq!(target.trust.key, Some(key(2)));
    }

    #[test]
    fn remembers_the_certificate_that_the_connection_accepted() {
        let mut app = connecting("ana");
        let (link, _messages) = Link::channel(name("ana"), Role::Member, key(1), 4);
        let certificate = Fingerprint::from_bytes([9; 32]);
        let link = link.with_certificate(Some(certificate));

        let message = connection(&app, Event::Connected(link));
        let effect = app.update(message);

        let expected = KnownServer {
            certificate: Some("09".repeat(32)),
            ..server("localhost:7678", 1, "ana")
        };
        assert_eq!(effect, Effect::Connected(expected.clone()));
        assert_eq!(app.servers(), [expected]);
    }

    #[test]
    fn opens_the_chat_screen_when_the_server_accepts_the_user() {
        let mut app = connecting("ana");
        let before = app.target().cloned();
        let (link, _messages) = Link::channel(name("ana-2"), Role::Owner, key(1), 4);

        let message = connection(&app, Event::Connected(link));
        let effect = app.update(message);

        assert_eq!(
            effect,
            Effect::Connected(server("localhost:7678", 1, "ana-2"))
        );
        let Screen::Chat(chat) = app.screen() else {
            panic!("the application shows the chat");
        };
        assert_eq!(chat.session().name(), &name("ana-2"));
        assert_eq!(app.target().cloned(), before);
    }

    #[test]
    fn knows_the_server_at_the_next_connection() {
        let known = vec![
            server("chat.example.org:443", 2, "ana"),
            server("localhost:7678", 3, "ana-old"),
        ];
        let mut app = App::new(Mode::Light, known);
        connect(&mut app);
        let (link, _messages) = Link::channel(name("ana"), Role::Member, key(3), 4);
        let message = connection(&app, Event::Connected(link));
        app.update(message);

        assert_eq!(
            app.servers(),
            [
                server("localhost:7678", 3, "ana"),
                server("chat.example.org:443", 2, "ana"),
            ]
        );

        leave(&mut app);
        connect(&mut app);
        let target = app.target().expect("a second connection");
        assert_eq!(target.trust.key, Some(key(3)));
    }

    #[test]
    fn records_what_the_server_sends() {
        let (mut app, _outbox) = chatting("ana", 4);

        let message = connection(&app, Event::Received(joined("ben")));
        assert_eq!(app.update(message), Effect::None);

        assert_eq!(
            entries(&app),
            [Entry::Joined {
                time: Timestamp::parse("2026-11-21T14:02:00.000Z").expect("valid"),
                name: name("ben"),
            }]
        );
    }

    #[test]
    fn goes_back_to_the_servers_screen_when_the_connection_stops() {
        for reason in [
            Disconnect::Unreachable,
            Disconnect::Refused(ErrorCode::NameTaken),
        ] {
            let mut app = connecting("ana");

            let message = connection(&app, Event::Disconnected(reason));
            assert_eq!(app.update(message), Effect::FocusName);

            assert_eq!(form_problem(&app), Some(Problem::Disconnected(reason)));
            assert_eq!(app.target(), None);
            let Screen::Servers(servers) = app.screen() else {
                panic!("the application is on the servers screen");
            };
            assert_eq!(servers.form().address, "localhost:7678");
            assert_eq!(servers.form().name, "ana");
        }

        let (mut app, _outbox) = chatting("ana", 4);
        let message = connection(&app, Event::Disconnected(Disconnect::Closed));
        assert_eq!(app.update(message), Effect::FocusName);
        assert_eq!(
            form_problem(&app),
            Some(Problem::Disconnected(Disconnect::Closed))
        );
    }

    #[test]
    fn leaves_the_chat_when_the_user_asks() {
        let (mut app, _outbox) = chatting("ana", 4);

        assert_eq!(leave(&mut app), Effect::FocusName);

        assert_eq!(form_problem(&app), None);
        assert_eq!(app.target(), None);
        let Screen::Servers(servers) = app.screen() else {
            panic!("the application is on the servers screen");
        };
        assert_eq!(servers.form().name, "ana");
    }

    #[test]
    fn ignores_an_event_of_an_earlier_connection() {
        let mut app = connecting("ana");
        let old = app.target().expect("a connection is in progress").attempt;
        app.update(Message::Servers(servers::Message::Cancel));
        connect(&mut app);
        let (link, _messages) = Link::channel(name("ana"), Role::Member, key(1), 4);

        for event in [
            Event::Connected(link),
            Event::Received(joined("ben")),
            Event::Disconnected(Disconnect::Closed),
        ] {
            assert_eq!(app.update(Message::Connection(old, event)), Effect::None);
            assert!(matches!(app.screen(), Screen::Servers(_)));
            assert_eq!(app.target().map(|target| target.attempt), Some(old + 1));
        }

        let (mut app, _outbox) = chatting("ana", 4);
        let stale = Message::Connection(99, Event::Disconnected(Disconnect::Closed));
        assert_eq!(app.update(stale), Effect::None);
        assert!(matches!(app.screen(), Screen::Chat(_)));
        let stale = Message::Connection(99, Event::Received(joined("ben")));
        app.update(stale);
        assert_eq!(entries(&app), []);
    }

    #[test]
    fn ignores_a_connection_event_when_it_does_not_connect() {
        let mut app = app();

        let event = Event::Disconnected(Disconnect::Closed);
        assert_eq!(app.update(Message::Connection(0, event)), Effect::None);

        assert_eq!(form_problem(&app), None);
    }

    #[test]
    fn gives_the_messages_of_the_chat_screen_to_the_chat() {
        let (mut app, mut outbox) = chatting("ana", 4);
        let draft = chat::Message::DraftChanged("Hello".to_owned());
        assert_eq!(app.update(Message::Chat(draft)), Effect::None);

        assert_eq!(app.update(Message::Chat(chat::Message::Send)), Effect::None);

        assert_eq!(
            outbox(),
            Some(ClientMessage::Send {
                reference: Some(1),
                channel: "lobby".to_owned(),
                text: "Hello".to_owned(),
            })
        );
    }

    #[test]
    fn tells_the_user_that_the_profile_did_not_save_the_server() {
        let (mut app, _outbox) = chatting("ana", 4);

        assert_eq!(app.update(Message::ServerNotSaved), Effect::None);

        assert_eq!(notice(&app), Some(Text::ChatServerNotSaved));
    }

    #[test]
    fn changes_between_light_and_dark_mode() {
        let mut app = app();

        assert_eq!(app.update(Message::ModeChanged(Mode::Dark)), Effect::None);

        assert_eq!(app.mode(), Mode::Dark);
    }

    #[test]
    fn moves_the_focus_when_the_user_asks() {
        let mut app = app();

        assert_eq!(app.update(Message::FocusNext), Effect::FocusNext);
        assert_eq!(app.update(Message::FocusPrevious), Effect::FocusPrevious);
    }

    #[test]
    fn ignores_a_message_that_is_for_a_different_screen() {
        let mut app = app();
        let draft = chat::Message::DraftChanged("x".to_owned());
        assert_eq!(app.update(Message::Chat(draft)), Effect::None);
        assert_eq!(app.update(Message::Chat(chat::Message::Send)), Effect::None);
        assert_eq!(leave(&mut app), Effect::None);
        assert_eq!(app.update(Message::ServerNotSaved), Effect::None);
        assert!(matches!(app.screen(), Screen::Servers(_)));

        let (mut app, _outbox) = chatting("ana", 4);
        typed(&mut app, "x", "x");
        assert_eq!(connect(&mut app), Effect::None);
        assert!(matches!(app.screen(), Screen::Chat(_)));
        leave(&mut app);
        let Screen::Servers(servers) = app.screen() else {
            panic!("the application is on the servers screen");
        };
        assert_eq!(servers.form().address, "localhost:7678");
        assert_eq!(servers.form().name, "ana");
    }

    #[test]
    fn stays_stopped_when_no_profile_is_open() {
        let mut app = App::stopped(Mode::Light, StartProblem::InUse);
        assert_eq!(app.servers(), []);

        typed(&mut app, "localhost", "ana");
        assert_eq!(connect(&mut app), Effect::None);
        assert_eq!(leave(&mut app), Effect::None);
        assert_eq!(app.update(Message::Chat(chat::Message::Send)), Effect::None);
        let event = Event::Disconnected(Disconnect::Closed);
        assert_eq!(app.update(Message::Connection(0, event)), Effect::None);
        app.update(Message::ModeChanged(Mode::Dark));

        assert!(matches!(app.screen(), Screen::Stopped(StartProblem::InUse)));
        assert_eq!(app.target(), None);
        assert_eq!(app.mode(), Mode::Dark);
    }

    #[test]
    fn each_error_code_has_a_text() {
        let codes = [
            (
                ErrorCode::UnsupportedProtocol,
                Text::ErrorUnsupportedProtocol,
            ),
            (ErrorCode::InvalidName, Text::ErrorInvalidName),
            (ErrorCode::NameTaken, Text::ErrorNameTaken),
            (ErrorCode::NoSuchChannel, Text::ErrorNoSuchChannel),
            (ErrorCode::EmptyText, Text::ErrorEmptyText),
            (ErrorCode::TextTooLong, Text::ErrorTextTooLong),
            (ErrorCode::WrongAddress, Text::ErrorWrongAddress),
            (ErrorCode::AccountExists, Text::ErrorAccountExists),
            (ErrorCode::InvalidInvitation, Text::ErrorInvalidInvitation),
            (ErrorCode::MissingPart, Text::ProblemMissingParts),
            (ErrorCode::ServerError, Text::ErrorServer),
            (ErrorCode::NotPermitted, Text::ErrorNotPermitted),
            (ErrorCode::TooSoon, Text::ErrorTooSoon),
            (ErrorCode::BadMessage, Text::ErrorOther),
            (ErrorCode::HelloRequired, Text::ErrorOther),
            (ErrorCode::DuplicateHello, Text::ErrorOther),
            (ErrorCode::IdentifyRequired, Text::ErrorOther),
            (ErrorCode::DuplicateIdentify, Text::ErrorOther),
            (ErrorCode::InvalidProof, Text::ErrorOther),
            (ErrorCode::UnknownAccount, Text::ErrorOther),
        ];
        for (code, text) in codes {
            assert_eq!(error_text(code), text, "{code:?}");
        }
    }
}
