use std::io::{BufRead, BufReader};
use std::net::Ipv4Addr;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use futures_util::future::join_all;
use futures_util::{SinkExt, StreamExt};
use post_crypto::{Exchange, Seed};
use post_proto::{ChannelKind, ClientMessage, Code, ServerMessage};
use post_testing::Directory;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

const PATIENCE: Duration = Duration::from_secs(120);
const DELIVERY_BUDGET: Duration = Duration::from_millis(50);
const DELIVERY_ALARM: Duration = Duration::from_millis(150);
const BURST_ROUNDS: usize = 5;
const MEMBERS: usize = 1_000;
const CHANNEL: &str = "budget";
const ROUNDS: usize = 30;
const MEMORY_BUDGET_BYTES: u64 = 1_000_000_000;
const IDLE_CONNECTIONS: usize = 10_000;
const BATCH: usize = 200;

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

async fn read(socket: &mut Socket) -> ServerMessage {
    loop {
        let frame = timeout(PATIENCE, socket.next())
            .await
            .expect("the server sends a message in time")
            .expect("the connection is open")
            .expect("the frame is valid");
        if let Message::Text(line) = frame {
            return ServerMessage::decode(line.as_str()).expect("a valid line");
        }
    }
}

async fn write(socket: &mut Socket, message: &ClientMessage) {
    socket
        .send(Message::text(message.encode()))
        .await
        .expect("the socket is open");
}

async fn register(port: u16, seed: &Seed, who: &str, link: &str) -> Socket {
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{port}/"))
        .await
        .expect("the server accepts the connection");
    let address = format!("localhost:{port}");
    let client = post_crypto::nonce();
    let hello = ClientMessage::Hello {
        reference: None,
        protocol: 0,
        address: address.clone(),
        nonce: client,
        supports: Vec::new(),
    };
    write(&mut socket, &hello).await;
    let ServerMessage::Challenge { key, nonce, .. } = read(&mut socket).await else {
        panic!("the server answers a hello message with a challenge");
    };
    let exchange = Exchange {
        address: &address,
        client: &client,
        server: &nonce,
    };
    let account = seed.account_key(&key);
    let (_, code) = link.rsplit_once('/').expect("a link has a code");
    let register = ClientMessage::Register {
        reference: None,
        key: account.public(),
        proof: account.prove(&key, &exchange),
        name: who.to_owned(),
        invitation: Code::parse(code).expect("a valid code"),
    };
    write(&mut socket, &register).await;
    assert!(
        matches!(read(&mut socket).await, ServerMessage::Welcome { .. }),
        "{who} gets an account"
    );
    socket
}

fn member_seed(number: usize) -> Seed {
    let number = u64::try_from(number).expect("a small number");
    let mut bytes = [9; 32];
    bytes[..8].copy_from_slice(&number.to_le_bytes());
    Seed::from_bytes(bytes)
}

