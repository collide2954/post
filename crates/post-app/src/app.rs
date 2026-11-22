pub mod chat;
pub mod console;
pub mod profiles;
pub mod servers;
pub mod settings;
pub mod start;

use post_client::{Event, KnownServer, Link, ProfileId};
use post_proto::ErrorCode;

use self::chat::Chat;
use self::console::Console;
use self::servers::{Problem, Servers, Target};
use self::start::StartProblem;
use crate::i18n::Text;
use crate::palette::Mode;
use crate::settings::Settings;

#[derive(Debug, Clone)]
pub enum Message {
    Servers(servers::Message),
    Chat(chat::Message),
    Settings(settings::Message),
    Profiles(profiles::Message),
    Console(console::Message),
    Connection(u64, Event),
    ProfileNotSaved,
    ModeChanged(Mode),
    FocusNext,
    FocusPrevious,
    Open(Page),
    Back,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Settings,
    Profiles,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    None,
    FocusForm,
    FocusDraft,
    FocusPanel,
    FocusNext,
    FocusPrevious,
    Remember(KnownServer),
    Forget(String),
    Copy(String),
    SaveSettings(Settings),
    ListProfiles,
    CreateProfile(String),
    RenameProfile(ProfileId, String),
    DeleteProfile(ProfileId),
    SwitchProfile(ProfileId),
}

