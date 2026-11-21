use std::future::Future;
use std::net::Ipv4Addr;
use std::pin::Pin;
use std::time::Duration;

use futures_util::{SinkExt, Stream, StreamExt};
use post_client::{Disconnect, Event, Link, ServerAddress, connect};
use post_proto::{ClientMessage, ErrorCode, Name, ServerMessage};
use post_server::Server;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{WebSocketStream, accept_async};

const PATIENCE: Duration = Duration::from_secs(20);

type Events = Pin<Box<dyn Stream<Item = Event> + Send>>;

fn name(text: &str) -> Name {
    Name::parse(text).expect(text)
}

fn address(text: &str) -> ServerAddress {
    ServerAddress::parse(text).expect(text)
}

async fn start_server() -> (u16, oneshot::Sender<()>) {
    let server = Server::bind(0).await.expect("the server can bind");
    let port = server.addresses()[0].port();
    let (stop, stopped) = oneshot::channel::<()>();
    tokio::spawn(server.run(async {
        let _ = stopped.await;
    }));
    (port, stop)
}

async fn fake_server<F, Fut>(script: F) -> ServerAddress
where
    F: FnOnce(WebSocketStream<TcpStream>) -> Fut + Send + 'static,
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
        assert!(matches!(
            ClientMessage::decode(hello.to_text().expect("text")),
            Ok(ClientMessage::Guest { protocol: 0, .. })
        ));
        script(socket).await;
    });
    address(&format!("127.0.0.1:{port}"))
}

async fn say(socket: &mut WebSocketStream<TcpStream>, line: &str) {
    socket
        .send(Message::text(line))
        .await
        .expect("the socket is open");
}

async fn hold(mut socket: WebSocketStream<TcpStream>) {
    while let Some(Ok(_)) = socket.next().await {}
}

