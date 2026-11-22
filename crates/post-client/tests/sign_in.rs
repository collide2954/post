use std::future::Future;
use std::net::Ipv4Addr;
use std::pin::Pin;
use std::time::Duration;

use futures_util::{SinkExt, Stream, StreamExt};
use post_client::{
    Destination, Direction, Disconnect, Event, Identity, Line, Link, Outgoing, Trust, sign_in,
    sign_in_with_lines,
};
use post_crypto::{Exchange, Fingerprint, Seed, ServerKey};
use post_proto::{
    ChannelKind, ClientMessage, ErrorCode, Key, MAX_LINE_BYTES, Name, Nonce, Part, Role,
    ServerMessage, Timestamp,
};
use post_server::Server;
use post_server::config::{Config, Tls};
use post_testing::Directory;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{WebSocketStream, accept_async};

const PATIENCE: Duration = Duration::from_secs(20);

type Events = Pin<Box<dyn Stream<Item = Event> + Send>>;
type Socket = WebSocketStream<TcpStream>;

struct Running {
    port: u16,
    owner_link: String,
    fingerprint: Option<Fingerprint>,
    stop: oneshot::Sender<()>,
    _data: Directory,
}

fn name(text: &str) -> Name {
    Name::parse(text).expect(text)
}

fn seed(person: u8) -> Seed {
    Seed::from_bytes([person; 32])
}

fn destination(text: &str) -> Destination {
    Destination::parse(text).expect(text)
}

async fn start_server() -> Running {
    start_server_with(Tls::Off).await
}

async fn start_server_with(tls: Tls) -> Running {
    let data = Directory::new();
    let config = Config {
        data: data.path().to_owned(),
        tls,
        ..Config::development(0)
    };
    let server = Server::bind(&config).await.expect("the server can bind");
    let port = server.addresses()[0].port();
    let fingerprint = server.fingerprint();
    let owner_link = server
        .owner_link()
        .expect("a new server has no owner")
        .to_owned();
    let (stop, stopped) = oneshot::channel::<()>();
    tokio::spawn(server.run(async {
        let _ = stopped.await;
    }));
    Running {
        port,
        owner_link,
        fingerprint,
        stop,
        _data: data,
    }
}

fn events(to: &str, person: u8, who: Option<&str>, known: Option<Key>) -> Events {
    let trust = Trust {
        key: known,
        certificate: None,
    };
    events_with(to, person, who, trust)
}

fn events_with(to: &str, person: u8, who: Option<&str>, trust: Trust) -> Events {
    let identity = Identity {
        seed: seed(person),
        name: who.map(name),
    };
    Box::pin(sign_in(destination(to), identity, trust))
}

fn events_with_lines(to: &str, person: u8, who: Option<&str>) -> Events {
    let identity = Identity {
        seed: seed(person),
        name: who.map(name),
    };
    Box::pin(sign_in_with_lines(
        destination(to),
        identity,
        Trust::default(),
    ))
}

async fn line(events: &mut Events) -> Line {
    match next(events).await {
        Some(Event::Line(line)) => line,
        other => panic!("not a line: {other:?}"),
    }
}

async fn line_of(events: &mut Events, direction: Direction, kind: &str) -> Line {
    let line = line(events).await;
    assert_eq!(line.direction, direction, "{}", line.text);
    assert!(
        line.text.contains(&format!("\"type\":\"{kind}\"")),
        "not a {kind} line: {}",
        line.text
    );
    line
}

async fn next(events: &mut Events) -> Option<Event> {
    timeout(PATIENCE, events.next())
        .await
        .expect("an event comes in time")
}

async fn received(events: &mut Events) -> ServerMessage {
    match next(events).await {
        Some(Event::Received(message)) => message,
        other => panic!("not a message from the server: {other:?}"),
    }
}

async fn connected(events: &mut Events) -> Link {
    match next(events).await {
        Some(Event::Connected(link)) => link,
        other => panic!("not connected: {other:?}"),
    }
}

async fn signed_in(events: &mut Events) -> Link {
    let link = connected(events).await;
    assert_eq!(received(events).await, ServerMessage::Ready {});
    link
}

