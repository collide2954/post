use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use futures_util::{SinkExt, Stream, StreamExt, stream};
use post_crypto::{Exchange, Fingerprint, Seed};
use post_proto::{
    ClientMessage, ErrorCode, Host, Key, MAX_LINE_BYTES, Name, PROTOCOL_VERSION, Role,
    ServerMessage, Timestamp,
};
use tokio::net::{TcpStream, lookup_host};
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::{WebSocketStream, client_async_with_config};

use crate::destination::{Destination, Security};
use crate::tls::{self, Transport};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const ANSWER_TIMEOUT: Duration = Duration::from_secs(10);
const QUEUE_MESSAGES: usize = 64;

type Socket = WebSocketStream<Transport>;

#[derive(Debug, Clone)]
pub enum Event {
    Connected(Link),
    Received(ServerMessage),
    Line(Line),
    Disconnected(Disconnect),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub time: Timestamp,
    pub direction: Direction,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Sent,
    Received,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outgoing {
    Message(ClientMessage),
    Line(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disconnect {
    Unreachable,
    Refused(ErrorCode),
    Protocol,
    Closed,
    FalseServer,
    DifferentServer,
    MissingParts,
    NameRequired,
    InvitationRequired,
    UnknownCertificate(Fingerprint),
    ChangedCertificate(Fingerprint),
}

pub struct Identity {
    pub seed: Seed,
    pub name: Option<Name>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Trust {
    pub key: Option<Key>,
    pub certificate: Option<Fingerprint>,
}

#[derive(Debug, Clone)]
pub struct Link {
    name: Name,
    role: Role,
    server: Key,
    certificate: Option<Fingerprint>,
    queue: mpsc::Sender<Outgoing>,
}

impl Link {
    #[must_use]
    pub fn channel(
        name: Name,
        role: Role,
        server: Key,
        capacity: usize,
    ) -> (Self, mpsc::Receiver<Outgoing>) {
        let (queue, messages) = mpsc::channel(capacity);
        let link = Self {
            name,
            role,
            server,
            certificate: None,
            queue,
        };
        (link, messages)
    }

    #[must_use]
    pub fn with_certificate(mut self, certificate: Option<Fingerprint>) -> Self {
        self.certificate = certificate;
        self
    }

    #[must_use]
    pub fn name(&self) -> &Name {
        &self.name
    }

    #[must_use]
    pub fn role(&self) -> Role {
        self.role
    }

    #[must_use]
    pub fn server(&self) -> &Key {
        &self.server
    }

    #[must_use]
    pub fn certificate(&self) -> Option<Fingerprint> {
        self.certificate
    }

    #[must_use]
    pub fn send(&self, message: ClientMessage) -> bool {
        self.queue.try_send(Outgoing::Message(message)).is_ok()
    }

    #[must_use]
    pub fn send_line(&self, line: String) -> bool {
        line.len() <= MAX_LINE_BYTES && self.queue.try_send(Outgoing::Line(line)).is_ok()
    }
}

enum State {
    Start(Box<SignIn>),
    Open(Box<Open>),
    End,
}

struct SignIn {
    destination: Destination,
    identity: Identity,
    trust: Trust,
    lines: Option<mpsc::UnboundedSender<Line>>,
}

struct Open {
    wire: Wire,
    queue: mpsc::Receiver<Outgoing>,
}

struct Wire {
    socket: Socket,
    lines: Option<mpsc::UnboundedSender<Line>>,
}

type Step = Pin<Box<dyn Future<Output = Option<(Event, State)>> + Send>>;

struct Watched {
    lines: mpsc::UnboundedReceiver<Line>,
    running: Option<Step>,
    waiting: Option<Event>,
}

pub fn sign_in(
    destination: Destination,
    identity: Identity,
    trust: Trust,
) -> impl Stream<Item = Event> + Send + 'static {
    let start = SignIn {
        destination,
        identity,
        trust,
        lines: None,
    };
    stream::unfold(State::Start(Box::new(start)), step)
}

pub fn sign_in_with_lines(
    destination: Destination,
    identity: Identity,
    trust: Trust,
) -> impl Stream<Item = Event> + Send + 'static {
    let (sender, lines) = mpsc::unbounded_channel();
    let start = SignIn {
        destination,
        identity,
        trust,
        lines: Some(sender),
    };
    let watched = Watched {
        lines,
        running: Some(Box::pin(step(State::Start(Box::new(start))))),
        waiting: None,
    };
    stream::unfold(watched, Watched::next)
}

impl Watched {
    async fn next(mut self) -> Option<(Event, Self)> {
        loop {
            if let Ok(line) = self.lines.try_recv() {
                return Some((Event::Line(line), self));
            }
            if let Some(event) = self.waiting.take() {
                return Some((event, self));
            }
            let running = self.running.as_mut()?;
            tokio::select! {
                biased;
                Some(line) = self.lines.recv() => return Some((Event::Line(line), self)),
                outcome = running => {
                    self.running = None;
                    if let Some((event, state)) = outcome {
                        self.waiting = Some(event);
                        self.running = Some(Box::pin(step(state)));
                    }
                }
            }
        }
    }
}

async fn step(state: State) -> Option<(Event, State)> {
    match state {
        State::Start(start) => Some(match start.open().await {
            Ok((wire, link, queue)) => (
                Event::Connected(link),
                State::Open(Box::new(Open { wire, queue })),
            ),
            Err(reason) => (Event::Disconnected(reason), State::End),
        }),
        State::Open(mut open) => Some(match open.next().await {
            Ok(message) => (Event::Received(message), State::Open(open)),
            Err(reason) => (Event::Disconnected(reason), State::End),
        }),
        State::End => None,
    }
}

impl SignIn {
    async fn open(&self) -> Result<(Wire, Link, mpsc::Receiver<Outgoing>), Disconnect> {
        let address = self.destination.address().to_string();
        let (socket, certificate) = dial(&self.destination, self.trust.certificate).await?;
        let mut wire = Wire {
            socket,
            lines: self.lines.clone(),
        };
        let client = post_crypto::nonce();
        let hello = ClientMessage::Hello {
            reference: None,
            protocol: PROTOCOL_VERSION,
            address: address.clone(),
            nonce: client,
            supports: Vec::new(),
        };
        wire.write(&hello).await?;
        let ServerMessage::Challenge {
            key: server,
            nonce,
            proof,
            requires,
            ..
        } = wire.answer().await?
        else {
            return Err(Disconnect::Protocol);
        };
        let exchange = Exchange {
            address: &address,
            client: &client,
            server: &nonce,
        };
        if !exchange.server_proof_is_valid(&server, &proof) {
            return Err(Disconnect::FalseServer);
        }
        if self.trust.key.is_some_and(|known| known != server) {
            return Err(Disconnect::DifferentServer);
        }
        if !requires.is_empty() {
            return Err(Disconnect::MissingParts);
        }

        let account = self.identity.seed.account_key(&server);
        let key = account.public();
        let proof = account.prove(&server, &exchange);
        let invitation = self.destination.invitation();
        let identify = ClientMessage::Identify {
            reference: None,
            key,
            proof,
            invitation: invitation.cloned(),
        };
        wire.write(&identify).await?;
        let welcome = match wire.answer().await {
            Err(Disconnect::Refused(ErrorCode::UnknownAccount)) => {
                let invitation = invitation.ok_or(Disconnect::InvitationRequired)?;
                let name = self
                    .identity
                    .name
                    .as_ref()
                    .ok_or(Disconnect::NameRequired)?;
                let register = ClientMessage::Register {
                    reference: None,
                    key,
                    proof,
                    name: name.as_str().to_owned(),
                    invitation: invitation.clone(),
                };
                wire.write(&register).await?;
                wire.answer().await?
            }
            other => other?,
        };
        let ServerMessage::Welcome { name, role, .. } = welcome else {
            return Err(Disconnect::Protocol);
        };
        let (link, queue) = Link::channel(name, role, server, QUEUE_MESSAGES);
        Ok((wire, link.with_certificate(certificate), queue))
    }
}

async fn dial(
    destination: &Destination,
    accepted: Option<Fingerprint>,
) -> Result<(Socket, Option<Fingerprint>), Disconnect> {
    let config = WebSocketConfig::default()
        .max_message_size(Some(MAX_LINE_BYTES))
        .max_frame_size(Some(MAX_LINE_BYTES));
    let address = destination.address();
    let host = match address.host() {
        Host::Name(name) => name.clone(),
        Host::Ip(ip) => ip.to_string(),
    };
    let candidates = lookup_host(address.to_string())
        .await
        .map_err(|_| Disconnect::Unreachable)?;
    for candidate in candidates {
        let attempt = async {
            let stream = TcpStream::connect(candidate)
                .await
                .map_err(|_| Disconnect::Unreachable)?;
            let (stream, certificate) = match destination.security() {
                Security::Plain => (Transport::Plain(stream), None),
                Security::Tls => tls::secure(stream, &host, accepted).await?,
            };
            let (socket, _) = client_async_with_config(destination.url(), stream, Some(config))
                .await
                .map_err(|_| Disconnect::Unreachable)?;
            Ok((socket, certificate))
        };
        match timeout(CONNECT_TIMEOUT, attempt).await {
            Ok(Ok(connected)) => return Ok(connected),
            Ok(Err(Disconnect::Unreachable)) | Err(_) => {}
            Ok(Err(refused)) => return Err(refused),
        }
    }
    Err(Disconnect::Unreachable)
}

impl From<tls::Problem> for Disconnect {
    fn from(problem: tls::Problem) -> Self {
        match problem {
            tls::Problem::UnknownCertificate(fingerprint) => Self::UnknownCertificate(fingerprint),
            tls::Problem::ChangedCertificate(fingerprint) => Self::ChangedCertificate(fingerprint),
            tls::Problem::NoConnection => Self::Unreachable,
        }
    }
}

impl Wire {
    async fn read(&mut self) -> Result<ServerMessage, Disconnect> {
        loop {
            match self.socket.next().await {
                Some(Ok(Message::Text(line))) => {
                    self.record(Direction::Received, || line.as_str().to_owned());
                    return ServerMessage::decode(line.as_str()).map_err(|_| Disconnect::Protocol);
                }
                Some(Ok(Message::Binary(_))) => return Err(Disconnect::Protocol),
                Some(Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_))) => {}
                Some(Ok(Message::Close(_)) | Err(_)) | None => return Err(Disconnect::Closed),
            }
        }
    }