fn events(address: &ServerAddress, who: &str) -> Events {
    Box::pin(connect(address.clone(), name(who)))
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

async fn join(address: &ServerAddress, who: &str) -> (Events, Link) {
    let mut events = events(address, who);
    let Some(Event::Connected(link)) = next(&mut events).await else {
        panic!("the server accepts {who}");
    };
    assert_eq!(link.name(), &name(who));
    assert!(matches!(
        received(&mut events).await,
        ServerMessage::Join { name: joined, .. } if joined == name(who)
    ));
    (events, link)
}

async fn disconnected(events: &mut Events) -> Disconnect {
    let Some(Event::Disconnected(reason)) = next(events).await else {
        panic!("the connection stops");
    };
    assert!(next(events).await.is_none(), "the stream stops");
    reason
}

#[tokio::test]
async fn two_users_exchange_text_through_the_server() {
    let (port, _stop) = start_server().await;
    let (mut ana, ana_link) = join(&address(&format!("localhost:{port}")), "ana").await;
    let (mut ben, ben_link) = join(&address(&format!("127.0.0.1:{port}")), "ben").await;
    assert!(matches!(
        received(&mut ana).await,
        ServerMessage::Join { name: joined, .. } if joined == name("ben")
    ));

    assert!(ana_link.send(ClientMessage::Send {
        reference: Some(1),
        channel: "lobby".to_owned(),
        text: "Hello".to_owned(),
    }));

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
    assert!(matches!(
        received(&mut ana).await,
        ServerMessage::Leave { name: left, .. } if left == name("ben")
    ));
}

#[tokio::test]
async fn gives_the_error_of_the_server_as_a_message() {
    let (port, _stop) = start_server().await;
    let (mut ana, link) = join(&address(&format!("localhost:{port}")), "ana").await;

    assert!(link.send(ClientMessage::Send {
        reference: Some(3),
        channel: "garden".to_owned(),
        text: "Hello".to_owned(),
    }));

    assert_eq!(
        received(&mut ana).await,
        ServerMessage::error(Some(3), ErrorCode::NoSuchChannel)
    );
}

#[tokio::test]
async fn tells_that_the_server_refused_the_name() {
    let (port, _stop) = start_server().await;
    let address = address(&format!("localhost:{port}"));
    let (_ana, _link) = join(&address, "ana").await;

    let mut second = events(&address, "ana");

    assert_eq!(
        disconnected(&mut second).await,
        Disconnect::Refused(ErrorCode::NameTaken)
    );
}

#[tokio::test]
async fn tells_that_no_server_is_at_the_address() {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("a free port");
    let port = listener.local_addr().expect("a local address").port();
    drop(listener);

    let mut events = events(&address(&format!("localhost:{port}")), "ana");

    assert_eq!(disconnected(&mut events).await, Disconnect::Unreachable);
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

    let mut events = events(&address(&format!("127.0.0.1:{port}")), "ana");

    assert_eq!(disconnected(&mut events).await, Disconnect::Unreachable);
}

#[tokio::test]
async fn tells_that_the_server_stopped() {
    let (port, stop) = start_server().await;
    let (mut ana, _link) = join(&address(&format!("localhost:{port}")), "ana").await;

    stop.send(()).expect("the server runs");

    assert_eq!(disconnected(&mut ana).await, Disconnect::Closed);
}

#[tokio::test]
async fn tells_that_the_server_closed_the_connection_before_the_welcome() {
    let address = fake_server(|mut socket| async move {
        let _ = socket.close(None).await;
    })
    .await;

    let mut events = events(&address, "ana");

    assert_eq!(disconnected(&mut events).await, Disconnect::Closed);
}

#[tokio::test]
async fn refuses_a_first_answer_that_is_not_a_welcome_or_an_error() {
    for line in [
        "this is not a message",
        r#"{"type":"join","channel":"lobby","name":"ana","time":"2026-11-21T14:02:00.000Z"}"#,
    ] {
        let address = fake_server(move |mut socket| async move {
            say(&mut socket, line).await;
            hold(socket).await;
        })
        .await;

        let mut events = events(&address, "ana");

        assert_eq!(
            disconnected(&mut events).await,
            Disconnect::Protocol,
            "{line}"
        );
    }
}

const WELCOME: &str = r#"{"type":"welcome","name":"ana","role":"member"}"#;

async fn connects_and_then_stops_for_a_protocol_error(address: &ServerAddress) {
    let mut events = events(address, "ana");

    let Some(Event::Connected(_link)) = next(&mut events).await else {
        panic!("the server accepts the user");
    };
    assert_eq!(disconnected(&mut events).await, Disconnect::Protocol);
}

#[tokio::test]
async fn refuses_a_line_from_the_server_that_is_not_a_message() {
    let address = fake_server(|mut socket| async move {
        say(&mut socket, WELCOME).await;
        say(&mut socket, r#"{"type":"dance"}"#).await;
        hold(socket).await;
    })
    .await;

    connects_and_then_stops_for_a_protocol_error(&address).await;
}

#[tokio::test]
async fn refuses_data_from_the_server_that_is_not_text() {
    let address = fake_server(|mut socket| async move {
        say(&mut socket, WELCOME).await;
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
async fn uses_the_name_that_the_server_gives() {
    let address = fake_server(|mut socket| async move {
        say(
            &mut socket,
            r#"{"type":"welcome","name":"ana-2","role":"member"}"#,
        )
        .await;
        hold(socket).await;
    })
    .await;

    let mut events = events(&address, "ana");

    let Some(Event::Connected(link)) = next(&mut events).await else {
        panic!("the server accepts the user");
    };
    assert_eq!(link.name(), &name("ana-2"));
}

#[tokio::test]
async fn a_link_cannot_send_after_the_connection_stops() {
    let (port, stop) = start_server().await;
    let (mut ana, link) = join(&address(&format!("localhost:{port}")), "ana").await;
    stop.send(()).expect("the server runs");
    assert_eq!(disconnected(&mut ana).await, Disconnect::Closed);
    drop(ana);

    assert!(!link.send(ClientMessage::Send {
        reference: None,
        channel: "lobby".to_owned(),
        text: "Hello".to_owned(),
    }));
}