async fn disconnected(events: &mut Events) -> Disconnect {
    let Some(Event::Disconnected(reason)) = next(events).await else {
        panic!("the connection stops");
    };
    assert!(next(events).await.is_none(), "the stream stops");
    reason
}

async fn owner(server: &Running) -> (Events, Link) {
    let mut ana = events(&server.owner_link, 1, Some("ana"), None);
    let link = signed_in(&mut ana).await;
    (ana, link)
}

fn create_general() -> ClientMessage {
    ClientMessage::ChannelCreate {
        reference: None,
        channel: "general".to_owned(),
        kind: ChannelKind::Public,
    }
}

fn to_general(reference: Option<u32>, text: &str) -> ClientMessage {
    ClientMessage::Send {
        reference,
        channel: "general".to_owned(),
        text: text.to_owned(),
    }
}

async fn made_general(link: &Link, events: &mut Events) {
    assert!(link.send(create_general()));
    assert!(matches!(
        received(events).await,
        ServerMessage::Channel { .. }
    ));
}

async fn invite_link(owner: &Link, events: &mut Events) -> String {
    assert!(owner.send(ClientMessage::Invite {
        reference: None,
        lifetime: None,
        uses: None,
    }));
    let ServerMessage::Invitations { list, .. } = received(events).await else {
        panic!("the owner gets the open invitations");
    };
    list[0].link.clone()
}

async fn fake_server<F, Fut>(script: F) -> String
where
    F: FnOnce(Socket, String, Nonce) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send,
{
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("the fake server can bind");
    let port = listener.local_addr().expect("a local address").port();
    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("a client connects");
        let mut socket = accept_async(stream).await.expect("a WebSocket client");
        let hello = socket.next().await.expect("a hello").expect("a frame");
        let Ok(ClientMessage::Hello {
            protocol: 0,
            address,
            nonce,
            ..
        }) = ClientMessage::decode(hello.to_text().expect("text"))
        else {
            panic!("the first message is a hello message");
        };
        script(socket, address, nonce).await;
    });
    format!("127.0.0.1:{port}")
}

async fn say(socket: &mut Socket, message: &ServerMessage) {
    say_line(socket, &message.encode()).await;
}

async fn say_line(socket: &mut Socket, line: &str) {
    socket
        .send(Message::text(line))
        .await
        .expect("the socket is open");
}

async fn hold(mut socket: Socket) {
    while let Some(Ok(_)) = socket.next().await {}
}

fn challenge(key: &ServerKey, address: &str, client: &Nonce, requires: &[&str]) -> ServerMessage {
    let nonce = Nonce::from_bytes([8; 32]);
    let exchange = Exchange {
        address,
        client,
        server: &nonce,
    };
    ServerMessage::Challenge {
        reference: None,
        protocol: 0,
        key: key.public(),
        proof: key.prove(&exchange),
        nonce,
        requires: requires
            .iter()
            .map(|part| Part::parse(part).expect("a valid part"))
            .collect(),
    }
}

fn server_key() -> ServerKey {
    ServerKey::from_bytes(&[9; 32])
}

fn welcome() -> ServerMessage {
    ServerMessage::Welcome {
        reference: None,
        name: name("ana"),
        role: Role::Member,
    }
}

async fn welcomed(mut socket: Socket, address: &str, client: &Nonce) -> Socket {
    say(&mut socket, &challenge(&server_key(), address, client, &[])).await;
    socket.next().await;
    say(&mut socket, &welcome()).await;
    socket
}

#[tokio::test]
async fn a_new_user_makes_an_account_and_signs_in_again() {
    let server = start_server().await;
    let address = format!("127.0.0.1:{}", server.port);

    let mut first = events(&server.owner_link, 1, Some("ana"), None);
    let link = signed_in(&mut first).await;
    assert_eq!(link.name(), &name("ana"));
    assert_eq!(link.role(), Role::Owner);
    let key = *link.server();
    drop(link);
    assert_eq!(disconnected(&mut first).await, Disconnect::Closed);

    let mut second = events(&address, 1, None, Some(key));
    let link = connected(&mut second).await;
    assert_eq!(link.name(), &name("ana"));
    assert_eq!(link.role(), Role::Owner);
    assert_eq!(link.server(), &key);
}

