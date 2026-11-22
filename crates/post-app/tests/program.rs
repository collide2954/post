use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use iced::{Program, Size, Theme};
use iced_test::emulator::{self, Emulator, Mode};
use iced_test::futures::futures::channel::mpsc;
use iced_test::selector::{Candidate, Selector};
use iced_test::{Instruction, Simulator};
use post_app::app::{Message, console, profiles, servers};
use post_app::arguments::Arguments;
use post_app::i18n::Language;
use post_app::shell;
use post_client::{
    Destination, Event, Fingerprint, Home, Identity, Profiles, Seed, Trust, sign_in,
};
use post_proto::{ChannelKind, ClientMessage, Name, ServerMessage};
use post_server::Server;
use post_server::config::{Config, Tls};
use post_testing::Directory;
use tokio::runtime::Runtime;
use tokio::sync::oneshot;

const PATIENCE: Duration = Duration::from_secs(30);
const QUIET: Duration = Duration::from_millis(40);

struct Driver<P: Program + 'static> {
    program: P,
    emulator: Emulator<P>,
    events: mpsc::Receiver<emulator::Event<P>>,
}

impl<P> Driver<P>
where
    P: Program<Message = Message, Theme = Theme> + 'static,
{
    fn start(program: P) -> Self {
        let (sender, events) = mpsc::channel(1);
        let emulator = Emulator::new(sender, &program, Mode::Zen, Size::new(900.0, 600.0));
        let mut driver = Self {
            program,
            emulator,
            events,
        };
        driver.wait_until_ready();
        driver.pump();
        driver
    }

    fn wait_until_ready(&mut self) {
        let start = Instant::now();
        loop {
            match self.events.try_recv() {
                Ok(emulator::Event::Action(action)) => {
                    self.emulator.perform(&self.program, action);
                }
                Ok(emulator::Event::Ready) => return,
                Ok(emulator::Event::Failed(instruction)) => {
                    panic!("the instruction failed: {instruction}")
                }
                Err(error) if error.is_closed() => panic!("the emulator stopped"),
                Err(_) => {
                    assert!(start.elapsed() < PATIENCE, "the emulator is not ready");
                    thread::sleep(Duration::from_millis(2));
                }
            }
        }
    }

    fn pump(&mut self) {
        let mut last_event = Instant::now();
        while last_event.elapsed() < QUIET {
            match self.events.try_recv() {
                Ok(emulator::Event::Action(action)) => {
                    self.emulator.perform(&self.program, action);
                    last_event = Instant::now();
                }
                Ok(emulator::Event::Ready) => last_event = Instant::now(),
                Ok(emulator::Event::Failed(instruction)) => {
                    panic!("the instruction failed: {instruction}")
                }
                Err(error) if error.is_closed() => panic!("the emulator stopped"),
                Err(_) => thread::sleep(Duration::from_millis(2)),
            }
        }
    }

    fn act(&mut self, line: &str) {
        let instruction = Instruction::parse(line).unwrap_or_else(|_| panic!("not valid: {line}"));
        self.emulator.run(&self.program, instruction);
        self.wait_until_ready();
        self.pump();
    }

    fn type_text(&mut self, text: &str) {
        self.type_after("", text);
    }

    fn type_after(&mut self, before: &str, text: &str) {
        let mut characters = text.chars();
        let Some(first) = characters.next() else {
            return;
        };
        let shown = format!("{before}{first}");
        let start = Instant::now();
        loop {
            self.act(&format!("type \"{first}\""));
            if self.shows(&shown) {
                break;
            }
            assert!(
                start.elapsed() < PATIENCE,
                "no field takes the text: {text}"
            );
        }
        for character in characters {
            self.act(&format!("type \"{character}\""));
        }
    }

    fn send(&mut self, message: Message) {
        self.emulator.update(&self.program, message);
        self.pump();
    }

    fn sign_up(&mut self, to: String, name: &str) {
        self.send(address(to));
        self.send(Message::Servers(servers::Message::NameChanged(
            name.to_owned(),
        )));
        self.send(Message::Servers(servers::Message::Connect));
    }

    fn shows(&self, text: &str) -> bool {
        Simulator::new(self.emulator.view(&self.program))
            .find(text)
            .is_ok()
    }

    fn expect(&mut self, text: &str) {
        let start = Instant::now();
        while !self.shows(text) {
            assert!(start.elapsed() < PATIENCE, "the text does not show: {text}");
            self.pump();
        }
    }

    fn expect_no(&mut self, text: &str) {
        let start = Instant::now();
        while self.shows(text) {
            assert!(start.elapsed() < PATIENCE, "the text stays: {text}");
            self.pump();
        }
    }

    fn expect_in_file(&mut self, file: &Path, part: &str) {
        let start = Instant::now();
        while !std::fs::read_to_string(file).is_ok_and(|text| text.contains(part)) {
            assert!(
                start.elapsed() < PATIENCE,
                "{} does not contain: {part}",
                file.display()
            );
            self.pump();
        }
    }

    fn text_with(&self, part: &'static str) -> Option<String> {
        Simulator::new(self.emulator.view(&self.program))
            .find(Containing(part))
            .ok()
    }

    fn expect_text_with(&mut self, part: &'static str) -> String {
        let start = Instant::now();
        loop {
            if let Some(text) = self.text_with(part) {
                return text;
            }
            assert!(start.elapsed() < PATIENCE, "no text contains: {part}");
            self.pump();
        }
    }

    fn invite_link(&self) -> Option<String> {
        self.text_with("/i/")
    }
}

