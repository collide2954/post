use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use post_crypto::{Exchange, Seed};
use post_proto::{
    ClientMessage, Code, ErrorCode, Key, MAX_LINE_BYTES, Name, Role, ServerMessage, Signature,
};
use post_server::config::{Config, Listen, Tls};
use post_server::{Server, StartError, new_owner_link};
use post_testing::Directory;
use tokio::net::TcpStream;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

const PATIENCE: Duration = Duration::from_secs(10);

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

struct Running {
    port: u16,
    addresses: Vec<SocketAddr>,
    owner_link: Option<String>,
    config: Config,
    data: Directory,
    stop: oneshot::Sender<()>,
    task: JoinHandle<std::io::Result<()>>,
}

async fn start() -> Running {
    start_with(Directory::new()).await
}

async fn start_with(data: Directory) -> Running {
    let config = Config {
        data: data.path().to_owned(),
        ..Config::development(0)
    };
    let server = Server::bind(&config).await.expect("the server can bind");
    let addresses = server.addresses().to_vec();
    let owner_link = server.owner_link().map(str::to_owned);
    let (stop, stopped) = oneshot::channel::<()>();
    let task = tokio::spawn(server.run(async {
        let _ = stopped.await;
    }));
    Running {
        port: addresses[0].port(),
        addresses,
        owner_link,
        config,
        data,
        stop,
        task,
    }
}

impl Running {
    async fn stop(self) -> Directory {
        self.stop.send(()).expect("the server runs");
        timeout(PATIENCE, self.task)
            .await
            .expect("the server stops in time")
            .expect("the server task does not panic")
            .expect("the server stops without an error");
        self.data
    }
}

async fn connect(address: SocketAddr) -> Socket {
    let (socket, _) = timeout(PATIENCE, connect_async(format!("ws://{address}/")))
        .await
        .expect("the server answers in time")
        .expect("the server accepts the connection");
    socket
}

async fn connect_v4(port: u16) -> Socket {
    connect(SocketAddr::from((Ipv4Addr::LOCALHOST, port))).await
}

async fn write(socket: &mut Socket, message: &ClientMessage) {
    socket
        .send(Message::text(message.encode()))
        .await
        .expect("the socket is open");
}

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

async fn is_closed(socket: &mut Socket) -> bool {
    loop {
        match timeout(PATIENCE, socket.next()).await {
            Ok(None | Some(Err(_) | Ok(Message::Close(_)))) => return true,
            Ok(Some(Ok(_))) => {}
            Err(_) => return false,
        }
    }
}

fn guest(name: &str) -> ClientMessage {
    ClientMessage::Guest {
        reference: None,
        protocol: 0,
        name: name.to_owned(),
    }
}

fn name(text: &str) -> Name {
    Name::parse(text).expect(text)
}

async fn join_as(port: u16, who: &str) -> Socket {
    let mut socket = connect_v4(port).await;
    write(&mut socket, &guest(who)).await;
    assert_eq!(
        read(&mut socket).await,
        ServerMessage::Welcome {
            reference: None,
            name: name(who),
            role: Role::Member,
        }
    );
    assert!(
        matches!(read(&mut socket).await, ServerMessage::Join { name: joined, .. } if joined == name(who))
    );
    socket
}

async fn prove(socket: &mut Socket, address: &str, seed: &Seed) -> (Key, Signature) {
    let client = post_crypto::nonce();
    let hello = ClientMessage::Hello {
        reference: None,
        protocol: 0,
        address: address.to_owned(),
        nonce: client,
        supports: Vec::new(),
    };
    write(socket, &hello).await;
    let ServerMessage::Challenge {
        key, nonce, proof, ..
    } = read(socket).await
    else {
        panic!("the server answers a hello message with a challenge");
    };
    let exchange = Exchange {
        address,
        client: &client,
        server: &nonce,
    };
    assert!(exchange.server_proof_is_valid(&key, &proof));
    let account = seed.account_key(&key);
    (account.public(), account.prove(&key, &exchange))
}

#[tokio::test]
async fn the_owner_makes_an_account_and_signs_in_again() {
    let server = start().await;
    let owner_link = server
        .owner_link
        .clone()
        .expect("a new server has no owner");
    let (start, code) = owner_link.rsplit_once('/').expect("a link has a code");
    assert_eq!(start, format!("http://localhost:{}/i", server.port));
    let seed = Seed::from_bytes([7; 32]);

    let mut first = connect_v4(server.port).await;
    let address = format!("localhost:{}", server.port);
    let (key, proof) = prove(&mut first, &address, &seed).await;
    let register = ClientMessage::Register {
        reference: None,
        key,
        proof,
        name: "ana".to_owned(),
        invitation: Code::parse(code).expect("a valid code"),
    };
    write(&mut first, &register).await;
    let welcome = ServerMessage::Welcome {
        reference: None,
        name: name("ana"),
        role: Role::Owner,
    };
    assert_eq!(read(&mut first).await, welcome);

    let mut second = connect(server.addresses[0]).await;
    let address = format!("127.0.0.1:{}", server.port);
    let (key, proof) = prove(&mut second, &address, &seed).await;
    let identify = ClientMessage::Identify {
        reference: None,
        key,
        proof,
        invitation: None,
    };
    write(&mut second, &identify).await;
    assert_eq!(read(&mut second).await, welcome);
}