#[tokio::test]
async fn two_users_exchange_text_through_the_server() {
    let server = start_server().await;
    let (mut ana, ana_link) = owner(&server).await;
    let invitation = invite_link(&ana_link, &mut ana).await;
    let mut ben = events(&invitation, 2, Some("ben"), None);
    let ben_link = signed_in(&mut ben).await;
    assert_eq!(ben_link.name(), &name("ben"));
    assert_eq!(ben_link.role(), Role::Member);
    made_general(&ana_link, &mut ana).await;
    assert!(ben_link.send(ClientMessage::Join {
        reference: None,
        channel: "general".to_owned(),
    }));
    assert!(matches!(
        received(&mut ben).await,
        ServerMessage::Channel { .. }
    ));
    for user in [&mut ana, &mut ben] {
        assert!(matches!(
            received(user).await,
            ServerMessage::Join { name: joined, .. } if joined == name("ben")
        ));
    }

    assert!(ana_link.send(to_general(Some(1), "Hello")));

    assert!(matches!(
        received(&mut ana).await,
        ServerMessage::Message { reference: Some(1), from, text, .. }
            if from == name("ana") && text == "Hello"
    ));
    assert!(matches!(
        received(&mut ben).await,
        ServerMessage::Message { reference: None, from, text, .. }
            if from == name("ana") && text == "Hello"
    ));

    drop(ben_link);
    assert_eq!(disconnected(&mut ben).await, Disconnect::Closed);
}

#[tokio::test]
async fn two_connections_of_one_account_exchange_text() {
    let server = start_server().await;
    let (mut first, first_link) = owner(&server).await;
    let mut second = events(&format!("localhost:{}", server.port), 1, None, None);
    let _second_link = signed_in(&mut second).await;
    assert!(first_link.send(create_general()));
    for connection in [&mut first, &mut second] {
        assert!(matches!(
            received(connection).await,
            ServerMessage::Channel { .. }
        ));
    }

    assert!(first_link.send(to_general(Some(1), "Hello")));

    assert!(matches!(
        received(&mut first).await,
        ServerMessage::Message { reference: Some(1), from, text, .. }
            if from == name("ana") && text == "Hello"
    ));
    assert!(matches!(
        received(&mut second).await,
        ServerMessage::Message { reference: None, from, text, .. }
            if from == name("ana") && text == "Hello"
    ));
}

#[tokio::test]
async fn gives_an_error_of_the_server_as_a_message_after_the_sign_in() {
    let server = start_server().await;
    let (mut ana, link) = owner(&server).await;

    assert!(link.send(ClientMessage::Send {
        reference: Some(7),
        channel: "no-such-channel".to_owned(),
        text: "Hello".to_owned(),
    }));

    assert!(matches!(
        received(&mut ana).await,
        ServerMessage::Error {
            reference: Some(7),
            code: ErrorCode::NoSuchChannel,
            ..
        }
    ));
}

#[tokio::test]
async fn tells_that_a_new_account_needs_an_invitation() {
    let server = start_server().await;

    let mut ana = events(&format!("127.0.0.1:{}", server.port), 1, Some("ana"), None);

    assert_eq!(disconnected(&mut ana).await, Disconnect::InvitationRequired);
}

#[tokio::test]
async fn tells_that_a_new_account_needs_a_name() {
    let server = start_server().await;

    let mut ana = events(&server.owner_link, 1, None, None);

    assert_eq!(disconnected(&mut ana).await, Disconnect::NameRequired);
}

#[tokio::test]
async fn tells_why_the_server_refused_the_new_account() {
    let server = start_server().await;
    let link = format!("http://127.0.0.1:{}/i/AAAAAAAAAAAA", server.port);

    let mut ana = events(&link, 1, Some("ana"), None);

    assert_eq!(
        disconnected(&mut ana).await,
        Disconnect::Refused(ErrorCode::InvalidInvitation)
    );
}

