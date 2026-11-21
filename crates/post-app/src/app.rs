use post_client::{AddressError, Disconnect, Event, Link, ServerAddress, Session};
use post_proto::{DEFAULT_PORT, ErrorCode, Name, NameError, TextError};

use crate::i18n::Text;
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
    ModeChanged(Mode),
    FocusNext,
    FocusPrevious,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    None,
    FocusName,
    FocusDraft,
    FocusNext,
    FocusPrevious,
}

#[derive(Debug)]
pub struct App {
    mode: Mode,
    attempts: u64,
    screen: Screen,
}

#[derive(Debug)]
pub enum Screen {
    Connect(Form),
    Connecting { form: Form, target: Target },
    Chat(Box<Chat>),
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
    pub address: ServerAddress,
    pub name: Name,
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
    Address(AddressError),
    Name(NameError),
    Disconnected(Disconnect),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Notice {
    TooLong,
    NotSent,
}

impl App {
    #[must_use]
    pub fn new(mode: Mode) -> Self {
        Self {
            mode,
            attempts: 0,
            screen: Screen::Connect(Form {
                address: format!("localhost:{DEFAULT_PORT}"),
                name: String::new(),
                problem: None,
            }),
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
    pub fn target(&self) -> Option<&Target> {
        match &self.screen {
            Screen::Connect(_) => None,
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
        let address = match ServerAddress::parse(&form.address) {
            Ok(address) => address,
            Err(error) => {
                form.problem = Some(Problem::Address(error));
                return Effect::None;
            }
        };
        let name = match Name::parse(form.name.trim()) {
            Ok(name) => name,
            Err(error) => {
                form.problem = Some(Problem::Name(error));
                return Effect::None;
            }
        };
        self.attempts += 1;
        let target = Target {
            attempt: self.attempts,
            address,
            name,
        };
        self.screen = Screen::Connecting {
            form: std::mem::take(form),
            target,
        };
        Effect::None
    }

    fn leave(&mut self, problem: Option<Problem>) -> Effect {
        let form = match &mut self.screen {
            Screen::Connect(_) => return Effect::None,
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
        self.screen = Screen::Chat(Box::new(Chat {
            form: std::mem::take(form),
            target: target.clone(),
            session: Session::new(link.name().clone()),
            link,
            draft: String::new(),
            notice: None,
        }));
        Effect::FocusDraft
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

impl Problem {
    #[must_use]
    pub fn text(self) -> Text {
        match self {
            Self::Address(AddressError::Empty) => Text::ProblemAddressEmpty,
            Self::Address(AddressError::Invalid) => Text::ProblemAddressInvalid,
            Self::Address(AddressError::InvalidPort) => Text::ProblemAddressPort,
            Self::Address(AddressError::NotLocal) => Text::ProblemAddressNotLocal,
            Self::Name(NameError::InvalidCharacter) => Text::ProblemNameCharacters,
            Self::Name(NameError::TooShort) => Text::ProblemNameShort,
            Self::Name(NameError::TooLong) => Text::ProblemNameLong,
            Self::Disconnected(Disconnect::Unreachable) => Text::ProblemUnreachable,
            Self::Disconnected(Disconnect::Closed) => Text::ProblemClosed,
            Self::Disconnected(Disconnect::Protocol) => Text::ProblemProtocol,
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
        ErrorCode::BadMessage | ErrorCode::HelloRequired | ErrorCode::DuplicateHello => {
            Text::ErrorOther
        }
    }
}

#[cfg(test)]
mod tests {
    use post_client::{AddressError, Disconnect, Entry, Event, Link, ServerAddress};
    use post_proto::{ClientMessage, ErrorCode, Name, NameError, ServerMessage, Timestamp};

    use super::{App, Effect, Message, Problem, Screen, error_text};
    use crate::i18n::Text;
    use crate::palette::Mode;

    fn name(text: &str) -> Name {
        Name::parse(text).expect(text)
    }

    fn typed(app: &mut App, address: &str, name: &str) {
        app.update(Message::AddressChanged(address.to_owned()));
        app.update(Message::NameChanged(name.to_owned()));
    }

    fn connecting(who: &str) -> App {
        let mut app = App::new(Mode::Light);
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
        let message = connection(&app, Event::Connected(link));
        assert_eq!(app.update(message), Effect::FocusDraft);
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
        let app = App::new(Mode::Dark);

        let Screen::Connect(form) = app.screen() else {
            panic!("the application starts on the connect screen");
        };
        assert_eq!(form.address, "localhost:7678");
        assert_eq!(form.name, "");
        assert_eq!(form.problem, None);
        assert_eq!(app.mode(), Mode::Dark);
        assert_eq!(app.target(), None);
    }

    #[test]
    fn keeps_what_the_user_types_on_the_connect_screen() {
        let mut app = App::new(Mode::Light);

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
            ("", AddressError::Empty),
            ("local host", AddressError::Invalid),
            ("localhost:0", AddressError::InvalidPort),
            ("chat.example.org", AddressError::NotLocal),
        ] {
            let mut app = App::new(Mode::Light);
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
            ("", NameError::TooShort),
            ("a", NameError::TooShort),
            ("Ana", NameError::InvalidCharacter),
            ("abcdefghij0123456789x", NameError::TooLong),
        ] {
            let mut app = App::new(Mode::Light);
            typed(&mut app, "localhost", who);

            app.update(Message::Connect);

            assert_eq!(form_problem(&app), Some(Problem::Name(error)), "{who}");
            assert_eq!(app.target(), None);
        }
    }

    #[test]
    fn removes_the_problem_when_the_user_types() {
        let mut app = App::new(Mode::Light);
        typed(&mut app, "", "ana");
        app.update(Message::Connect);
        app.update(Message::AddressChanged("l".to_owned()));
        assert_eq!(form_problem(&app), None);

        typed(&mut app, "localhost", "");
        app.update(Message::Connect);
        app.update(Message::NameChanged("a".to_owned()));
        assert_eq!(form_problem(&app), None);
    }

    #[test]
    fn starts_a_connection_with_a_valid_address_and_name() {
        let mut app = App::new(Mode::Light);
        typed(&mut app, " LOCALHOST:9000 ", "  ana  ");

        app.update(Message::Connect);

        let target = app.target().expect("a connection is in progress");
        assert_eq!(target.attempt, 1);
        assert_eq!(
            target.address,
            ServerAddress::parse("localhost:9000").expect("valid")
        );
        assert_eq!(target.name, name("ana"));
        let Screen::Connecting { form, .. } = app.screen() else {
            panic!("the application connects");
        };
        assert_eq!(form.address, " LOCALHOST:9000 ");
        assert_eq!(form.problem, None);
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

        let message = connection(&app, Event::Connected(link));
        assert_eq!(app.update(message), Effect::FocusDraft);

        let Screen::Chat(chat) = app.screen() else {
            panic!("the application shows the chat");
        };
        assert_eq!(chat.session().name(), &name("ana-2"));
        assert_eq!(chat.draft(), "");
        assert_eq!(chat.notice(), None);
        assert_eq!(app.target().cloned(), before);
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
        let mut app = App::new(Mode::Light);

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
    fn changes_between_light_and_dark_mode() {
        let mut app = App::new(Mode::Light);

        assert_eq!(app.update(Message::ModeChanged(Mode::Dark)), Effect::None);

        assert_eq!(app.mode(), Mode::Dark);
    }

    #[test]
    fn moves_the_focus_when_the_user_asks() {
        let mut app = App::new(Mode::Light);

        assert_eq!(app.update(Message::FocusNext), Effect::FocusNext);
        assert_eq!(app.update(Message::FocusPrevious), Effect::FocusPrevious);
    }

    #[test]
    fn ignores_a_message_that_is_for_a_different_screen() {
        let mut app = App::new(Mode::Light);
        assert_eq!(
            app.update(Message::DraftChanged("x".to_owned())),
            Effect::None
        );
        assert_eq!(app.update(Message::Send), Effect::None);
        assert_eq!(app.update(Message::Leave), Effect::None);
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
    fn each_problem_has_a_text() {
        let problems = [
            (
                Problem::Address(AddressError::Empty),
                Text::ProblemAddressEmpty,
            ),
            (
                Problem::Address(AddressError::Invalid),
                Text::ProblemAddressInvalid,
            ),
            (
                Problem::Address(AddressError::InvalidPort),
                Text::ProblemAddressPort,
            ),
            (
                Problem::Address(AddressError::NotLocal),
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
            (ErrorCode::BadMessage, Text::ErrorOther),
            (ErrorCode::HelloRequired, Text::ErrorOther),
            (ErrorCode::DuplicateHello, Text::ErrorOther),
        ];
        for (code, text) in codes {
            assert_eq!(error_text(code), text, "{code:?}");
        }
    }
}