struct Containing(&'static str);

impl Selector for Containing {
    type Output = String;

    fn select(&mut self, candidate: Candidate<'_>) -> Option<String> {
        match candidate {
            Candidate::Text { content, .. } if content.contains(self.0) => Some(content.to_owned()),
            _ => None,
        }
    }

    fn description(&self) -> String {
        format!("a text that contains {}", self.0)
    }
}

struct Backstage {
    runtime: Runtime,
    port: u16,
    owner_link: String,
    fingerprint: Option<Fingerprint>,
    _stop: oneshot::Sender<()>,
    _data: Directory,
}

impl Backstage {
    fn address(&self) -> String {
        format!("localhost:{}", self.port)
    }
}

fn address(text: String) -> Message {
    Message::Servers(servers::Message::AddressChanged(text))
}

fn name(text: &str) -> Name {
    Name::parse(text).expect(text)
}

fn home(directory: &Directory) -> Home {
    Home::at(directory.path())
}

fn open(
    language: Language,
    directory: &Directory,
) -> Driver<impl Program<Message = Message, Theme = Theme> + 'static> {
    open_with(language, directory, &[])
}

fn open_with(
    language: Language,
    directory: &Directory,
    arguments: &[&str],
) -> Driver<impl Program<Message = Message, Theme = Theme> + 'static> {
    let arguments = Arguments::parse(arguments.iter().map(|argument| (*argument).to_owned()));
    Driver::start(shell::application(
        language,
        Some(home(directory)),
        arguments,
    ))
}

fn new_profile_name(name: &str) -> Message {
    Message::Profiles(profiles::Message::NewNameChanged(name.to_owned()))
}

fn edit(text: &str) -> Message {
    Message::Profiles(profiles::Message::EditChanged(text.to_owned()))
}

fn start_server() -> Backstage {
    start_server_with(Tls::Off)
}

fn start_server_with(tls: Tls) -> Backstage {
    let runtime = Runtime::new().expect("a runtime for the server");
    let data = Directory::new();
    let config = Config {
        data: data.path().to_owned(),
        tls,
        ..Config::development(0)
    };
    let server = runtime
        .block_on(Server::bind(&config))
        .expect("the server can bind");
    let port = server.addresses()[0].port();
    let fingerprint = server.fingerprint();
    let owner_link = server
        .owner_link()
        .expect("a new server has no owner")
        .to_owned();
    let (stop, stopped) = oneshot::channel::<()>();
    runtime.spawn(server.run(async {
        let _ = stopped.await;
    }));
    Backstage {
        runtime,
        port,
        owner_link,
        fingerprint,
        _stop: stop,
        _data: data,
    }
}