#[tokio::test]
async fn tells_that_a_different_account_has_the_name() {
    let server = start_server().await;
    let (mut ana, ana_link) = owner(&server).await;
    let invitation = invite_link(&ana_link, &mut ana).await;

    let mut second = events(&invitation, 2, Some("ana"), None);

    assert_eq!(
        disconnected(&mut second).await,
        Disconnect::Refused(ErrorCode::NameTaken)
    );
}

#[tokio::test]
async fn refuses_a_server_that_does_not_prove_its_key() {
    let address = fake_server(|mut socket, address, client| async move {
        let different = Nonce::from_bytes([3; 32]);
        say(
            &mut socket,
            &challenge(&server_key(), &address, &different, &[]),
        )
        .await;
        let _ = client;
        hold(socket).await;
    })
    .await;

    let mut ana = events(&address, 1, Some("ana"), None);

    assert_eq!(disconnected(&mut ana).await, Disconnect::FalseServer);
}

#[tokio::test]
async fn refuses_a_server_that_has_a_different_key_than_before() {
    let address = fake_server(|mut socket, address, client| async move {
        say(
            &mut socket,
            &challenge(&server_key(), &address, &client, &[]),
        )
        .await;
        hold(socket).await;
    })
    .await;
    let known = ServerKey::from_bytes(&[4; 32]).public();

    let mut ana = events(&address, 1, Some("ana"), Some(known));

    assert_eq!(disconnected(&mut ana).await, Disconnect::DifferentServer);
}

#[tokio::test]
async fn refuses_a_server_that_requires_a_part_that_it_does_not_have() {
    let address = fake_server(|mut socket, address, client| async move {
        say(
            &mut socket,
            &challenge(&server_key(), &address, &client, &["sealed"]),
        )
        .await;
        hold(socket).await;
    })
    .await;

    let mut ana = events(&address, 1, Some("ana"), None);

    assert_eq!(disconnected(&mut ana).await, Disconnect::MissingParts);
}

#[tokio::test]
async fn sends_the_proof_of_its_account_key_for_this_server() {
    let (seen, proof) = oneshot::channel();
    let address = fake_server(|mut socket, address, client| async move {
        let key = server_key();
        let challenge = challenge(&key, &address, &client, &[]);
        let ServerMessage::Challenge { nonce, .. } = &challenge else {
            unreachable!("a challenge message");
        };
        let nonce = *nonce;
        say(&mut socket, &challenge).await;
        let identify = socket.next().await.expect("a message").expect("a frame");
        let Ok(ClientMessage::Identify {
            key: account,
            proof,
            invitation: None,
            ..
        }) = ClientMessage::decode(identify.to_text().expect("text"))
        else {
            panic!("the second message is an identify message");
        };
        let exchange = Exchange {
            address: &address,
            client: &client,
            server: &nonce,
        };
        let is_valid = exchange.account_proof_is_valid(&account, &key.public(), &proof);
        let expected = seed(1).account_key(&key.public()).public();
        seen.send(is_valid && account == expected)
            .expect("the test waits");
        say(&mut socket, &welcome()).await;
        hold(socket).await;
    })
    .await;

    let mut ana = events(&address, 1, None, None);
    let link = connected(&mut ana).await;

    assert!(proof.await.expect("the fake server answers"));
    assert_eq!(link.name(), &name("ana"));
    assert_eq!(link.role(), Role::Member);
    assert_eq!(link.server(), &server_key().public());
}

#[tokio::test]
async fn tells_that_no_server_is_at_the_address() {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("a free port");
    let port = listener.local_addr().expect("a local address").port();
    drop(listener);

    let mut ana = events(&format!("127.0.0.1:{port}"), 1, Some("ana"), None);

    assert_eq!(disconnected(&mut ana).await, Disconnect::Unreachable);
}

#[tokio::test]
async fn tells_that_a_program_at_the_address_is_not_a_post_server() {
    use tokio::io::AsyncWriteExt;

    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("the fake server can bind");
    let port = listener.local_addr().expect("a local address").port();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("a client connects");
        let _ = stream.write_all(b"220 mail server ready\r\n").await;
    });

    let mut ana = events(&format!("127.0.0.1:{port}"), 1, Some("ana"), None);

    assert_eq!(disconnected(&mut ana).await, Disconnect::Unreachable);
}

