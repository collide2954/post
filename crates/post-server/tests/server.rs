use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use post_crypto::{Exchange, Seed};
use post_proto::{
    ClientMessage, Code, ErrorCode, Key, MAX_LINE_BYTES, Name, Role, ServerMessage, Signature,
};
use post_server::Server;
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
    stop: oneshot::Sender<()>,
    task: JoinHandle<std::io::Result<()>>,
}

async fn start() -> Running {
    let server = Server::bind(0).await.expect("the server can bind");
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
        stop,
        task,
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

    server.stop.send(()).expect("the server runs");

    assert!(is_closed(&mut ana).await);
    timeout(PATIENCE, server.task)
        .await
        .expect("the server stops in time")
        .expect("the server task does not panic")
        .expect("the server stops without an error");
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