#[derive(Debug)]
pub struct App {
    mode: Mode,
    servers: Vec<KnownServer>,
    attempts: u64,
    screen: Screen,
    page: Option<Page>,
    settings: settings::Store,
    profiles: profiles::Profiles,
    console: Console,
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
        Self::after(0, mode, servers)
    }

    #[must_use]
    pub fn for_a_different_profile(&self, servers: Vec<KnownServer>) -> Self {
        Self::after(self.attempts, self.mode, servers)
    }

    fn after(mut attempts: u64, mode: Mode, servers: Vec<KnownServer>) -> Self {
        let screen = match servers.first() {
            Some(last_in_use) => Servers::opening(last_in_use, &mut attempts),
            None => Servers::default(),
        };
        Self {
            mode,
            servers,
            attempts,
            screen: Screen::Servers(Box::new(screen)),
            page: None,
            settings: settings::Store::default(),
            profiles: profiles::Profiles::default(),
            console: Console::default(),
        }
    }

    #[must_use]
    pub fn stopped(mode: Mode, problem: StartProblem) -> Self {
        Self {
            mode,
            servers: Vec::new(),
            attempts: 0,
            screen: Screen::Stopped(problem),
            page: None,
            settings: settings::Store::default(),
            profiles: profiles::Profiles::default(),
            console: Console::default(),
        }
    }

    #[must_use]
    pub fn profiles(&self) -> &profiles::Profiles {
        &self.profiles
    }

    #[must_use]
    pub fn console(&self) -> Option<&Console> {
        self.settings().console.then_some(&self.console)
    }

    #[must_use]
    pub fn page(&self) -> Option<Page> {
        self.page
    }

    #[must_use]
    pub fn settings(&self) -> &Settings {
        self.settings.values()
    }

    #[must_use]
    pub fn settings_problem(&self) -> Option<settings::Problem> {
        self.settings.problem()
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
                    servers.update(message, &mut self.servers, &mut self.attempts)
                }
                Screen::Stopped(_) | Screen::Chat(_) => Effect::None,
            },
            Message::Chat(message) => self.chat(message),
            Message::Settings(message) => self.settings.update(message),
            Message::Profiles(message) => self.profiles.update(message),
            Message::Console(message) => {
                self.type_in_the_console(message);
                Effect::None
            }
            Message::Connection(attempt, event) => self.connection(attempt, event),
            Message::ProfileNotSaved => {
                match &mut self.screen {
                    Screen::Servers(servers) => servers.warn_that_the_profile_is_not_saved(),
                    Screen::Chat(chat) => chat.warn_that_the_server_is_not_saved(),
                    Screen::Stopped(_) => {}
                }
                Effect::None
            }
            Message::ModeChanged(mode) => {
                self.mode = mode;
                Effect::None
            }
            Message::FocusNext => Effect::FocusNext,
            Message::FocusPrevious => Effect::FocusPrevious,
            Message::Open(page) => {
                if matches!(self.screen, Screen::Stopped(_)) {
                    return Effect::None;
                }
                self.page = Some(page);
                match page {
                    Page::Settings => Effect::None,
                    Page::Profiles => Effect::ListProfiles,
                }
            }
            Message::Back => {
                if self.page == Some(Page::Profiles) && self.profiles.cancel() {
                    return Effect::None;
                }
                if self.page.take().is_some() {
                    return self.focus();
                }
                self.chat(chat::Message::Close)
            }
        }
    }

    fn type_in_the_console(&mut self, message: console::Message) {
        match message {
            console::Message::DraftChanged(draft) => self.console.edit(draft),
            console::Message::Clear => self.console.clear(),
            console::Message::Send => {
                let line = self.console.draft().to_owned();
                if line.trim().is_empty() {
                    return;
                }
                let sent = match &self.screen {
                    Screen::Chat(chat) => chat.send_line(line),
                    Screen::Servers(_) | Screen::Stopped(_) => false,
                };
                self.console.finish(sent);
            }
        }
    }

    fn focus(&self) -> Effect {
        match &self.screen {
            Screen::Servers(servers)
                if servers.connecting().is_none() && servers.question().is_none() =>
            {
                Effect::FocusForm
            }
            Screen::Chat(chat) => match chat.panel() {
                None => Effect::FocusDraft,
                Some(chat::Open::Account(_) | chat::Open::Channels(_)) => Effect::FocusPanel,
                Some(chat::Open::Invitations(_) | chat::Open::Menu) => Effect::None,
            },
            Screen::Servers(_) | Screen::Stopped(_) => Effect::None,
        }
    }

    fn chat(&mut self, message: chat::Message) -> Effect {
        let Screen::Chat(chat) = &mut self.screen else {
            return Effect::None;
        };
        match message {
            chat::Message::Leave => self.leave(None),
            message => chat.update(message),
        }
    }

    fn connection(&mut self, attempt: u64, event: Event) -> Effect {
        if self.target().is_none_or(|target| target.attempt != attempt) {
            return Effect::None;
        }
        match event {
            Event::Line(line) => {
                if self.settings().console {
                    self.console.record(line);
                }
                Effect::None
            }
            Event::Connected(link) => self.open_chat(link),
            Event::Received(message) => {
                let Screen::Chat(chat) = &mut self.screen else {
                    return Effect::None;
                };
                let effect = chat.receive(message);
                if let Effect::Remember(server) = &effect {
                    server.clone().remember_in(&mut self.servers);
                }
                effect
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
        let chat = Chat::new(form, target, link);
        let server = chat.server();
        self.screen = Screen::Chat(Box::new(chat));
        server.clone().remember_in(&mut self.servers);
        Effect::Remember(server)
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
        Effect::FocusForm
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
        ErrorCode::InvalidChannelName => Text::ErrorInvalidChannelName,
        ErrorCode::TooManyChannels => Text::ErrorTooManyChannels,
        ErrorCode::AlreadyMember => Text::ErrorAlreadyMember,
        ErrorCode::NotMember => Text::ErrorNotMember,
        ErrorCode::OwnerCannotLeave => Text::ErrorOwnerCannotLeave,
        ErrorCode::InvalidTopic => Text::ErrorInvalidTopic,
        ErrorCode::NoSuchUser => Text::ErrorNoSuchUser,
        ErrorCode::NoSuchMember => Text::ErrorNoSuchMember,
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
        Direction, Disconnect, Entry, Event, Fingerprint, KnownServer, Line, Link, Outgoing,
        ProfileEntry, ProfileId,
    };
    use post_proto::{
        ChannelKind, ChannelName, ClientMessage, ErrorCode, Key, Name, Role, ServerMessage,
        Timestamp,
    };

    use super::servers::Problem;
    use super::start::StartProblem;
    use super::{
        App, Effect, Message, Page, Screen, chat, console, error_text, profiles, servers, settings,
    };
    use crate::i18n::Text;
    use crate::palette::Mode;
    use crate::settings::{Clock, Settings};

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

    fn key(number: u8) -> Key {
        Key::from_bytes([number; 32])
    }

    fn server(address: &str, number: u8, who: &str) -> KnownServer {
        KnownServer {
            address: address.to_owned(),
            key: key(number),
            name: name(who),
            tls: false,
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

    fn cancel(app: &mut App) -> Effect {
        app.update(Message::Servers(servers::Message::Cancel))
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
            Effect::Remember(server("localhost:7678", 1, who))
        );
        assert!(matches!(app.screen(), Screen::Chat(_)));
        let state = ServerMessage::Channel {
            reference: None,
            channel: general(),
            kind: ChannelKind::Public,
            topic: String::new(),
            owner: name(who),
            created: time(),
        };
        let received = [
            (state, Effect::None),
            (ServerMessage::Ready {}, Effect::FocusDraft),
        ];
        for (message, effect) in received {
            let message = connection(&app, Event::Received(message));
            assert_eq!(app.update(message), effect);
        }
        (
            app,
            Box::new(move || messages.try_recv().ok().map(sent_message)),
        )
    }

    fn form_problem(app: &App) -> Option<Problem> {
        match app.screen() {
            Screen::Servers(servers) => servers.form().problem,
            _ => panic!("the application is not on the servers screen"),
        }
    }

    fn entries(app: &App) -> Vec<Entry> {
        match app.screen() {
            Screen::Chat(chat) => chat.entries().to_vec(),
            _ => panic!("the application is not on the chat screen"),
        }
    }

    fn notice(app: &App) -> Option<Text> {
        match app.screen() {
            Screen::Chat(chat) => chat.notice(),
            _ => panic!("the application is not on the chat screen"),
        }
    }

    fn time() -> Timestamp {
        Timestamp::parse("2026-11-21T14:02:00.000Z").expect("valid")
    }

    fn general() -> ChannelName {
        ChannelName::parse("general").expect("valid")
    }

    fn joined(who: &str) -> ServerMessage {
        ServerMessage::Join {
            channel: general(),
            name: name(who),
            time: time(),
        }
    }

    #[test]
    fn starts_on_the_servers_screen_with_an_empty_form() {
        let app = App::new(Mode::Dark, Vec::new());

        let Screen::Servers(servers) = app.screen() else {
            panic!("the application starts on the servers screen");
        };
        assert_eq!(servers.form().address, "");
        assert_eq!(app.mode(), Mode::Dark);
        assert_eq!(app.target(), None);
        assert_eq!(app.servers(), []);
    }

    #[test]
    fn connects_at_the_start_to_the_server_that_was_last_in_use() {
        let known = vec![
            server("chat.example.org:443", 2, "ana"),
            server("localhost:7678", 1, "ana"),
        ];

        let app = App::new(Mode::Light, known.clone());

        let target = app.target().expect("a connection is in progress");
        assert_eq!(target.attempt, 1);
        assert_eq!(target.destination.to_string(), "chat.example.org");
        assert_eq!(target.trust.key, Some(key(2)));
        assert_eq!(app.servers(), known);
    }

    #[test]
    fn gives_each_connection_a_new_number() {
        let mut app = App::new(Mode::Light, vec![server("localhost:7678", 1, "ana")]);
        assert_eq!(app.target().map(|target| target.attempt), Some(1));
        assert_eq!(cancel(&mut app), Effect::FocusForm);
        assert_eq!(app.target(), None);

        app.update(Message::Servers(servers::Message::Open(0)));

        assert_eq!(app.target().expect("a second connection").attempt, 2);
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
            Effect::Remember(server("localhost:7678", 1, "ana-2"))
        );
        let Screen::Chat(chat) = app.screen() else {
            panic!("the application shows the chat");
        };
        assert_eq!(chat.session().name(), &name("ana-2"));
        assert_eq!(app.target().cloned(), before);
    }

    #[test]
    fn remembers_the_certificate_that_the_connection_accepted() {
        let mut app = app();
        typed(&mut app, "https://localhost:8443", "ana");
        connect(&mut app);
        let (link, _messages) = Link::channel(name("ana"), Role::Member, key(1), 4);
        let certificate = Fingerprint::from_bytes([9; 32]);
        let link = link.with_certificate(Some(certificate));

        let message = connection(&app, Event::Connected(link));
        let effect = app.update(message);

        let expected = KnownServer {
            tls: true,
            certificate: Some("09".repeat(32)),
            ..server("localhost:8443", 1, "ana")
        };
        assert_eq!(effect, Effect::Remember(expected.clone()));
        assert_eq!(app.servers(), [expected]);
    }

    #[test]
    fn puts_the_server_of_the_connection_first_in_the_list() {
        let known = vec![
            server("chat.example.org:443", 2, "ana"),
            server("localhost:7678", 3, "ana-old"),
        ];
        let mut app = App::new(Mode::Light, known);
        cancel(&mut app);
        app.update(Message::Servers(servers::Message::Open(1)));
        assert_eq!(
            app.target().map(|target| target.trust.key),
            Some(Some(key(3)))
        );
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
    }

    #[test]
    fn removes_a_server_when_the_user_asks() {
        let known = vec![
            server("chat.example.org:443", 2, "ana"),
            server("localhost:7678", 1, "ana"),
        ];
        let mut app = App::new(Mode::Light, known);
        cancel(&mut app);

        let effect = app.update(Message::Servers(servers::Message::Forget(0)));

        assert_eq!(effect, Effect::Forget("chat.example.org:443".to_owned()));
        assert_eq!(app.servers(), [server("localhost:7678", 1, "ana")]);
    }

    #[test]
    fn records_what_the_server_sends() {
        let (mut app, _outbox) = chatting("ana", 4);

        let message = connection(&app, Event::Received(joined("ben")));
        assert_eq!(app.update(message), Effect::None);

        assert_eq!(
            entries(&app),
            [Entry::Joined {
                time: time(),
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
            assert_eq!(app.update(message), Effect::FocusForm);

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
        assert_eq!(app.update(message), Effect::FocusForm);
        assert_eq!(
            form_problem(&app),
            Some(Problem::Disconnected(Disconnect::Closed))
        );
    }

    #[test]
    fn asks_about_a_private_certificate_before_it_connects() {
        let mut app = app();
        typed(&mut app, "https://localhost:8443", "ana");
        connect(&mut app);
        let certificate = Fingerprint::from_bytes([9; 32]);

        let refused = Event::Disconnected(Disconnect::UnknownCertificate(certificate));
        let message = connection(&app, refused);
        assert_eq!(app.update(message), Effect::None);

        let Screen::Servers(servers) = app.screen() else {
            panic!("the application is on the servers screen");
        };
        assert_eq!(
            servers.question().map(|question| question.fingerprint),
            Some(certificate)
        );
        assert_eq!(app.target(), None);

        app.update(Message::Servers(servers::Message::AcceptCertificate));
        let target = app.target().expect("a second connection");
        assert_eq!(target.attempt, 2);
        assert_eq!(target.trust.certificate, Some(certificate));
    }

    #[test]
    fn leaves_the_chat_when_the_user_asks() {
        let (mut app, _outbox) = chatting("ana", 4);

        assert_eq!(leave(&mut app), Effect::FocusForm);

        assert_eq!(form_problem(&app), None);
        assert_eq!(app.target(), None);
        let Screen::Servers(servers) = app.screen() else {
            panic!("the application is on the servers screen");
        };
        assert_eq!(servers.form().name, "ana");
        assert_eq!(app.servers().len(), 1);
    }

    #[test]
    fn ignores_an_event_of_an_earlier_connection() {
        let mut app = connecting("ana");
        let old = app.target().expect("a connection is in progress").attempt;
        cancel(&mut app);
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
                channel: "general".to_owned(),
                text: "Hello".to_owned(),
            })
        );
        let open = chat::Message::Open(chat::Panel::Account);
        assert_eq!(app.update(Message::Chat(open)), Effect::FocusPanel);
        let close = Message::Chat(chat::Message::Close);
        assert_eq!(app.update(close), Effect::FocusDraft);
    }

    #[test]
    fn keeps_the_new_name_of_the_user_in_the_list_of_servers() {
        let (mut app, _outbox) = chatting("ana", 4);
        let renamed = ServerMessage::Rename {
            reference: None,
            old: name("ana"),
            new: name("anna"),
            time: time(),
        };

        let message = connection(&app, Event::Received(renamed));
        let effect = app.update(message);

        let expected = server("localhost:7678", 1, "anna");
        assert_eq!(effect, Effect::Remember(expected.clone()));
        assert_eq!(app.servers(), [expected]);
    }

    #[test]
    fn tells_the_user_that_the_profile_did_not_save_a_change() {
        let (mut app, _outbox) = chatting("ana", 4);
        assert_eq!(app.update(Message::ProfileNotSaved), Effect::None);
        assert_eq!(notice(&app), Some(Text::ChatServerNotSaved));

        let mut app = App::new(Mode::Light, Vec::new());
        assert_eq!(app.update(Message::ProfileNotSaved), Effect::None);
        assert_eq!(form_problem(&app), Some(Problem::NotSaved));
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
        assert!(matches!(app.screen(), Screen::Servers(_)));

        let (mut app, _outbox) = chatting("ana", 4);
        typed(&mut app, "x", "x");
        assert_eq!(connect(&mut app), Effect::None);
        assert_eq!(
            app.update(Message::Servers(servers::Message::Forget(0))),
            Effect::None
        );
        assert!(matches!(app.screen(), Screen::Chat(_)));
        assert_eq!(app.servers().len(), 1);
        leave(&mut app);
        let Screen::Servers(servers) = app.screen() else {
            panic!("the application is on the servers screen");
        };
        assert_eq!(servers.form().address, "localhost:7678");
        assert_eq!(servers.form().name, "ana");
    }

    #[test]
    fn starts_with_the_default_settings_and_no_page() {
        let app = app();

        assert_eq!(app.settings(), &Settings::default());
        assert_eq!(app.settings_problem(), None);
        assert_eq!(app.page(), None);
    }

    #[test]
    fn changes_a_setting_on_each_screen() {
        let larger = || Message::Settings(settings::Message::Larger);
        let expected = Settings {
            text_size: 110,
            ..Settings::default()
        };

        let mut app = app();
        assert_eq!(app.update(larger()), Effect::SaveSettings(expected));
        assert_eq!(app.settings(), &expected);

        let (mut app, _outbox) = chatting("ana", 4);
        assert_eq!(app.update(larger()), Effect::SaveSettings(expected));

        let mut app = App::stopped(Mode::Light, StartProblem::InUse);
        assert_eq!(app.update(larger()), Effect::SaveSettings(expected));
        app.update(Message::Settings(settings::Message::NotSaved));
        assert_eq!(app.settings_problem(), Some(settings::Problem::NotSaved));
    }

    #[test]
    fn takes_the_settings_of_the_file() {
        let mut app = app();

        let text = "clock = 12\n".to_owned();
        let effect = app.update(Message::Settings(settings::Message::FileChanged(text)));

        assert_eq!(effect, Effect::None);
        assert_eq!(app.settings().clock, Clock::Hours12);
    }

    #[test]
    fn opens_the_settings_page_and_goes_back_to_the_screen() {
        let mut app = app();
        assert_eq!(app.update(Message::Open(Page::Settings)), Effect::None);
        assert_eq!(app.page(), Some(Page::Settings));
        assert_eq!(app.update(Message::Back), Effect::FocusForm);
        assert_eq!(app.page(), None);
        assert_eq!(app.update(Message::Back), Effect::None);

        let mut app = connecting("ana");
        app.update(Message::Open(Page::Settings));
        assert_eq!(app.update(Message::Back), Effect::None);
        assert!(app.target().is_some());

        let (mut app, _outbox) = chatting("ana", 4);
        app.update(Message::Open(Page::Settings));
        assert!(app.target().is_some());
        assert_eq!(app.update(Message::Back), Effect::FocusDraft);

        let open = |panel| Message::Chat(chat::Message::Open(panel));
        app.update(open(chat::Panel::Account));
        app.update(Message::Open(Page::Settings));
        assert_eq!(app.update(Message::Back), Effect::FocusPanel);
        assert_eq!(app.update(Message::Back), Effect::FocusDraft);

        let (mut app, _outbox) = chatting("ana", 4);
        app.update(open(chat::Panel::Invitations));
        app.update(Message::Open(Page::Settings));
        assert_eq!(app.update(Message::Back), Effect::None);
        assert_eq!(app.page(), None);
        assert_eq!(app.update(Message::Back), Effect::FocusDraft);
    }

    fn line(direction: Direction, text: &str) -> Line {
        Line {
            time: Timestamp::parse("2026-11-21T14:02:07.120Z").expect("valid"),
            direction,
            text: text.to_owned(),
        }
    }

    fn with_console(app: &mut App) {
        let on = settings::Message::ConsoleToggled(true);
        assert!(matches!(
            app.update(Message::Settings(on)),
            Effect::SaveSettings(settings) if settings.console
        ));
    }

    fn console_texts(app: &App) -> Vec<String> {
        let console = app.console().expect("the console shows");
        console
            .lines()
            .iter()
            .map(|line| line.text.clone())
            .collect()
    }

    #[test]
    fn shows_the_console_only_when_the_setting_is_on() {
        let mut app = app();
        assert!(app.console().is_none());

        with_console(&mut app);
        assert!(
            app.console()
                .is_some_and(|console| console.lines().is_empty())
        );

        let flip = || Message::Settings(settings::Message::ConsoleFlipped);
        app.update(flip());
        assert!(app.console().is_none());
        app.update(flip());
        assert!(app.console().is_some());
    }

    #[test]
    fn records_the_lines_of_the_connection_in_the_console() {
        let mut app = connecting("ana");
        let hello = line(Direction::Sent, r#"{"type":"hello"}"#);
        let message = connection(&app, Event::Line(hello.clone()));
        assert_eq!(app.update(message), Effect::None);
        with_console(&mut app);
        assert_eq!(console_texts(&app), Vec::<String>::new());

        let challenge = line(Direction::Received, r#"{"type":"challenge"}"#);
        for recorded in [hello, challenge.clone()] {
            let message = connection(&app, Event::Line(recorded));
            assert_eq!(app.update(message), Effect::None);
        }

        assert_eq!(
            console_texts(&app),
            [r#"{"type":"hello"}"#, r#"{"type":"challenge"}"#]
        );
        let stale = Message::Connection(99, Event::Line(challenge));
        app.update(stale);
        assert_eq!(console_texts(&app).len(), 2);

        app.update(Message::Console(console::Message::Clear));
        assert_eq!(console_texts(&app), Vec::<String>::new());
    }

    #[test]
    fn sends_a_line_that_the_user_types_in_the_console() {
        let mut app = connecting("ana");
        let (link, mut queue) = Link::channel(name("ana"), Role::Member, key(1), 1);
        let message = connection(&app, Event::Connected(link));
        app.update(message);
        with_console(&mut app);
        let typed = |text: &str| Message::Console(console::Message::DraftChanged(text.to_owned()));
        let send = || Message::Console(console::Message::Send);

        app.update(typed("  "));
        assert_eq!(app.update(send()), Effect::None);
        assert!(queue.try_recv().is_err());

        app.update(typed(r#"{"type":"invitations"}"#));
        assert_eq!(app.update(send()), Effect::None);

        assert_eq!(
            queue.try_recv(),
            Ok(Outgoing::Line(r#"{"type":"invitations"}"#.to_owned()))
        );
        let console = app.console().expect("the console shows");
        assert_eq!(console.draft(), "");
        assert!(!console.not_sent());

        app.update(typed("one"));
        app.update(send());
        app.update(typed("two"));
        app.update(send());
        let console = app.console().expect("the console shows");
        assert_eq!(console.draft(), "two");
        assert!(console.not_sent());
    }

    #[test]
    fn does_not_send_a_line_without_a_connection() {
        let mut app = app();
        with_console(&mut app);
        let draft = console::Message::DraftChanged("{}".to_owned());
        app.update(Message::Console(draft));

        app.update(Message::Console(console::Message::Send));

        let console = app.console().expect("the console shows");
        assert_eq!(console.draft(), "{}");
        assert!(console.not_sent());
    }

    #[test]
    fn asks_for_the_list_when_the_profiles_page_opens() {
        let mut app = app();

        assert_eq!(
            app.update(Message::Open(Page::Profiles)),
            Effect::ListProfiles
        );
        assert_eq!(app.page(), Some(Page::Profiles));

        let list = vec![
            ProfileEntry {
                id: ProfileId::from_number(1),
                name: "main".to_owned(),
            },
            ProfileEntry {
                id: ProfileId::from_number(2),
                name: "work".to_owned(),
            },
        ];
        let listed = profiles::Message::Listed(list.clone(), ProfileId::from_number(1));
        assert_eq!(app.update(Message::Profiles(listed)), Effect::None);
        assert_eq!(app.profiles().list(), list);

        let rename = profiles::Message::Rename(ProfileId::from_number(2));
        assert_eq!(app.update(Message::Profiles(rename)), Effect::FocusPanel);
        assert_eq!(app.update(Message::Back), Effect::None);
        assert_eq!(app.profiles().edit(), None);
        assert_eq!(app.page(), Some(Page::Profiles));
        assert_eq!(app.update(Message::Back), Effect::FocusForm);
        assert_eq!(app.page(), None);
    }

    #[test]
    fn gives_a_connection_of_the_next_profile_a_new_number() {
        let (app, _outbox) = chatting("ana", 4);
        let old = app.target().expect("a connection").attempt;

        let mut next = app.for_a_different_profile(vec![server("localhost:7678", 2, "zoe")]);

        let target = next.target().expect("the application connects at once");
        assert_eq!(target.attempt, old + 1);
        assert_eq!(next.mode(), app.mode());
        assert_eq!(next.settings(), &Settings::default());
        assert_eq!(next.page(), None);
        let stale = Message::Connection(old, Event::Received(joined("ben")));
        assert_eq!(next.update(stale), Effect::None);
        let stale = Message::Connection(old, Event::Disconnected(Disconnect::Closed));
        assert_eq!(next.update(stale), Effect::None);
        assert!(next.target().is_some());

        let empty = app.for_a_different_profile(Vec::new());
        assert_eq!(empty.target(), None);
        assert_eq!(empty.servers(), []);
    }

    #[test]
    fn keeps_the_connection_while_a_page_is_open() {
        let (mut app, _outbox) = chatting("ana", 4);
        app.update(Message::Open(Page::Settings));

        let message = connection(&app, Event::Received(joined("ben")));
        app.update(message);

        assert_eq!(app.page(), Some(Page::Settings));
        assert_eq!(entries(&app).len(), 1);
    }

    #[test]
    fn stays_stopped_when_no_profile_is_open() {
        let mut app = App::stopped(Mode::Light, StartProblem::InUse);
        assert_eq!(app.servers(), []);

        typed(&mut app, "localhost", "ana");
        assert_eq!(connect(&mut app), Effect::None);
        assert_eq!(leave(&mut app), Effect::None);
        assert_eq!(app.update(Message::Chat(chat::Message::Send)), Effect::None);
        assert_eq!(app.update(Message::ProfileNotSaved), Effect::None);
        let event = Event::Disconnected(Disconnect::Closed);
        assert_eq!(app.update(Message::Connection(0, event)), Effect::None);
        app.update(Message::ModeChanged(Mode::Dark));
        assert_eq!(app.update(Message::Open(Page::Settings)), Effect::None);
        assert_eq!(app.update(Message::Open(Page::Profiles)), Effect::None);
        assert_eq!(app.page(), None);
        assert_eq!(app.update(Message::Back), Effect::None);

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
            (ErrorCode::InvalidChannelName, Text::ErrorInvalidChannelName),
            (ErrorCode::TooManyChannels, Text::ErrorTooManyChannels),
            (ErrorCode::AlreadyMember, Text::ErrorAlreadyMember),
            (ErrorCode::NotMember, Text::ErrorNotMember),
            (ErrorCode::OwnerCannotLeave, Text::ErrorOwnerCannotLeave),
            (ErrorCode::InvalidTopic, Text::ErrorInvalidTopic),
            (ErrorCode::NoSuchUser, Text::ErrorNoSuchUser),
            (ErrorCode::NoSuchMember, Text::ErrorNoSuchMember),
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