async fn invitation_of_an_owner_with_a_channel(port: u16, owner_link: &str) -> String {
    let mut owner = register(port, &Seed::from_bytes([7; 32]), "owner", owner_link).await;
    let invite = ClientMessage::Invite {
        reference: None,
        lifetime: None,
        uses: None,
    };
    write(&mut owner, &invite).await;
    let link = loop {
        if let ServerMessage::Invitations { list, .. } = read(&mut owner).await {
            break list[0].link.clone();
        }
    };
    let create = ClientMessage::ChannelCreate {
        reference: None,
        channel: CHANNEL.to_owned(),
        kind: ChannelKind::Public,
    };
    write(&mut owner, &create).await;
    tokio::spawn(async move { while owner.next().await.is_some() {} });
    link
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "a measurement of a performance budget"]
async fn the_server_sends_a_message_to_1000_members_in_the_budget() {
    let server = start_server_process();
    let port = server.port;
    let process = server.child.id();
    let link = invitation_of_an_owner_with_a_channel(port, &server.owner_link).await;

    let join = ClientMessage::Join {
        reference: None,
        channel: CHANNEL.to_owned(),
    };
    let (receipts, mut received) = mpsc::unbounded_channel();
    let mut writers = Vec::with_capacity(MEMBERS);
    for number in 0..MEMBERS {
        let name = format!("m{number:04}");
        let mut socket = register(port, &member_seed(number), &name, &link).await;
        write(&mut socket, &join).await;
        let (write_half, mut read_half) = socket.split();
        let receipts = receipts.clone();
        tokio::spawn(async move {
            while let Some(Ok(frame)) = read_half.next().await {
                if let Message::Text(line) = frame
                    && let Some((_, rest)) = line.as_str().split_once("\"text\":\"ping ")
                {
                    let round: usize = rest.trim_end_matches("\"}").parse().expect("a round");
                    let _ = receipts.send((round, Instant::now()));
                }
            }
        });
        writers.push(write_half);
    }
    tokio::time::sleep(Duration::from_secs(2)).await;

    let before = processor_time(process);
    let mut times = Vec::with_capacity(ROUNDS);
    for round in 0..ROUNDS {
        let send = ClientMessage::Send {
            reference: None,
            channel: CHANNEL.to_owned(),
            text: format!("ping {round}"),
        };
        let start = Instant::now();
        writers[0]
            .send(Message::text(send.encode()))
            .await
            .expect("the socket is open");
        let mut last = start;
        for _ in 0..MEMBERS {
            let (seen, at) = timeout(PATIENCE, received.recv())
                .await
                .expect("each member gets the message in time")
                .expect("the readers run");
            assert_eq!(seen, round);
            last = last.max(at);
        }
        times.push(last - start);
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    if let (Some(before), Some(after)) = (before, processor_time(process)) {
        let rounds = u32::try_from(ROUNDS).expect("a small number");
        println!(
            "processor time of the server for each message: {:?} in the program, {:?} in the system",
            after.0.saturating_sub(before.0) / rounds,
            after.1.saturating_sub(before.1) / rounds
        );
    }

    let mut first = times[..BURST_ROUNDS].to_vec();
    first.sort();
    times.sort();
    let median = times[ROUNDS / 2];
    println!(
        "delivery to {MEMBERS} members: median {median:?}, shortest {:?}, longest {:?}, \
         median of the first {BURST_ROUNDS} rounds {:?}",
        times[0],
        times[ROUNDS - 1],
        first[BURST_ROUNDS / 2]
    );
    println!(
        "the budget of {DELIVERY_BUDGET:?}: {}",
        if median <= DELIVERY_BUDGET {
            "met"
        } else {
            "not met"
        }
    );
    assert!(median <= DELIVERY_ALARM, "the median is {median:?}");
}

struct ServerProcess {
    child: Child,
    port: u16,
    owner_link: String,
    _data: Directory,
}

impl Drop for ServerProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn start_server_process() -> ServerProcess {
    let data = Directory::new();
    let listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("a free port");
    let port = listener.local_addr().expect("a local address").port();
    drop(listener);
    let mut child = Command::new(env!("CARGO_BIN_EXE_post-server"))
        .args(["--port", &port.to_string()])
        .current_dir(data.path())
        .stdout(Stdio::piped())
        .spawn()
        .expect("the server program starts");
    let output = child.stdout.take().expect("the output of the server");
    let mut lines = BufReader::new(output).lines();
    let mut owner_link = None;
    loop {
        let line = lines
            .next()
            .expect("the server tells that it listens")
            .expect("a line of text");
        if line.starts_with("http") {
            owner_link = Some(line);
        } else if line.contains("Press Ctrl+C") {
            break;
        }
    }
    std::thread::spawn(move || lines.for_each(drop));
    ServerProcess {
        child,
        port,
        owner_link: owner_link.expect("a new server prints an owner link"),
        _data: data,
    }
}

fn processor_time(process: u32) -> Option<(Duration, Duration)> {
    let (user, system): (f64, f64) = if cfg!(windows) {
        let script = format!(
            "$p = Get-Process -Id {process}; \
             '{{0}} {{1}}' -f $p.UserProcessorTime.TotalMilliseconds, $p.PrivilegedProcessorTime.TotalMilliseconds"
        );
        let output = Command::new("powershell")
            .args(["-NoProfile", "-Command", &script])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&output.stdout).replace(',', ".");
        let (user, system) = text.trim().split_once(' ')?;
        (
            user.parse::<f64>().ok()? / 1000.0,
            system.parse::<f64>().ok()? / 1000.0,
        )
    } else if cfg!(target_os = "linux") {
        let text = std::fs::read_to_string(format!("/proc/{process}/stat")).ok()?;
        let (_, fields) = text.rsplit_once(") ")?;
        let mut fields = fields.split(' ').skip(11);
        let user: f64 = fields.next()?.parse().ok()?;
        let system: f64 = fields.next()?.parse().ok()?;
        (user / 100.0, system / 100.0)
    } else {
        let output = Command::new("ps")
            .args(["-o", "time=", "-p", &process.to_string()])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&output.stdout).into_owned();
        let mut seconds = 0.0;
        for part in text.trim().split(':') {
            seconds = seconds * 60.0 + part.parse::<f64>().ok()?;
        }
        (seconds, 0.0)
    };
    Some((
        Duration::from_secs_f64(user),
        Duration::from_secs_f64(system),
    ))
}
fn resident_bytes(process: u32) -> Option<u64> {
    let kibibytes: u64 = if cfg!(windows) {
        let filter = format!("PID eq {process}");
        let output = Command::new("tasklist")
            .args(["/FI", &filter, "/FO", "CSV", "/NH"])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&output.stdout).into_owned();
        let field = text.trim().rsplit('"').nth(1)?;
        let digits: String = field.chars().filter(char::is_ascii_digit).collect();
        digits.parse().ok()?
    } else {
        let output = Command::new("ps")
            .args(["-o", "rss=", "-p", &process.to_string()])
            .output()
            .ok()?;
        String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse()
            .ok()?
    };
    Some(kibibytes * 1024)
}