fn invite_and_answer_in_a_channel_as_ben(backstage: &Backstage, channel: &str) -> String {
    let channel = channel.to_owned();
    let destination = Destination::parse(&backstage.owner_link).expect("a valid owner link");
    let identity = Identity {
        seed: Seed::from_bytes([2; 32]),
        name: Some(name("ben")),
    };
    let (invited, invitation) = std::sync::mpsc::channel();
    backstage.runtime.spawn(async move {
        let mut events = Box::pin(sign_in(destination, identity, Trust::default()));
        let mut link = None;
        while let Some(event) = events.next().await {
            match event {
                Event::Connected(connected) => {
                    let invite = ClientMessage::Invite {
                        reference: None,
                        lifetime: None,
                        uses: None,
                    };
                    assert!(connected.send(invite));
                    let create = ClientMessage::ChannelCreate {
                        reference: None,
                        channel: channel.clone(),
                        kind: ChannelKind::Public,
                    };
                    assert!(connected.send(create));
                    link = Some(connected);
                }
                Event::Received(ServerMessage::Invitations { list, .. }) => {
                    let _ = invited.send(list[0].link.clone());
                }
                Event::Received(ServerMessage::Message { channel, from, .. })
                    if from == name("ana") =>
                {
                    let answer = ClientMessage::Send {
                        reference: None,
                        channel,
                        text: "Hi ana".to_owned(),
                    };
                    assert!(link.as_ref().is_some_and(|link| link.send(answer)));
                }
                Event::Received(_) | Event::Line(_) => {}
                Event::Disconnected(_) => break,
            }
        }
    });
    invitation
        .recv_timeout(PATIENCE)
        .expect("ben makes an invitation in time")
}

fn join(backstage: &Backstage, link: &str, who: &str) {
    let destination = Destination::parse(link).expect("a valid invite link");
    let identity = Identity {
        seed: Seed::from_bytes([3; 32]),
        name: Some(name(who)),
    };
    let (joined, welcome) = std::sync::mpsc::channel();
    backstage.runtime.spawn(async move {
        let mut events = Box::pin(sign_in(destination, identity, Trust::default()));
        let mut links = Vec::new();
        while let Some(event) = events.next().await {
            match event {
                Event::Connected(link) => {
                    links.push(link);
                    let _ = joined.send(());
                }
                Event::Received(_) | Event::Line(_) => {}
                Event::Disconnected(_) => break,
            }
        }
    });
    welcome
        .recv_timeout(PATIENCE)
        .expect("the user joins in time");
}

fn servers_file(directory: &Directory) -> PathBuf {
    directory.join("profiles").join("1").join("servers.toml")
}

fn settings_file(directory: &Directory) -> PathBuf {
    directory.join("profiles").join("1").join("settings.toml")
}

#[test]
fn a_user_makes_a_second_profile_and_opens_it() {
    let backstage = start_server();
    let directory = Directory::new();
    let mut driver = open(Language::English, &directory);
    driver.expect("Connect");
    driver.sign_up(backstage.owner_link.clone(), "ana");
    driver.expect("ana · connected");
    driver.act("click \"Leave\"");
    driver.expect("Servers");

    driver.act("click \"Profiles\"");
    driver.expect("Open in this window");
    driver.send(new_profile_name("work"));
    driver.act("click \"Make a profile\"");
    driver.expect("work");
    driver.act("click \"Open\"");

    driver.expect("Connect");
    driver.expect_no("Servers");
    assert!(!driver.shows(&backstage.address()));
    let second = directory.join("profiles").join("2");
    assert!(second.join("seed").is_file());
    assert!(second.join("settings.toml").is_file());
    let state = std::fs::read_to_string(directory.join("state.toml")).expect("a state file");
    assert_eq!(state, "last-profile = 2\n");
    let first = std::fs::read_to_string(directory.join("profiles").join("1").join("seed"));
    let other = std::fs::read_to_string(second.join("seed"));
    assert_ne!(first.expect("a seed"), other.expect("a seed"));
    drop(driver);

    let mut next = open(Language::English, &directory);
    next.expect("Connect");
    assert!(!next.shows("Servers"));
    next.act("click \"Profiles\"");
    next.expect("Open in this window");
    next.act("click \"Open\"");
    next.expect("ana · connected");
}