#[tokio::test]
async fn asks_about_a_certificate_that_no_authority_signed() {
    let server = start_server_with(Tls::Own).await;
    let fingerprint = server.fingerprint.expect("a server with TLS has one");
    assert!(server.owner_link.starts_with("https://localhost:"));

    let mut ana = events(&server.owner_link, 1, Some("ana"), None);

    assert_eq!(
        disconnected(&mut ana).await,
        Disconnect::UnknownCertificate(fingerprint)
    );
}

#[tokio::test]
async fn signs_in_with_tls_when_the_user_accepted_the_certificate() {
    let server = start_server_with(Tls::Own).await;
    let fingerprint = server.fingerprint.expect("a server with TLS has one");
    let trust = Trust {
        key: None,
        certificate: Some(fingerprint),
    };

    let mut ana = events_with(&server.owner_link, 1, Some("ana"), trust);
    let link = signed_in(&mut ana).await;

    assert_eq!(link.certificate(), Some(fingerprint));
    assert_eq!(link.role(), Role::Owner);
    made_general(&link, &mut ana).await;
    assert!(link.send(to_general(None, "Hello")));
    assert!(matches!(
        received(&mut ana).await,
        ServerMessage::Message { text, .. } if text == "Hello"
    ));
}

#[tokio::test]
async fn warns_about_a_certificate_that_is_not_the_accepted_one() {
    let server = start_server_with(Tls::Own).await;
    let fingerprint = server.fingerprint.expect("a server with TLS has one");
    let trust = Trust {
        key: None,
        certificate: Some(Fingerprint::from_bytes([7; 32])),
    };

    let mut ana = events_with(&server.owner_link, 1, Some("ana"), trust);

    assert_eq!(
        disconnected(&mut ana).await,
        Disconnect::ChangedCertificate(fingerprint)
    );
}

#[tokio::test]
async fn has_no_certificate_to_remember_without_tls() {
    let server = start_server().await;

    let (_ana, link) = owner(&server).await;

    assert_eq!(server.fingerprint, None);
    assert_eq!(link.certificate(), None);
}

#[tokio::test]
async fn does_not_connect_when_only_one_side_uses_tls() {
    let with_tls = start_server_with(Tls::Own).await;
    let without_tls = start_server().await;
    let plain = format!("http://localhost:{}", with_tls.port);
    let secure = format!("https://localhost:{}", without_tls.port);

    let mut to_tls = events(&plain, 1, Some("ana"), None);
    let mut to_plain = events(&secure, 1, Some("ana"), None);

    assert_eq!(disconnected(&mut to_tls).await, Disconnect::Unreachable);
    assert_eq!(disconnected(&mut to_plain).await, Disconnect::Unreachable);
}

#[tokio::test]
async fn tells_that_the_server_refused_the_hello_message() {
    let address = fake_server(|mut socket, _address, _client| async move {
        say(
            &mut socket,
            &ServerMessage::error(None, ErrorCode::WrongAddress),
        )
        .await;
        hold(socket).await;
    })
    .await;

    let mut ana = events(&address, 1, Some("ana"), None);

    assert_eq!(
        disconnected(&mut ana).await,
        Disconnect::Refused(ErrorCode::WrongAddress)
    );
}

#[tokio::test]
async fn refuses_an_answer_that_is_not_a_step_of_the_sign_in() {
    let before_the_challenge = fake_server(|mut socket, _address, _client| async move {
        say(&mut socket, &welcome()).await;
        hold(socket).await;
    })
    .await;
    let not_a_message = fake_server(|mut socket, _address, _client| async move {
        say_line(&mut socket, "this is not a message").await;
        hold(socket).await;
    })
    .await;
    let after_the_challenge = fake_server(|mut socket, address, client| async move {
        let challenge = challenge(&server_key(), &address, &client, &[]);
        say(&mut socket, &challenge).await;
        socket.next().await;
        say(&mut socket, &challenge).await;
        hold(socket).await;
    })
    .await;

    for address in [before_the_challenge, not_a_message, after_the_challenge] {
        let mut ana = events(&address, 1, Some("ana"), None);

        assert_eq!(disconnected(&mut ana).await, Disconnect::Protocol);
    }
}

