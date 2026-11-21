use std::net::Ipv4Addr;
use std::thread;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use iced::{Program, Size, Theme};
use iced_test::emulator::{self, Emulator, Mode};
use iced_test::futures::futures::channel::mpsc;
use iced_test::{Instruction, Simulator};
use post_app::app::Message;
use post_app::i18n::Language;
use post_app::shell;
use post_client::{Event, ServerAddress, connect};
use post_proto::{ClientMessage, Name, ServerMessage};
use post_server::Server;
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
    _stop: oneshot::Sender<()>,
}

fn start_server() -> Backstage {
    let runtime = Runtime::new().expect("a runtime for the server");
    let server = runtime
        .block_on(Server::bind(0))
        .expect("the server can bind");
    let port = server.addresses()[0].port();
    let (stop, stopped) = oneshot::channel::<()>();
    runtime.spawn(server.run(async {
        let _ = stopped.await;
    }));
    Backstage {
        runtime,
        port,
        _stop: stop,
    }
}

fn name(text: &str) -> Name {
    Name::parse(text).expect(text)
}

fn answer_as_ben(backstage: &Backstage) {
    let address = ServerAddress::parse(&format!("127.0.0.1:{}", backstage.port)).expect("valid");
    let (ready, is_ready) = std::sync::mpsc::channel();
    backstage.runtime.spawn(async move {
        let mut events = Box::pin(connect(address, name("ben")));
        let mut link = None;
        while let Some(event) = events.next().await {
            match event {
                Event::Connected(connected) => {
                    link = Some(connected);
                    let _ = ready.send(());
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
    is_ready
        .recv_timeout(PATIENCE)
        .expect("ben connects in time");
}

#[test]
fn a_user_connects_and_chats_with_a_second_user() {
    let backstage = start_server();
    answer_as_ben(&backstage);
    let mut driver = Driver::start(shell::application(Language::English));

    driver.expect("Connect");
    driver.send(Message::AddressChanged(format!(
        "localhost:{}",
        backstage.port
    )));
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
}

#[test]
fn a_user_sees_a_problem_when_no_server_answers() {
    let listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("a free port");
    let port = listener.local_addr().expect("a local address").port();
    drop(listener);
    let mut driver = Driver::start(shell::application(Language::Spanish));

    driver.expect("Conectar");
    driver.send(Message::AddressChanged(format!("localhost:{port}")));
    driver.type_text("ana");
    driver.act("type enter");

    driver.expect("Ningún servidor de Post responde en esta dirección.");
}