#[test]
fn a_user_renames_a_profile_and_deletes_a_profile() {
    let directory = Directory::new();
    let mut driver = open(Language::English, &directory);
    driver.expect("Connect");
    driver.act("click \"Profiles\"");
    driver.expect("Open in this window");
    driver.send(new_profile_name("work"));
    driver.act("click \"Make a profile\"");
    driver.expect("work");

    driver.act("click \"Rename\"");
    driver.expect("Save");
    driver.send(edit("home"));
    driver.act("click \"Save\"");
    driver.expect("home");
    driver.expect_no("Save");
    let profile = directory.join("profiles").join("1").join("profile.toml");
    let name = std::fs::read_to_string(profile).expect("a profile file");
    assert_eq!(name, "name = \"home\"\n");

    driver.send(new_profile_name("HOME"));
    driver.act("click \"Make a profile\"");
    driver.expect("A different profile has this name.");
    driver.send(new_profile_name(""));

    driver.act("click \"Delete\"");
    driver.expect(
        "If you delete the profile \"work\", you lose its accounts on all servers. No person can get them back.",
    );
    driver.act("click \"Delete this profile\"");
    driver.send(edit("wor"));
    driver.act("click \"Delete this profile\"");
    assert!(directory.join("profiles").join("2").exists());
    driver.send(edit("work"));
    driver.act("click \"Delete this profile\"");

    driver.expect_no("Delete this profile");
    assert!(!driver.shows("work"));
    assert!(!directory.join("profiles").join("2").exists());
    assert!(directory.join("profiles").join("1").exists());
}

#[test]
fn the_command_line_opens_a_profile_by_its_name() {
    let directory = Directory::new();
    let profiles = Profiles::new(home(&directory));
    profiles.create("main").expect("a profile");
    profiles.create("work").expect("a profile");

    let mut driver = open_with(Language::English, &directory, &["--profile", "work"]);

    driver.expect("Connect");
    let state = std::fs::read_to_string(directory.join("state.toml")).expect("a state file");
    assert_eq!(state, "last-profile = 2\n");

    let mut second = open_with(Language::English, &directory, &["--profile=main"]);
    second.expect("Connect");
    second.act("click \"Profiles\"");
    second.expect("Open in this window");
    second.act("click \"Open\"");
    second.expect("This profile is open in a different window.");
    second.act("click \"Delete\"");
    second.send(edit("work"));
    second.act("click \"Delete this profile\"");
    second.expect("This profile is open in a different window.");
    assert!(directory.join("profiles").join("2").exists());
    drop(driver);
}

#[test]
fn the_window_tells_about_a_problem_of_the_command_line() {
    let directory = Directory::new();

    let mut unknown = open_with(Language::English, &directory, &["--profile", "garden"]);
    unknown.expect("No profile has the name \"garden\".");
    assert!(!unknown.shows("Connect"));
    assert!(!directory.join("profiles").exists());

    let mut argument = open_with(Language::Spanish, &directory, &["--rapido"]);
    argument.expect(
        "Post no conoce el argumento \"--rapido\". Para abrir un perfil, usa: post --profile <nombre>",
    );

    let mut missing = open_with(Language::English, &directory, &["--profile"]);
    missing.expect("Give the name of a profile after --profile.");
}

#[test]
fn a_developer_reads_the_lines_and_sends_a_line_in_the_console() {
    let backstage = start_server();
    let directory = Directory::new();
    let mut driver = open(Language::English, &directory);
    driver.expect("Connect");
    assert!(!driver.shows("Protocol console"));

    driver.act("click \"Settings\"");
    driver.act("click \"Protocol console (F12 key)\"");
    driver.expect("The console shows the lines of a connection.");
    driver.act("click \"Back\"");
    driver.expect("Connect");
    driver.sign_up(backstage.owner_link.clone(), "ana");
    driver.expect("ana · connected");

    for kind in ["hello", "challenge", "identify", "register", "welcome"] {
        let part: &'static str = format!("\"type\":\"{kind}\"").leak();
        driver.expect_text_with(part);
    }
    assert!(driver.text_with("\"name\":\"ana\"").is_some());

    let create = r#"{"type":"channel-create","ref":1,"channel":"general","kind":"public"}"#;
    let draft = console::Message::DraftChanged(create.to_owned());
    driver.send(Message::Console(draft));
    driver.act("click \"Send\"");
    driver.expect("#general");

    let typed = r#"{"type":"send","channel":"general","text":"from the console"}"#;
    let draft = console::Message::DraftChanged(typed.to_owned());
    driver.send(Message::Console(draft));
    driver.act("click \"Send\"");
    driver.expect("from the console");
    driver.expect(typed);

    let draft = console::Message::DraftChanged("dance".to_owned());
    driver.send(Message::Console(draft));
    driver.act("click \"Send\"");
    let refused = driver.expect_text_with("bad-message");
    assert!(refused.contains("\"type\":\"error\""), "{refused}");

    driver.act("click \"Clear\"");
    driver.expect_no(typed);
    let settings = std::fs::read_to_string(settings_file(&directory)).expect("a settings file");
    assert!(settings.contains("console = true\n"), "{settings}");

    driver.act("click \"Close\"");
    driver.expect_no("Protocol console");
    driver.expect("from the console");
    let settings = std::fs::read_to_string(settings_file(&directory)).expect("a settings file");
    assert!(settings.contains("console = false\n"), "{settings}");
}