async fn register(port: u16, seed: &Seed, who: &str, link: &str) -> ServerMessage {
    let (_, code) = link.rsplit_once('/').expect("a link has a code");
    let mut socket = connect_v4(port).await;
    let (key, proof) = prove(&mut socket, &format!("localhost:{port}"), seed).await;
    let register = ClientMessage::Register {
        reference: None,
        key,
        proof,
        name: who.to_owned(),
        invitation: Code::parse(code).expect("a valid code"),
    };
    write(&mut socket, &register).await;
    read(&mut socket).await
}

async fn identify(port: u16, seed: &Seed) -> ServerMessage {
    let mut socket = connect_v4(port).await;
    let (key, proof) = prove(&mut socket, &format!("localhost:{port}"), seed).await;
    let identify = ClientMessage::Identify {
        reference: None,
        key,
        proof,
        invitation: None,
    };
    write(&mut socket, &identify).await;
    read(&mut socket).await
}

fn welcome(who: &str, role: Role) -> ServerMessage {
    ServerMessage::Welcome {
        reference: None,
        name: name(who),
        role,
    }
}

#[tokio::test]
async fn keeps_the_accounts_when_it_starts_again() {
    let server = start().await;
    let owner_link = server
        .owner_link
        .clone()
        .expect("a new server has no owner");
    let seed = Seed::from_bytes([7; 32]);
    assert_eq!(
        register(server.port, &seed, "ana", &owner_link).await,
        welcome("ana", Role::Owner)
    );
    let data = server.stop().await;
    assert!(data.join("post.db").is_file());

    let server = start_with(data).await;

    assert_eq!(server.owner_link, None);
    assert_eq!(
        identify(server.port, &seed).await,
        welcome("ana", Role::Owner)
    );
    assert_eq!(
        identify(server.port, &Seed::from_bytes([8; 32])).await,
        ServerMessage::error(None, ErrorCode::UnknownAccount)
    );
}

#[tokio::test]
async fn makes_a_new_owner_link_for_a_server_that_runs() {
    let server = start().await;
    let owner_link = server
        .owner_link
        .clone()
        .expect("a new server has no owner");
    let ana = Seed::from_bytes([7; 32]);
    register(server.port, &ana, "ana", &owner_link).await;

    let config = Config {
        listen: Listen::Loopback(server.port),
        ..server.config.clone()
    };
    let link = new_owner_link(&config).expect("the storage is open");

    assert!(link.starts_with(&format!("http://localhost:{}/i/", server.port)));
    assert_ne!(link, owner_link);
    let ben = Seed::from_bytes([8; 32]);
    assert_eq!(
        register(server.port, &ben, "ben", &link).await,
        welcome("ben", Role::Owner)
    );
    assert_eq!(
        identify(server.port, &ana).await,
        welcome("ana", Role::Member)
    );
}

#[tokio::test]
async fn tells_why_it_cannot_start() {
    let data = Directory::new();
    let development = Config {
        data: data.path().to_owned(),
        ..Config::development(0)
    };

    let with_tls = Config {
        tls: Tls::Own,
        ..development.clone()
    };
    let error = Server::bind(&with_tls).await.err().expect("an error");
    assert!(matches!(error, StartError::NoTls));
    assert!(
        error
            .to_string()
            .starts_with("this version cannot use TLS yet")
    );

    let first = Server::bind(&development)
        .await
        .expect("the server can bind");
    let taken = Config {
        listen: Listen::Given(first.addresses().to_vec()),
        ..development.clone()
    };
    let error = Server::bind(&taken).await.err().expect("an error");
    assert!(matches!(error, StartError::Listen { .. }));
    assert!(error.to_string().starts_with("cannot listen on 127.0.0.1:"));

    let file = data.join("a-file");
    std::fs::write(&file, "text").expect("a file");
    let in_a_file = Config {
        data: file.join("data"),
        ..Config::development(0)
    };
    let error = Server::bind(&in_a_file).await.err().expect("an error");
    assert!(matches!(error, StartError::Data { .. }));
    assert!(
        error
            .to_string()
            .starts_with("cannot make the data directory")
    );

    let damaged = Directory::new();
    std::fs::write(damaged.join("post.db"), "This file is not a database.").expect("a file");
    let with_damage = Config {
        data: damaged.path().to_owned(),
        ..Config::development(0)
    };
    let error = Server::bind(&with_damage).await.err().expect("an error");
    assert!(matches!(error, StartError::Store(_)));
    assert!(error.to_string().starts_with("the database has a problem"));
}

