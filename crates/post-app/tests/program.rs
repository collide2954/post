use std::net::Ipv4Addr;
use std::thread;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use iced::{Program, Size, Theme};
use iced_test::emulator::{self, Emulator, Mode};
use iced_test::futures::futures::channel::mpsc;
use iced_test::{Instruction, Simulator};
use post_app::app::{Message, servers};
use post_app::i18n::Language;
use post_app::shell;
use post_client::{Destination, Event, Home, Identity, Seed, Trust, sign_in};
use post_proto::{ClientMessage, Name, ServerMessage};
use post_server::Server;
use post_server::config::Config;
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
        driver.pump();
        driver
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
        self.pump();
    }

    fn type_text(&mut self, text: &str) {
        for character in text.chars() {
            self.act(&format!("type \"{character}\""));
        }
    }

    fn send(&mut self, message: Message) {
        self.emulator.update(&self.program, message);
        self.pump();
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
}

struct Backstage {
    runtime: Runtime,
    port: u16,
    owner_link: String,
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

fn start_server() -> Backstage {
    let runtime = Runtime::new().expect("a runtime for the server");
    let data = Directory::new();
    let config = Config {
        data: data.path().to_owned(),
        ..Config::development(0)
    };
    let server = runtime
        .block_on(Server::bind(&config))
        .expect("the server can bind");
    let port = server.addresses()[0].port();
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
        _stop: stop,
        _data: data,
    }
}

fn invite_and_answer_as_ben(backstage: &Backstage) -> String {
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
                    link = Some(connected);
                }
                Event::Received(ServerMessage::Invitations { list, .. }) => {
                    let _ = invited.send(list[0].link.clone());
                }
                Event::Received(ServerMessage::Message { from, .. }) if from == name("ana") => {
                    let answer = ClientMessage::Send {
                        reference: None,
                        channel: "lobby".to_owned(),
                        text: "Hi ana".to_owned(),
                    };
                    assert!(link.as_ref().is_some_and(|link| link.send(answer)));
                }
                Event::Received(_) => {}
                Event::Disconnected(_) => break,
            }
        }
    });
    invitation
        .recv_timeout(PATIENCE)
        .expect("ben makes an invitation in time")
}

#[test]
fn a_new_user_joins_with_an_invite_link_and_chats_with_a_second_user() {
    let backstage = start_server();
    let invite_link = invite_and_answer_as_ben(&backstage);
    let directory = Directory::new();
    let mut driver = Driver::start(shell::application(
        Language::English,
        Some(home(&directory)),
    ));

    driver.expect("Connect");
    driver.send(address(invite_link));
    driver.type_text("ana");
    driver.act("type tab");
    driver.act("type enter");

    driver.expect("#lobby");
    driver.expect("ana · connected");
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
fn a_user_signs_in_again_with_the_account_of_the_profile() {
    let backstage = start_server();
    let directory = Directory::new();
    let mut first = Driver::start(shell::application(
        Language::English,
        Some(home(&directory)),
    ));
    first.expect("Connect");
    first.send(address(backstage.owner_link.clone()));
    first.type_text("ana");
    first.act("type enter");
    first.expect("ana · connected");
    drop(first);

    let mut second = Driver::start(shell::application(
        Language::English,
        Some(home(&directory)),
    ));
    second.expect("Connect");
    second.send(address(backstage.address()));
    second.act("type enter");

    second.expect("ana · connected");
}

#[test]
fn a_user_without_an_account_sees_that_an_invite_link_is_necessary() {
    let backstage = start_server();
    let directory = Directory::new();
    let mut driver = Driver::start(shell::application(
        Language::English,
        Some(home(&directory)),
    ));

    driver.expect("Connect");
    driver.send(address(backstage.address()));
    driver.type_text("ana");
    driver.act("type enter");

    driver.expect("You have no account on this server. Paste an invite link to join.");
}

#[test]
fn a_user_sees_a_note_when_the_profile_cannot_save_the_server() {
    let backstage = start_server();
    let directory = Directory::new();
    let mut driver = Driver::start(shell::application(
        Language::English,
        Some(home(&directory)),
    ));
    driver.expect("Connect");
    let servers = directory.join("profiles").join("1").join("servers.toml");
    std::fs::create_dir(servers).expect("a directory in the place of the file");

    driver.send(address(backstage.owner_link.clone()));
    driver.type_text("ana");
    driver.act("type enter");

    driver.expect("ana · connected");
    driver.expect("Post could not save this server in your profile.");
}

#[test]
fn a_user_sees_a_problem_when_no_server_answers() {
    let listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("a free port");
    let port = listener.local_addr().expect("a local address").port();
    drop(listener);
    let directory = Directory::new();
    let mut driver = Driver::start(shell::application(
        Language::Spanish,
        Some(home(&directory)),
    ));

    driver.expect("Conectar");
    driver.send(address(format!("localhost:{port}")));
    driver.type_text("ana");
    driver.act("type enter");

    driver.expect("Ningún servidor de Post responde en esta dirección.");
    let profile = directory.join("profiles").join("1").join("profile.toml");
    let name = std::fs::read_to_string(profile).expect("a profile file");
    assert_eq!(name, "name = \"principal\"\n");
}

#[test]
fn a_second_window_cannot_open_the_profile_of_the_first_window() {
    let directory = Directory::new();
    let mut first = Driver::start(shell::application(
        Language::English,
        Some(home(&directory)),
    ));
    first.expect("Connect");

    let mut second = Driver::start(shell::application(
        Language::Spanish,
        Some(home(&directory)),
    ));

    second.expect(
        "Este perfil está abierto en otra ventana de Post. Cierra esa ventana y vuelve a iniciar Post.",
    );
    assert!(!second.shows("Conectar"));
}

#[test]
fn the_window_tells_that_post_has_no_directory_for_its_data() {
    let mut driver = Driver::start(shell::application(Language::English, None));

    driver.expect(
        "Post cannot find a directory for its data. Set the environment variable POST_HOME to a directory.",
    );
    assert!(!driver.shows("Connect"));
}