#[test]
fn a_user_changes_the_language_and_the_application_keeps_it() {
    let directory = Directory::new();
    let mut driver = open(Language::English, &directory);
    driver.expect("Connect");
    let file = settings_file(&directory);
    let text = std::fs::read_to_string(&file).expect("a settings file");
    assert!(
        text.starts_with("# The settings of this Post profile.\n"),
        "{text}"
    );
    assert!(text.contains("language = \"system\"\n"), "{text}");

    driver.act("click \"Settings\"");
    driver.expect("Language");
    driver.expect(&format!(
        "Post keeps these settings in this file: {}",
        file.display()
    ));
    driver.act("click \"Español (Latinoamérica)\"");
    driver.expect("Idioma");
    driver.act("click \"Más grande\"");
    driver.expect("110%");
    driver.act("click \"Volver\"");
    driver.expect("Conectar");

    let text = std::fs::read_to_string(&file).expect("a settings file");
    assert!(
        text.starts_with("# The settings of this Post profile.\n"),
        "{text}"
    );
    assert!(text.contains("language = \"es-419\"\n"), "{text}");
    assert!(text.contains("text-size = 110\n"), "{text}");
    drop(driver);

    let mut second = open(Language::English, &directory);
    second.expect("Conectar");
    second.act("click \"Configuración\"");
    second.expect("110%");
}

#[test]
fn the_application_obeys_a_change_in_the_settings_file() {
    let directory = Directory::new();
    let mut driver = open(Language::English, &directory);
    driver.expect("Connect");
    let file = settings_file(&directory);

    std::fs::write(&file, "language = \"es-419\"\n").expect("a settings file");
    driver.expect("Conectar");

    std::fs::write(&file, "language = = \"en\"\n").expect("a settings file");
    driver.act("click \"Configuración\"");
    driver.expect(
        "El archivo de configuración no es TOML válido. Post ignora el archivo hasta que lo corrijas.",
    );

    driver.act("click \"English\"");
    driver.expect("Language");
    let text = std::fs::read_to_string(&file).expect("a settings file");
    assert!(text.contains("language = \"en\"\n"), "{text}");
    assert!(
        !driver.shows(
            "The settings file is not valid TOML. Post ignores the file until you correct it."
        )
    );
}

#[test]
fn a_user_sees_a_note_when_the_settings_file_cannot_take_a_change() {
    let directory = Directory::new();
    let file = settings_file(&directory);
    std::fs::create_dir_all(&file).expect("a directory in the place of the file");
    let mut driver = open(Language::English, &directory);
    driver.expect("Connect");

    driver.act("click \"Settings\"");
    driver.act("click \"12 hours\"");

    driver.expect("Post could not write the settings file.");
}

#[test]
fn the_owner_makes_an_invite_link_for_a_second_user_and_cancels_it() {
    let backstage = start_server();
    let directory = Directory::new();
    let mut driver = open(Language::English, &directory);
    driver.expect("Connect");
    driver.sign_up(backstage.owner_link.clone(), "ana");
    driver.expect("ana · connected");

    driver.act("click \"Invitations\"");
    driver.expect("The server has no open invitations.");
    driver.act("click \"No end\"");
    driver.act("click \"5\"");
    driver.act("click \"New invite link\"");
    driver.expect("No end · Uses: 0 of 5");
    let link = driver.invite_link().expect("the panel shows the link");
    assert!(
        link.starts_with(&format!("http://localhost:{}/i/", backstage.port)),
        "{link}"
    );
    driver.act("click \"Copy\"");
    driver.expect("Copied");

    join(&backstage, &link, "ben");
    driver.act("click \"Back\"");
    driver.expect_no("Copied");
    driver.act("click \"Invitations\"");
    driver.expect("No end · Uses: 1 of 5");

    driver.act("click \"Cancel invitation\"");
    driver.expect("The server has no open invitations.");
    assert!(driver.invite_link().is_none());
}