#[tokio::test]
async fn tells_that_the_server_closed_the_connection_before_the_welcome() {
    let before_the_challenge = fake_server(|mut socket, _address, _client| async move {
        let _ = socket.close(None).await;
    })
    .await;
    let after_the_challenge = fake_server(|mut socket, address, client| async move {
        say(
            &mut socket,
            &challenge(&server_key(), &address, &client, &[]),
        )
        .await;
        let _ = socket.close(None).await;
    })
    .await;

    for address in [before_the_challenge, after_the_challenge] {
        let mut ana = events(&address, 1, Some("ana"), None);

        assert_eq!(disconnected(&mut ana).await, Disconnect::Closed);
    }
}

async fn connects_and_then_stops_for_a_protocol_error(address: &str) {
    let mut ana = events(address, 1, None, None);

    let _link = connected(&mut ana).await;
    assert_eq!(disconnected(&mut ana).await, Disconnect::Protocol);
}

#[tokio::test]
async fn refuses_a_line_from_the_server_that_is_not_a_message() {
    let address = fake_server(|socket, address, client| async move {
        let mut socket = welcomed(socket, &address, &client).await;
        say_line(&mut socket, r#"{"type":"dance"}"#).await;
        hold(socket).await;
    })
    .await;

    connects_and_then_stops_for_a_protocol_error(&address).await;
}

#[tokio::test]
async fn refuses_data_from_the_server_that_is_not_text() {
    let address = fake_server(|socket, address, client| async move {
        let mut socket = welcomed(socket, &address, &client).await;
        socket
            .send(Message::binary(vec![1, 2, 3]))
            .await
            .expect("the socket is open");
        hold(socket).await;
    })
    .await;

    connects_and_then_stops_for_a_protocol_error(&address).await;
}

#[tokio::test]
async fn tells_that_the_server_stopped() {
    let server = start_server().await;
    let (mut ana, _link) = owner(&server).await;

    server.stop.send(()).expect("the server runs");

    assert_eq!(disconnected(&mut ana).await, Disconnect::Closed);
}

#[tokio::test]
async fn a_link_cannot_send_after_the_connection_stops() {
    let server = start_server().await;
    let (mut ana, link) = owner(&server).await;
    server.stop.send(()).expect("the server runs");
    assert_eq!(disconnected(&mut ana).await, Disconnect::Closed);
    drop(ana);

    assert!(!link.send(to_general(None, "Hello")));
}

#[tokio::test]
async fn tells_each_line_before_the_event_that_it_causes() {
    let server = start_server().await;
    let before = Timestamp::now();
    let mut ana = events_with_lines(&server.owner_link, 1, Some("ana"));

    let sign_in = [
        (Direction::Sent, "hello"),
        (Direction::Received, "challenge"),
        (Direction::Sent, "identify"),
        (Direction::Received, "error"),
        (Direction::Sent, "register"),
        (Direction::Received, "welcome"),
    ];
    for (direction, kind) in sign_in {
        let line = line_of(&mut ana, direction, kind).await;
        assert!(line.time.unix_millis() >= before.unix_millis());
        assert!(line.time.unix_millis() <= Timestamp::now().unix_millis());
    }
    let link = connected(&mut ana).await;
    line_of(&mut ana, Direction::Received, "ready").await;
    assert_eq!(received(&mut ana).await, ServerMessage::Ready {});

    assert!(link.send(create_general()));
    line_of(&mut ana, Direction::Sent, "channel-create").await;
    line_of(&mut ana, Direction::Received, "channel").await;
    assert!(matches!(
        received(&mut ana).await,
        ServerMessage::Channel { .. }
    ));

    let hello = to_general(Some(7), "Hello");
    assert!(link.send(hello.clone()));
    let sent = line_of(&mut ana, Direction::Sent, "send").await;
    assert_eq!(ClientMessage::decode(&sent.text).ok(), Some(hello));
    let echo = line_of(&mut ana, Direction::Received, "message").await;
    let message = received(&mut ana).await;
    assert_eq!(ServerMessage::decode(&echo.text).ok(), Some(message));

    let _ = server.stop.send(());
}

#[tokio::test]
async fn sends_a_line_that_the_user_typed() {
    let server = start_server().await;
    let mut ana = events_with_lines(&server.owner_link, 1, Some("ana"));
    let link = loop {
        if let Some(Event::Connected(link)) = next(&mut ana).await {
            break link;
        }
    };
    line_of(&mut ana, Direction::Received, "ready").await;
    received(&mut ana).await;

    let create = r#"{ "type": "channel-create", "channel": "general", "kind": "public" }"#;
    assert!(link.send_line(create.to_owned()));
    assert_eq!(line(&mut ana).await.text, create);
    line_of(&mut ana, Direction::Received, "channel").await;
    received(&mut ana).await;

    let typed = r#"{ "type": "send", "channel": "general", "text": "typed" }"#;
    assert!(link.send_line(typed.to_owned()));
    let sent = line(&mut ana).await;
    assert_eq!(sent.direction, Direction::Sent);
    assert_eq!(sent.text, typed);
    line_of(&mut ana, Direction::Received, "message").await;
    assert!(matches!(
        received(&mut ana).await,
        ServerMessage::Message { text, .. } if text == "typed"
    ));

    assert!(link.send_line("not a message".to_owned()));
    assert_eq!(line(&mut ana).await.text, "not a message");
    let refused = line_of(&mut ana, Direction::Received, "error").await;
    assert!(refused.text.contains("bad-message"), "{}", refused.text);
    assert!(matches!(
        received(&mut ana).await,
        ServerMessage::Error {
            code: ErrorCode::BadMessage,
            ..
        }
    ));

    let _ = server.stop.send(());
}

#[tokio::test]
async fn tells_the_last_line_before_the_end_of_the_connection() {
    let server = start_server().await;
    let address = format!("localhost:{}", server.port);
    let mut ana = events_with_lines(&address, 1, Some("ana"));

    line_of(&mut ana, Direction::Sent, "hello").await;
    line_of(&mut ana, Direction::Received, "challenge").await;
    line_of(&mut ana, Direction::Sent, "identify").await;
    let refused = line_of(&mut ana, Direction::Received, "error").await;
    assert!(refused.text.contains("unknown-account"), "{}", refused.text);
    assert_eq!(disconnected(&mut ana).await, Disconnect::InvitationRequired);

    let _ = server.stop.send(());
}

#[tokio::test]
async fn tells_no_lines_when_the_caller_does_not_ask_for_them() {
    let server = start_server().await;
    let mut ana = events(&server.owner_link, 1, Some("ana"), None);

    let link = signed_in(&mut ana).await;
    made_general(&link, &mut ana).await;
    assert!(link.send_line(r#"{"type":"send","channel":"general","text":"typed"}"#.to_owned()));
    assert!(matches!(
        received(&mut ana).await,
        ServerMessage::Message { text, .. } if text == "typed"
    ));

    let _ = server.stop.send(());
}

#[test]
fn a_link_gives_messages_and_typed_lines_to_its_queue() {
    let (link, mut queue) = Link::channel(name("ana"), Role::Member, server_key().public(), 2);
    let hello = ClientMessage::Send {
        reference: None,
        channel: "lobby".to_owned(),
        text: "Hello".to_owned(),
    };

    assert!(link.send(hello.clone()));
    assert!(link.send_line("{}".to_owned()));
    assert!(!link.send_line("{}".to_owned()));

    assert_eq!(queue.try_recv(), Ok(Outgoing::Message(hello)));
    assert_eq!(queue.try_recv(), Ok(Outgoing::Line("{}".to_owned())));
    assert!(queue.try_recv().is_err());
}

#[test]
fn a_link_refuses_a_line_that_is_longer_than_the_protocol_permits() {
    let (link, mut queue) = Link::channel(name("ana"), Role::Member, server_key().public(), 4);

    assert!(link.send_line("x".repeat(MAX_LINE_BYTES)));
    assert!(!link.send_line("x".repeat(MAX_LINE_BYTES + 1)));

    assert!(queue.try_recv().is_ok());
    assert!(queue.try_recv().is_err());
}