    async fn answer(&mut self) -> Result<ServerMessage, Disconnect> {
        match timeout(ANSWER_TIMEOUT, self.read()).await {
            Ok(Ok(ServerMessage::Error { code, .. })) => Err(Disconnect::Refused(code)),
            Ok(answer) => answer,
            Err(_) => Err(Disconnect::Unreachable),
        }
    }

    async fn write(&mut self, message: &ClientMessage) -> Result<(), Disconnect> {
        self.write_line(message.encode()).await
    }

    async fn write_line(&mut self, line: String) -> Result<(), Disconnect> {
        let copy = self.lines.as_ref().map(|_| line.clone());
        match timeout(ANSWER_TIMEOUT, self.socket.send(Message::text(line))).await {
            Ok(Ok(())) => {
                if let Some(copy) = copy {
                    self.record(Direction::Sent, || copy);
                }
                Ok(())
            }
            Ok(Err(_)) | Err(_) => Err(Disconnect::Closed),
        }
    }

    fn record(&self, direction: Direction, text: impl FnOnce() -> String) {
        if let Some(lines) = &self.lines {
            let _ = lines.send(Line {
                time: Timestamp::now(),
                direction,
                text: text(),
            });
        }
    }
}

impl Open {
    async fn next(&mut self) -> Result<ServerMessage, Disconnect> {
        loop {
            tokio::select! {
                message = self.wire.read() => return message,
                command = self.queue.recv() => match command {
                    Some(Outgoing::Message(message)) => self.wire.write(&message).await?,
                    Some(Outgoing::Line(line)) => self.wire.write_line(line).await?,
                    None => {
                        let _ = timeout(ANSWER_TIMEOUT, self.wire.socket.close(None)).await;
                        return Err(Disconnect::Closed);
                    }
                }
            }
        }
    }
}