#[tokio::test]
async fn listens_on_the_addresses_that_the_configuration_gives() {
    let data = Directory::new();
    let config = Config {
        listen: Listen::Given(vec![SocketAddr::from((Ipv4Addr::LOCALHOST, 0))]),
        data: data.path().to_owned(),
        ..Config::development(0)
    };

    let server = Server::bind(&config).await.expect("the server can bind");

    assert_eq!(server.addresses().len(), 1);
    assert!(server.addresses()[0].ip().is_loopback());
}

#[tokio::test]
async fn two_users_exchange_text() {
    let server = start().await;
    let mut ana = join_as(server.port, "ana").await;
    let mut ben = join_as(server.port, "ben").await;
    assert!(
        matches!(read(&mut ana).await, ServerMessage::Join { channel, name: joined, .. } if channel == "lobby" && joined == name("ben"))
    );

    write(
        &mut ana,
        &ClientMessage::Send {
            reference: Some(1),
            channel: "lobby".to_owned(),
            text: "Hello".to_owned(),
        },
    )
    .await;

    let ServerMessage::Message {
        reference,
        channel,
        from,
        time,
        text,
    } = read(&mut ana).await
    else {
        panic!("the sender gets the message");
    };
    assert_eq!(reference, Some(1));
    assert_eq!(channel, "lobby");
    assert_eq!(from, name("ana"));
    assert_eq!(text, "Hello");
    assert_eq!(
        read(&mut ben).await,
        ServerMessage::Message {
            reference: None,
            channel,
            from,
            time,
            text,
        }
    );

    ben.close(None).await.expect("the socket can close");
    assert!(
        matches!(read(&mut ana).await, ServerMessage::Leave { channel, name: left, .. } if channel == "lobby" && left == name("ben"))
    );
}

#[tokio::test]
async fn keeps_the_connection_open_after_an_error() {
    let server = start().await;
    let mut socket = connect_v4(server.port).await;

    socket
        .send(Message::text("this is not a message"))
        .await
        .expect("the socket is open");
    assert_eq!(
        read(&mut socket).await,
        ServerMessage::error(None, ErrorCode::BadMessage)
    );

    socket
        .send(Message::binary(vec![1, 2, 3]))
        .await
        .expect("the socket is open");
    assert_eq!(
        read(&mut socket).await,
        ServerMessage::error(None, ErrorCode::BadMessage)
    );

    write(&mut socket, &guest("ana")).await;
    assert!(matches!(
        read(&mut socket).await,
        ServerMessage::Welcome { .. }
    ));
}

#[tokio::test]
async fn closes_a_connection_that_sends_a_line_that_is_too_long() {
    let server = start().await;
    let mut socket = connect_v4(server.port).await;

    socket
        .send(Message::text("x".repeat(MAX_LINE_BYTES + 1)))
        .await
        .expect("the socket is open");

    assert!(is_closed(&mut socket).await);
}

#[tokio::test]
async fn listens_only_on_the_loopback_addresses() {
    let server = start().await;

    assert_eq!(
        server.addresses[0],
        SocketAddr::from((Ipv4Addr::LOCALHOST, server.port))
    );
    for address in &server.addresses {
        assert!(address.ip().is_loopback(), "{address}");
        assert_eq!(address.port(), server.port);
    }
    assert!(server.addresses.len() <= 2);
}

#[tokio::test]
async fn accepts_a_connection_on_the_ipv6_loopback_address_if_the_computer_has_one() {
    let server = start().await;
    let ipv6 = SocketAddr::from((Ipv6Addr::LOCALHOST, server.port));

    if server.addresses.contains(&ipv6) {
        let mut socket = connect(ipv6).await;
        write(&mut socket, &guest("ana")).await;
        assert!(matches!(
            read(&mut socket).await,
            ServerMessage::Welcome { .. }
        ));
    }
}

#[tokio::test]
async fn stops_and_closes_each_connection_when_the_stop_signal_comes() {
    let server = start().await;
    let mut ana = join_as(server.port, "ana").await;

    let stopped = tokio::spawn(server.stop());

    assert!(is_closed(&mut ana).await);
    stopped.await.expect("the server stops");
}

#[tokio::test]
async fn answers_a_request_that_is_not_a_websocket_request_with_an_http_error() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let server = start().await;
    let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, server.port))
        .await
        .expect("the server listens");
    stream
        .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .expect("the stream is open");

    let mut answer = String::new();
    timeout(PATIENCE, stream.read_to_string(&mut answer))
        .await
        .expect("the server answers in time")
        .expect("the answer is text");
    assert!(answer.starts_with("HTTP/1.1 4"), "{answer}");
}