async fn open_idle_connection(port: u16) -> TcpStream {
    let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port))
        .await
        .expect("the server listens");
    let request = format!(
        "GET / HTTP/1.1\r\nHost: localhost:{port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
         Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .await
        .expect("the request goes to the server");
    let mut answer = Vec::new();
    let mut byte = [0];
    while !answer.ends_with(b"\r\n\r\n") {
        stream
            .read_exact(&mut byte)
            .await
            .expect("the server answers");
        answer.push(byte[0]);
    }
    assert!(
        answer.starts_with(b"HTTP/1.1 101"),
        "the server changes the connection to a WebSocket"
    );
    stream
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "a measurement of a performance budget"]
async fn the_server_holds_10000_idle_connections_in_the_memory_budget() {
    let server = start_server_process();
    let process = server.child.id();
    let empty = resident_bytes(process).expect("the system tells the memory of the server");

    let mut connections = Vec::with_capacity(IDLE_CONNECTIONS);
    while connections.len() < IDLE_CONNECTIONS {
        let batch = (0..BATCH).map(|_| open_idle_connection(server.port));
        let opened = timeout(PATIENCE, join_all(batch))
            .await
            .expect("the server accepts the connections in time");
        connections.extend(opened);
    }
    tokio::time::sleep(Duration::from_secs(3)).await;

    let full = resident_bytes(process).expect("the system tells the memory of the server");
    let count = u64::try_from(connections.len()).expect("a small number");
    let each = full.saturating_sub(empty) / count;
    println!(
        "memory of the server: {} MB without connections, {} MB with {IDLE_CONNECTIONS} idle connections, {each} bytes for each connection",
        empty / 1_000_000,
        full / 1_000_000
    );
    assert_eq!(connections.len(), IDLE_CONNECTIONS);
    assert!(
        full <= MEMORY_BUDGET_BYTES,
        "the server uses {} MB",
        full / 1_000_000
    );
}