#[test]
fn a_user_changes_the_name_and_cannot_change_it_again_at_once() {
    let backstage = start_server();
    let directory = Directory::new();
    let mut driver = open(Language::English, &directory);
    driver.expect("Connect");
    driver.sign_up(backstage.owner_link.clone(), "ana");
    driver.expect("ana · connected");
    driver.act("click \"Channels\"");
    driver.expect("Name of a new channel");
    driver.type_text("plans");
    driver.act("click \"Make a channel\"");
    driver.expect("#plans");

    driver.act("click \"Change name\"");
    driver.expect("Your name on this server");
    driver.type_after("ana", "2");
    driver.act("type enter");

    driver.expect("ana2 · connected");
    driver.expect("ana is now ana2");
    assert!(!driver.shows("Your name on this server"));
    driver.expect_in_file(&servers_file(&directory), "name = \"ana2\"");

    driver.act("click \"Change name\"");
    driver.expect("Your name on this server");
    driver.type_after("ana2", "3");
    driver.act("type enter");
    driver.expect("You changed your name a short time ago. Try again later.");

    driver.act("click \"Back\"");
    driver.expect("ana is now ana2");
    assert!(!driver.shows("You changed your name a short time ago. Try again later."));
    driver.type_text("Hello");
    driver.act("type enter");
    driver.expect("Hello");
}

#[test]
fn a_new_user_joins_with_an_invite_link_and_chats_with_a_second_user() {
    let backstage = start_server();
    let invite_link = invite_and_answer_in_a_channel_as_ben(&backstage, "garden");
    let directory = Directory::new();
    let mut driver = open(Language::English, &directory);

    driver.expect("Connect");
    driver.sign_up(invite_link, "ana");

    driver.expect("ana · connected");
    driver.act("click \"Channels\"");
    driver.expect("#garden");
    driver.act("click \"Join\"");
    driver.expect("ana joined");

    driver.type_text("Hello");
    driver.act("type enter");

    driver.expect("Hello");
    driver.expect("Hi ana");
    driver.expect("ben");

    driver.act("click \"Leave\"");
    driver.expect("Connect");

    let profile = directory.join("profiles").join("1");
    let name = std::fs::read_to_string(profile.join("profile.toml")).expect("a profile file");
    assert_eq!(name, "name = \"main\"\n");
    let servers = std::fs::read_to_string(profile.join("servers.toml")).expect("a servers file");
    assert!(
        servers.contains(&format!("address = \"{}\"", backstage.address())),
        "{servers}"
    );
    assert!(servers.contains("name = \"ana\""), "{servers}");
}

#[test]
fn a_user_joins_a_channel_chats_there_and_makes_a_channel() {
    let backstage = start_server();
    let invite_link = invite_and_answer_in_a_channel_as_ben(&backstage, "garden");
    let directory = Directory::new();
    let mut driver = open(Language::English, &directory);
    driver.expect("Connect");
    driver.sign_up(invite_link, "ana");
    driver.expect("ana · connected");
    assert!(!driver.shows("#garden"));

    driver.act("click \"Channels\"");
    driver.expect("#garden");
    driver.expect("1 member");
    driver.act("click \"Join\"");

    driver.expect("Leave channel");
    driver.expect("ana joined");
    driver.type_text("Hello");
    driver.act("type enter");
    driver.expect("Hello");
    driver.expect("Hi ana");

    driver.act("click \"#lobby\"");
    driver.expect_no("Hi ana");
    driver.expect_no("Leave channel");

    driver.act("click \"Channels\"");
    driver.expect("Name of a new channel");
    driver.expect("2 members");
    driver.type_text("plans");
    driver.act("click \"Make a channel\"");
    driver.expect("#plans");
    driver.expect("Leave channel");

    driver.act("click \"#garden\"");
    driver.expect("Hi ana");
    driver.act("click \"Leave channel\"");
    driver.expect_no("#garden");
    driver.expect("#plans");
    driver.expect_no("Leave channel");
}

