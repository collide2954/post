use post_client::{
    Destination, DestinationError, Disconnect, Event, KnownServer, Link, ProfileError, Session,
};
use post_proto::{DEFAULT_PORT, ErrorCode, Key, Name, NameError, TextError};

use crate::i18n::{Strings, Text};
use crate::palette::Mode;

#[derive(Debug, Clone)]
pub enum Message {
    AddressChanged(String),
    NameChanged(String),
    Connect,
    Leave,
    DraftChanged(String),
    Send,
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
    FocusDraft,
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
    Connect(Form),
    Connecting { form: Form, target: Target },
    Chat(Box<Chat>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartProblem {
    NoHome,
    InUse,
    Damaged(&'static str),
    Files(String),
}

#[derive(Debug, Clone, Default)]
pub struct Form {
    pub address: String,
    pub name: String,
    pub problem: Option<Problem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Target {
    pub attempt: u64,
    pub destination: Destination,
    pub name: Option<Name>,
    pub known: Option<Key>,
}

#[derive(Debug)]
pub struct Chat {
    form: Form,
    target: Target,
    link: Link,
    session: Session,
    draft: String,
    notice: Option<Notice>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Problem {
    Address(DestinationError),
    Name(NameError),
    Disconnected(Disconnect),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Notice {
    TooLong,
    NotSent,
    ServerNotSaved,
}

impl App {
    #[must_use]
    pub fn new(mode: Mode, servers: Vec<KnownServer>) -> Self {
        Self {
            mode,
            servers,
            attempts: 0,
            screen: Screen::Connect(Form {
                address: format!("localhost:{DEFAULT_PORT}"),
                name: String::new(),
                problem: None,
            }),
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
            Screen::Stopped(_) | Screen::Connect(_) => None,
            Screen::Connecting { target, .. } => Some(target),
            Screen::Chat(chat) => Some(&chat.target),
        }
    }

    pub fn update(&mut self, message: Message) -> Effect {
        match message {
            Message::AddressChanged(address) => self.edit_form(|form| form.address = address),
            Message::NameChanged(name) => self.edit_form(|form| form.name = name),
            Message::Connect => self.connect(),
            Message::Leave => self.leave(None),
            Message::DraftChanged(draft) => self.edit_chat(|chat| {
                chat.draft = draft;
                chat.notice = None;
            }),
            Message::Send => self.edit_chat(Chat::send),
            Message::Connection(attempt, event) => self.connection(attempt, event),
            Message::ServerNotSaved => {
                self.edit_chat(|chat| chat.notice = Some(Notice::ServerNotSaved))
            }
            Message::ModeChanged(mode) => {
                self.mode = mode;
                Effect::None
            }
            Message::FocusNext => Effect::FocusNext,
            Message::FocusPrevious => Effect::FocusPrevious,
        }
    }

    fn edit_form(&mut self, edit: impl FnOnce(&mut Form)) -> Effect {
        if let Screen::Connect(form) = &mut self.screen {
            edit(form);
            form.problem = None;
        }
        Effect::None
    }

    fn edit_chat(&mut self, edit: impl FnOnce(&mut Chat)) -> Effect {
        if let Screen::Chat(chat) = &mut self.screen {
            edit(chat);
        }
        Effect::None
    }

    fn connect(&mut self) -> Effect {
        let Screen::Connect(form) = &mut self.screen else {
            return Effect::None;
        };
        let destination = match Destination::parse(&form.address) {
            Ok(destination) => destination,
            Err(error) => {
                form.problem = Some(Problem::Address(error));
                return Effect::None;
            }
        };
        let name = match Some(form.name.trim())
            .filter(|name| !name.is_empty())
            .map(Name::parse)
            .transpose()
        {
            Ok(name) => name,
            Err(error) => {
                form.problem = Some(Problem::Name(error));
                return Effect::None;
            }
        };
        let address = destination.address().to_string();
        let known = self
            .servers
            .iter()
            .find(|server| server.address == address)
            .map(|server| server.key);
        self.attempts += 1;
        let target = Target {
            attempt: self.attempts,
            destination,
            name,
            known,
        };
        self.screen = Screen::Connecting {
            form: std::mem::take(form),
            target,
        };
        Effect::None
    }

    fn leave(&mut self, problem: Option<Problem>) -> Effect {
        let form = match &mut self.screen {
            Screen::Stopped(_) | Screen::Connect(_) => return Effect::None,
            Screen::Connecting { form, .. } => form,
            Screen::Chat(chat) => &mut chat.form,
        };
        let mut form = std::mem::take(form);
        form.problem = problem;
        self.screen = Screen::Connect(form);
        Effect::FocusName
    }

    fn connection(&mut self, attempt: u64, event: Event) -> Effect {
        if self.target().is_none_or(|target| target.attempt != attempt) {
            return Effect::None;
        }
        match event {
            Event::Connected(link) => self.open_chat(link),
            Event::Received(message) => self.edit_chat(|chat| chat.session.apply(message)),
            Event::Disconnected(reason) => self.leave(Some(Problem::Disconnected(reason))),
        }
    }

    fn open_chat(&mut self, link: Link) -> Effect {
        let Screen::Connecting { form, target } = &mut self.screen else {
            return Effect::None;
        };
        let server = link.server().map(|key| KnownServer {
            address: target.destination.address().to_string(),
            key: *key,
            name: link.name().clone(),
            certificate: None,
        });
        self.screen = Screen::Chat(Box::new(Chat {
            form: std::mem::take(form),
            target: target.clone(),
            session: Session::new(link.name().clone()),
            link,
            draft: String::new(),
            notice: None,
        }));
        let Some(server) = server else {
            return Effect::FocusDraft;
        };
        server.clone().remember_in(&mut self.servers);
        Effect::Connected(server)
    }
}

impl Chat {
    #[must_use]
    pub fn session(&self) -> &Session {
        &self.session
    }

    #[must_use]
    pub fn draft(&self) -> &str {
        &self.draft
    }

    #[must_use]
    pub fn notice(&self) -> Option<Text> {
        match self.notice {
            Some(Notice::TooLong) => Some(Text::ErrorTextTooLong),
            Some(Notice::NotSent) => Some(Text::ChatNotSent),
            Some(Notice::ServerNotSaved) => Some(Text::ChatServerNotSaved),
            None => self.session.problem().map(error_text),
        }
    }

    fn send(&mut self) {
        match self.session.compose(&self.draft) {
            Ok(message) => {
                if self.link.send(message) {
                    self.draft.clear();
                    self.notice = None;
                } else {
                    self.notice = Some(Notice::NotSent);
                }
            }
            Err(TextError::TooLong) => self.notice = Some(Notice::TooLong),
            Err(TextError::Empty) => {}
        }
    }
}

impl StartProblem {
    #[must_use]
    pub fn describe(&self, strings: &Strings) -> String {
        match self {
            Self::NoHome => strings.text(Text::StartNoHome),
            Self::InUse => strings.text(Text::StartProfileInUse),
            Self::Damaged(file) => strings.with(Text::StartProfileDamaged, "file", file),
            Self::Files(reason) => strings.with(Text::StartProfileFiles, "reason", reason),
        }
    }
}

impl From<ProfileError> for StartProblem {
    fn from(error: ProfileError) -> Self {
        match error {
            ProfileError::InUse => Self::InUse,
            ProfileError::Damaged(file) => Self::Damaged(file),
            other => Self::Files(other.to_string()),
        }
    }
}

impl Problem {
    #[must_use]
    pub fn text(self) -> Text {
        match self {
            Self::Address(DestinationError::Empty) => Text::ProblemAddressEmpty,
            Self::Address(DestinationError::Invalid) => Text::ProblemAddressInvalid,
            Self::Address(DestinationError::InvalidPort) => Text::ProblemAddressPort,
            Self::Address(DestinationError::InvalidLink) => Text::ProblemAddressLink,
            Self::Address(DestinationError::NotLocal) => Text::ProblemAddressNotLocal,
            Self::Name(NameError::InvalidCharacter) => Text::ProblemNameCharacters,
            Self::Name(NameError::TooShort) => Text::ProblemNameShort,
            Self::Name(NameError::TooLong) => Text::ProblemNameLong,
            Self::Disconnected(Disconnect::Unreachable) => Text::ProblemUnreachable,
            Self::Disconnected(Disconnect::Closed) => Text::ProblemClosed,
            Self::Disconnected(Disconnect::Protocol) => Text::ProblemProtocol,
            Self::Disconnected(Disconnect::FalseServer) => Text::ProblemFalseServer,
            Self::Disconnected(Disconnect::DifferentServer) => Text::ProblemDifferentServer,
            Self::Disconnected(Disconnect::MissingParts) => Text::ProblemMissingParts,
            Self::Disconnected(Disconnect::NameRequired) => Text::ProblemNameRequired,
            Self::Disconnected(Disconnect::InvitationRequired) => Text::ProblemInvitationRequired,
            Self::Disconnected(Disconnect::NoTls) => Text::ProblemNoTls,
            Self::Disconnected(Disconnect::Refused(code)) => error_text(code),
        }
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
    use post_client::{
        Destination, DestinationError, Disconnect, Entry, Event, KnownServer, Link, NameProblem,
        ProfileError,
    };
    use post_proto::{
        ClientMessage, ErrorCode, Key, Name, NameError, Role, ServerMessage, Timestamp,
    };

    use super::{App, Effect, Message, Problem, Screen, StartProblem, error_text};
    use crate::i18n::{Language, Strings, Text};
    use crate::palette::Mode;

    const LINK: &str = "http://localhost:7678/i/7Hq2kdX9mPq4";

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
        app.update(Message::AddressChanged(address.to_owned()));
        app.update(Message::NameChanged(name.to_owned()));
    }

    fn connecting(who: &str) -> App {
        let mut app = app();
        typed(&mut app, "localhost:7678", who);
        assert_eq!(app.update(Message::Connect), Effect::None);
        assert!(matches!(app.screen(), Screen::Connecting { .. }));
        app
    }

    fn connection(app: &App, event: Event) -> Message {
        let attempt = app.target().expect("a connection is in progress").attempt;
        Message::Connection(attempt, event)
    }

    type Outbox = Box<dyn FnMut() -> Option<ClientMessage>>;

    fn chatting(who: &str, capacity: usize) -> (App, Outbox) {
        let mut app = connecting(who);
        let (link, mut messages) = Link::channel(name(who), capacity);
        let message = connection(
            &app,
            Event::Connected(link.of_account(Role::Member, key(1))),
        );
        assert_eq!(
            app.update(message),
            Effect::Connected(server("localhost:7678", 1, who))
        );
        assert!(matches!(app.screen(), Screen::Chat(_)));
        (app, Box::new(move || messages.try_recv().ok()))
    }

    fn form_problem(app: &App) -> Option<Problem> {
        match app.screen() {
            Screen::Connect(form) => form.problem,
            _ => panic!("the application is not on the connect screen"),
        }
    }

    fn entries(app: &App) -> Vec<Entry> {
        match app.screen() {
            Screen::Chat(chat) => chat.session().entries().to_vec(),
            _ => panic!("the application is not on the chat screen"),
        }
    }

    fn draft(app: &App) -> String {
        match app.screen() {
            Screen::Chat(chat) => chat.draft().to_owned(),
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
    fn starts_on_the_connect_screen_with_the_local_address() {
        let known = vec![server("localhost:7678", 1, "ana")];
        let app = App::new(Mode::Dark, known.clone());

        let Screen::Connect(form) = app.screen() else {
            panic!("the application starts on the connect screen");
        };
        assert_eq!(form.address, "localhost:7678");
        assert_eq!(form.name, "");
        assert_eq!(form.problem, None);
        assert_eq!(app.mode(), Mode::Dark);
        assert_eq!(app.target(), None);
        assert_eq!(app.servers(), known);
    }

    #[test]
    fn keeps_what_the_user_types_on_the_connect_screen() {
        let mut app = app();

        typed(&mut app, "127.0.0.1:9000", "ana");

        let Screen::Connect(form) = app.screen() else {
            panic!("the application is on the connect screen");
        };
        assert_eq!(form.address, "127.0.0.1:9000");
        assert_eq!(form.name, "ana");
    }

    #[test]
    fn tells_the_user_about_an_address_that_it_cannot_use() {
        for (address, error) in [
            ("", DestinationError::Empty),
            ("local host", DestinationError::Invalid),
            ("localhost:0", DestinationError::InvalidPort),
            ("localhost:7678/i/short", DestinationError::InvalidLink),
            ("http://chat.example.org", DestinationError::NotLocal),
        ] {
            let mut app = app();
            typed(&mut app, address, "ana");

            assert_eq!(app.update(Message::Connect), Effect::None);

            assert_eq!(
                form_problem(&app),
                Some(Problem::Address(error)),
                "{address}"
            );
            assert_eq!(app.target(), None);
        }
    }

    #[test]
    fn tells_the_user_about_a_name_that_breaks_the_rules() {
        for (who, error) in [
            ("a", NameError::TooShort),
            ("Ana", NameError::InvalidCharacter),
            ("abcdefghij0123456789x", NameError::TooLong),
        ] {
            let mut app = app();
            typed(&mut app, "localhost", who);

            app.update(Message::Connect);

            assert_eq!(form_problem(&app), Some(Problem::Name(error)), "{who}");
            assert_eq!(app.target(), None);
        }
    }

    #[test]
    fn removes_the_problem_when_the_user_types() {
        let mut app = app();
        typed(&mut app, "", "ana");
        app.update(Message::Connect);
        app.update(Message::AddressChanged("l".to_owned()));
        assert_eq!(form_problem(&app), None);

        typed(&mut app, "localhost", "a");
        app.update(Message::Connect);
        app.update(Message::NameChanged("an".to_owned()));
        assert_eq!(form_problem(&app), None);
    }

    #[test]
    fn starts_a_connection_with_an_invite_link_and_a_name() {
        let mut app = app();
        typed(&mut app, &format!(" {LINK} "), "  ana  ");

        app.update(Message::Connect);

        let target = app.target().expect("a connection is in progress");
        assert_eq!(target.attempt, 1);
        assert_eq!(target.destination, Destination::parse(LINK).expect("valid"));
        assert_eq!(target.name, Some(name("ana")));
        assert_eq!(target.known, None);
        let Screen::Connecting { form, .. } = app.screen() else {
            panic!("the application connects");
        };
        assert_eq!(form.address, format!(" {LINK} "));
        assert_eq!(form.problem, None);
    }

    #[test]
    fn starts_a_connection_with_an_address_and_no_name() {
        for who in ["", "   "] {
            let mut app = app();
            typed(&mut app, " LOCALHOST:9000 ", who);

            app.update(Message::Connect);

            let target = app.target().expect("a connection is in progress");
            assert_eq!(
                target.destination,
                Destination::parse("localhost:9000").expect("valid")
            );
            assert_eq!(target.name, None, "{who:?}");
        }
    }

    #[test]
    fn gives_the_key_of_a_known_server_to_the_connection() {
        let known = vec![
            server("chat.example.org:443", 1, "ana"),
            server("localhost:7678", 2, "ana-dev"),
        ];
        for (address, expected) in [
            ("localhost", Some(key(2))),
            ("http://localhost:7678/", Some(key(2))),
            ("chat.example.org", Some(key(1))),
            ("localhost:9000", None),
            ("127.0.0.1", None),
        ] {
            let mut app = App::new(Mode::Light, known.clone());
            typed(&mut app, address, "");

            app.update(Message::Connect);

            let target = app.target().expect("a connection is in progress");
            assert_eq!(target.known, expected, "{address}");
        }
    }

    #[test]
    fn gives_each_connection_a_new_number() {
        let mut app = connecting("ana");
        assert_eq!(app.update(Message::Leave), Effect::FocusName);
        assert_eq!(app.target(), None);

        app.update(Message::Connect);

        assert_eq!(app.target().expect("a second connection").attempt, 2);
    }

    #[test]
    fn opens_the_chat_screen_when_the_server_accepts_the_user() {
        let mut app = connecting("ana");
        let before = app.target().cloned();
        let (link, _messages) = Link::channel(name("ana-2"), 4);

        let message = connection(&app, Event::Connected(link.of_account(Role::Owner, key(1))));
        let effect = app.update(message);

        assert_eq!(
            effect,
            Effect::Connected(server("localhost:7678", 1, "ana-2"))
        );
        let Screen::Chat(chat) = app.screen() else {
            panic!("the application shows the chat");
        };
        assert_eq!(chat.session().name(), &name("ana-2"));
        assert_eq!(chat.draft(), "");
        assert_eq!(chat.notice(), None);
        assert_eq!(app.target().cloned(), before);
    }

    #[test]
    fn opens_the_chat_screen_of_a_connection_without_an_account() {
        let mut app = connecting("ana");
        let (link, _messages) = Link::channel(name("ana"), 4);

        let message = connection(&app, Event::Connected(link));

        assert_eq!(app.update(message), Effect::FocusDraft);
        assert!(matches!(app.screen(), Screen::Chat(_)));
        assert_eq!(app.servers(), []);
    }

    #[test]
    fn knows_the_server_at_the_next_connection() {
        let known = vec![
            server("chat.example.org:443", 2, "ana"),
            server("localhost:7678", 3, "ana-old"),
        ];
        let mut app = App::new(Mode::Light, known);
        typed(&mut app, "localhost:7678", "");
        app.update(Message::Connect);
        let (link, _messages) = Link::channel(name("ana"), 4);
        let message = connection(
            &app,
            Event::Connected(link.of_account(Role::Member, key(3))),
        );
        app.update(message);

        assert_eq!(
            app.servers(),
            [
                server("localhost:7678", 3, "ana"),
                server("chat.example.org:443", 2, "ana"),
            ]
        );

        app.update(Message::Leave);
        app.update(Message::Connect);
        let target = app.target().expect("a second connection");
        assert_eq!(target.known, Some(key(3)));
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
    fn goes_back_to_the_connect_screen_when_the_connection_stops() {
        for reason in [
            Disconnect::Unreachable,
            Disconnect::Refused(ErrorCode::NameTaken),
        ] {
            let mut app = connecting("ana");

            let message = connection(&app, Event::Disconnected(reason));
            assert_eq!(app.update(message), Effect::FocusName);

            assert_eq!(form_problem(&app), Some(Problem::Disconnected(reason)));
            assert_eq!(app.target(), None);
            let Screen::Connect(form) = app.screen() else {
                panic!("the application is on the connect screen");
            };
            assert_eq!(form.address, "localhost:7678");
            assert_eq!(form.name, "ana");
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

        assert_eq!(app.update(Message::Leave), Effect::FocusName);

        assert_eq!(form_problem(&app), None);
        assert_eq!(app.target(), None);
        let Screen::Connect(form) = app.screen() else {
            panic!("the application is on the connect screen");
        };
        assert_eq!(form.name, "ana");
    }

    #[test]
    fn ignores_an_event_of_an_earlier_connection() {
        let mut app = connecting("ana");
        let old = app.target().expect("a connection is in progress").attempt;
        app.update(Message::Leave);
        app.update(Message::Connect);
        let (link, _messages) = Link::channel(name("ana"), 4);

        for event in [
            Event::Connected(link),
            Event::Received(joined("ben")),
            Event::Disconnected(Disconnect::Closed),
        ] {
            assert_eq!(app.update(Message::Connection(old, event)), Effect::None);
            assert!(matches!(app.screen(), Screen::Connecting { .. }));
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
    fn ignores_a_connection_event_on_the_connect_screen() {
        let mut app = app();

        let event = Event::Disconnected(Disconnect::Closed);
        assert_eq!(app.update(Message::Connection(0, event)), Effect::None);

        assert_eq!(form_problem(&app), None);
    }

    #[test]
    fn sends_the_draft_and_makes_the_field_empty() {
        let (mut app, mut outbox) = chatting("ana", 4);
        app.update(Message::DraftChanged("Hello".to_owned()));
        assert_eq!(draft(&app), "Hello");

        assert_eq!(app.update(Message::Send), Effect::None);

        assert_eq!(
            outbox(),
            Some(ClientMessage::Send {
                reference: Some(1),
                channel: "lobby".to_owned(),
                text: "Hello".to_owned(),
            })
        );
        assert_eq!(outbox(), None);
        assert_eq!(draft(&app), "");
        assert_eq!(notice(&app), None);
    }

    #[test]
    fn does_not_send_an_empty_draft() {
        let (mut app, mut outbox) = chatting("ana", 4);
        app.update(Message::DraftChanged("   ".to_owned()));

        app.update(Message::Send);

        assert_eq!(outbox(), None);
        assert_eq!(draft(&app), "   ");
        assert_eq!(notice(&app), None);
    }

    #[test]
    fn tells_the_user_that_a_draft_is_too_long() {
        let (mut app, mut outbox) = chatting("ana", 4);
        let long = "x".repeat(4001);
        app.update(Message::DraftChanged(long.clone()));

        app.update(Message::Send);

        assert_eq!(outbox(), None);
        assert_eq!(draft(&app), long);
        assert_eq!(notice(&app), Some(Text::ErrorTextTooLong));

        app.update(Message::DraftChanged("short".to_owned()));
        assert_eq!(notice(&app), None);
    }

    #[test]
    fn keeps_the_draft_when_the_connection_cannot_take_it() {
        let (mut app, mut outbox) = chatting("ana", 1);
        app.update(Message::DraftChanged("one".to_owned()));
        app.update(Message::Send);
        app.update(Message::DraftChanged("two".to_owned()));

        app.update(Message::Send);

        assert_eq!(draft(&app), "two");
        assert_eq!(notice(&app), Some(Text::ChatNotSent));
        assert!(outbox().is_some());
        assert_eq!(outbox(), None);
    }

    #[test]
    fn shows_an_error_from_the_server_until_the_user_sends_again() {
        let (mut app, _outbox) = chatting("ana", 4);

        let refused = ServerMessage::error(Some(1), ErrorCode::NoSuchChannel);
        let message = connection(&app, Event::Received(refused));
        app.update(message);
        assert_eq!(notice(&app), Some(Text::ErrorNoSuchChannel));

        app.update(Message::DraftChanged("Hello".to_owned()));
        assert_eq!(notice(&app), Some(Text::ErrorNoSuchChannel));
        app.update(Message::Send);
        assert_eq!(notice(&app), None);
    }

    #[test]
    fn tells_the_user_that_the_profile_did_not_save_the_server() {
        let (mut app, _outbox) = chatting("ana", 4);

        assert_eq!(app.update(Message::ServerNotSaved), Effect::None);
        assert_eq!(notice(&app), Some(Text::ChatServerNotSaved));

        app.update(Message::DraftChanged("Hello".to_owned()));
        assert_eq!(notice(&app), None);
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
        assert_eq!(
            app.update(Message::DraftChanged("x".to_owned())),
            Effect::None
        );
        assert_eq!(app.update(Message::Send), Effect::None);
        assert_eq!(app.update(Message::Leave), Effect::None);
        assert_eq!(app.update(Message::ServerNotSaved), Effect::None);
        assert!(matches!(app.screen(), Screen::Connect(_)));

        let mut app = connecting("ana");
        app.update(Message::AddressChanged("x".to_owned()));
        app.update(Message::NameChanged("x".to_owned()));
        assert_eq!(app.update(Message::Connect), Effect::None);
        assert_eq!(app.update(Message::Send), Effect::None);
        let Screen::Connecting { form, target } = app.screen() else {
            panic!("the application connects");
        };
        assert_eq!(form.address, "localhost:7678");
        assert_eq!(form.name, "ana");
        assert_eq!(target.attempt, 1);

        let (mut app, _outbox) = chatting("ana", 4);
        app.update(Message::NameChanged("x".to_owned()));
        assert_eq!(app.update(Message::Connect), Effect::None);
        assert!(matches!(app.screen(), Screen::Chat(_)));
        app.update(Message::Leave);
        let Screen::Connect(form) = app.screen() else {
            panic!("the application is on the connect screen");
        };
        assert_eq!(form.name, "ana");
    }

    #[test]
    fn stays_stopped_when_no_profile_is_open() {
        let mut app = App::stopped(Mode::Light, StartProblem::InUse);
        assert_eq!(app.servers(), []);

        typed(&mut app, "localhost", "ana");
        for message in [Message::Connect, Message::Leave, Message::Send] {
            assert_eq!(app.update(message), Effect::None);
        }
        let event = Event::Disconnected(Disconnect::Closed);
        assert_eq!(app.update(Message::Connection(0, event)), Effect::None);
        app.update(Message::ModeChanged(Mode::Dark));

        assert!(matches!(app.screen(), Screen::Stopped(StartProblem::InUse)));
        assert_eq!(app.target(), None);
        assert_eq!(app.mode(), Mode::Dark);
    }

    #[test]
    fn makes_a_start_problem_from_an_error_of_the_profile() {
        let file = std::io::Error::other("no access");
        for (error, problem) in [
            (ProfileError::InUse, StartProblem::InUse),
            (ProfileError::Damaged("seed"), StartProblem::Damaged("seed")),
            (
                ProfileError::File(file),
                StartProblem::Files("a file of the profile has a problem: no access".to_owned()),
            ),
            (
                ProfileError::NotFound,
                StartProblem::Files("the profile does not exist".to_owned()),
            ),
            (
                ProfileError::Name(NameProblem::Empty),
                StartProblem::Files("a profile must have a name".to_owned()),
            ),
        ] {
            assert_eq!(StartProblem::from(error), problem);
        }
    }

    #[test]
    fn describes_each_start_problem_in_the_language_of_the_user() {
        let english = Strings::new(Language::English);
        let spanish = Strings::new(Language::Spanish);
        let files = StartProblem::Files("no access".to_owned());

        assert_eq!(
            StartProblem::NoHome.describe(&english),
            "Post cannot find a directory for its data. Set the environment variable POST_HOME to a directory."
        );
        assert_eq!(
            StartProblem::InUse.describe(&english),
            "This profile is open in a different window of Post. Close that window, then start Post again."
        );
        assert_eq!(
            StartProblem::Damaged("seed").describe(&english),
            "The seed file of this profile is not valid. Post cannot open the profile."
        );
        assert_eq!(
            files.describe(&english),
            "Post cannot use the files of this profile. no access"
        );
        assert_eq!(
            StartProblem::InUse.describe(&spanish),
            "Este perfil está abierto en otra ventana de Post. Cierra esa ventana y vuelve a iniciar Post."
        );
        assert_eq!(
            StartProblem::Damaged("seed").describe(&spanish),
            "El archivo seed de este perfil no es válido. Post no puede abrir el perfil."
        );
        assert_eq!(
            files.describe(&spanish),
            "Post no puede usar los archivos de este perfil. no access"
        );
        assert_ne!(
            StartProblem::NoHome.describe(&spanish),
            StartProblem::NoHome.describe(&english)
        );
    }

    #[test]
    fn each_problem_has_a_text() {
        let problems = [
            (
                Problem::Address(DestinationError::Empty),
                Text::ProblemAddressEmpty,
            ),
            (
                Problem::Address(DestinationError::Invalid),
                Text::ProblemAddressInvalid,
            ),
            (
                Problem::Address(DestinationError::InvalidPort),
                Text::ProblemAddressPort,
            ),
            (
                Problem::Address(DestinationError::InvalidLink),
                Text::ProblemAddressLink,
            ),
            (
                Problem::Address(DestinationError::NotLocal),
                Text::ProblemAddressNotLocal,
            ),
            (
                Problem::Name(NameError::InvalidCharacter),
                Text::ProblemNameCharacters,
            ),
            (Problem::Name(NameError::TooShort), Text::ProblemNameShort),
            (Problem::Name(NameError::TooLong), Text::ProblemNameLong),
            (
                Problem::Disconnected(Disconnect::Unreachable),
                Text::ProblemUnreachable,
            ),
            (
                Problem::Disconnected(Disconnect::Closed),
                Text::ProblemClosed,
            ),
            (
                Problem::Disconnected(Disconnect::Protocol),
                Text::ProblemProtocol,
            ),
            (
                Problem::Disconnected(Disconnect::FalseServer),
                Text::ProblemFalseServer,
            ),
            (
                Problem::Disconnected(Disconnect::DifferentServer),
                Text::ProblemDifferentServer,
            ),
            (
                Problem::Disconnected(Disconnect::MissingParts),
                Text::ProblemMissingParts,
            ),
            (
                Problem::Disconnected(Disconnect::NameRequired),
                Text::ProblemNameRequired,
            ),
            (
                Problem::Disconnected(Disconnect::InvitationRequired),
                Text::ProblemInvitationRequired,
            ),
            (Problem::Disconnected(Disconnect::NoTls), Text::ProblemNoTls),
            (
                Problem::Disconnected(Disconnect::Refused(ErrorCode::NameTaken)),
                Text::ErrorNameTaken,
            ),
        ];
        for (problem, text) in problems {
            assert_eq!(problem.text(), text, "{problem:?}");
        }
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