#[test]
fn a_user_signs_in_again_with_the_account_of_the_profile() {
    let backstage = start_server();
    let directory = Directory::new();
    let mut first = open(Language::English, &directory);
    first.expect("Connect");
    first.sign_up(backstage.owner_link.clone(), "ana");
    first.expect("ana · connected");
    drop(first);

    let mut second = open(Language::English, &directory);

    second.expect("ana · connected");
}

#[test]
fn a_user_removes_a_server_from_the_list() {
    let backstage = start_server();
    let directory = Directory::new();
    let mut driver = open(Language::English, &directory);
    driver.expect("Connect");
    driver.sign_up(backstage.owner_link.clone(), "ana");
    driver.expect("ana · connected");
    driver.act("click \"Leave\"");
    driver.expect("Servers");
    assert!(driver.shows(&backstage.address()));

    driver.act("click \"Remove\"");

    assert!(!driver.shows(&backstage.address()));
    assert!(!driver.shows("Servers"));
    let servers = directory.join("profiles").join("1").join("servers.toml");
    let servers = std::fs::read_to_string(servers).expect("a servers file");
    assert!(!servers.contains("address"), "{servers}");
}

#[test]
fn a_user_accepts_the_certificate_of_a_server_and_signs_in_with_tls() {
    let backstage = start_server_with(Tls::Own);
    let fingerprint = backstage
        .fingerprint
        .expect("a server with TLS has a certificate");
    assert!(backstage.owner_link.starts_with("https://localhost:"));
    let directory = Directory::new();
    let mut driver = open(Language::English, &directory);
    driver.expect("Connect");

    driver.sign_up(backstage.owner_link.clone(), "ana");

    driver.expect("This server has a certificate that no public authority signed.");
    let groups = fingerprint.grouped();
    driver.expect(groups[..39].trim());
    driver.expect(groups[39..].trim());
    assert!(!driver.shows("ana · connected"));

    driver.act("click \"Accept the certificate\"");

    driver.expect("ana · connected");
    let servers = servers_file(&directory);
    driver.expect_in_file(&servers, "tls = true");
    driver.expect_in_file(&servers, &format!("certificate = \"{fingerprint}\""));
}

#[test]
fn a_user_without_an_account_sees_that_an_invite_link_is_necessary() {
    let backstage = start_server();
    let directory = Directory::new();
    let mut driver = open(Language::English, &directory);

    driver.expect("Connect");
    driver.sign_up(backstage.address(), "ana");

    driver.expect("You have no account on this server. Paste an invite link to join.");
}

#[test]
fn a_user_sees_a_note_when_the_profile_cannot_save_the_server() {
    let backstage = start_server();
    let directory = Directory::new();
    let mut driver = open(Language::English, &directory);
    driver.expect("Connect");
    let servers = directory.join("profiles").join("1").join("servers.toml");
    std::fs::create_dir(servers).expect("a directory in the place of the file");

    driver.sign_up(backstage.owner_link.clone(), "ana");

    driver.expect("ana · connected");
    driver.expect("Post could not save this server in your profile.");
}

#[test]
fn a_user_sees_a_problem_when_no_server_answers() {
    let listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("a free port");
    let port = listener.local_addr().expect("a local address").port();
    drop(listener);
    let directory = Directory::new();
    let mut driver = open(Language::Spanish, &directory);

    driver.expect("Conectar");
    driver.sign_up(format!("localhost:{port}"), "ana");

    driver.expect("Ningún servidor de Post responde en esta dirección.");
    let profile = directory.join("profiles").join("1").join("profile.toml");
    let name = std::fs::read_to_string(profile).expect("a profile file");
    assert_eq!(name, "name = \"principal\"\n");
}

#[test]
fn a_second_window_cannot_open_the_profile_of_the_first_window() {
    let directory = Directory::new();
    let mut first = open(Language::English, &directory);
    first.expect("Connect");

    let mut second = open(Language::Spanish, &directory);

    second.expect(
        "Este perfil está abierto en otra ventana de Post. Cierra esa ventana y vuelve a iniciar Post.",
    );
    assert!(!second.shows("Conectar"));
}

#[test]
fn the_window_tells_that_post_has_no_directory_for_its_data() {
    let mut driver = Driver::start(shell::application(
        Language::English,
        None,
        Ok(Arguments::default()),
    ));

    driver.expect(
        "Post cannot find a directory for its data. Set the environment variable POST_HOME to a directory.",
    );
    assert!(!driver.shows("Connect"));
}
