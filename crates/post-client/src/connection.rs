use std::time::Duration;

use futures_util::{SinkExt, Stream, StreamExt, stream};
use post_crypto::{Exchange, Fingerprint, Seed};
use post_proto::{
    ClientMessage, ErrorCode, Host, Key, MAX_LINE_BYTES, Name, PROTOCOL_VERSION, Role,
    ServerMessage,
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
    Disconnected(Disconnect),
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
    queue: mpsc::Sender<ClientMessage>,
}

impl Link {
    #[must_use]
    pub fn channel(
        name: Name,
        role: Role,
        server: Key,
        capacity: usize,
    ) -> (Self, mpsc::Receiver<ClientMessage>) {
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
        self.queue.try_send(message).is_ok()
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
}

struct Open {
    socket: Socket,
    queue: mpsc::Receiver<ClientMessage>,
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
    };
    stream::unfold(State::Start(Box::new(start)), step)
}

async fn step(state: State) -> Option<(Event, State)> {
    match state {
        State::Start(start) => Some(match start.open().await {
            Ok((socket, link, queue)) => (
                Event::Connected(link),
                State::Open(Box::new(Open { socket, queue })),
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
    async fn open(&self) -> Result<(Socket, Link, mpsc::Receiver<ClientMessage>), Disconnect> {
        let address = self.destination.address().to_string();
        let (mut socket, certificate) = dial(&self.destination, self.trust.certificate).await?;
        let client = post_crypto::nonce();
        let hello = ClientMessage::Hello {
            reference: None,
            protocol: PROTOCOL_VERSION,
            address: address.clone(),
            nonce: client,
            supports: Vec::new(),
        };
        write(&mut socket, &hello).await?;
        let ServerMessage::Challenge {
            key: server,
            nonce,
            proof,
            requires,
            ..
        } = answer(&mut socket).await?
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
        write(&mut socket, &identify).await?;
        let welcome = match answer(&mut socket).await {
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
                write(&mut socket, &register).await?;
                answer(&mut socket).await?
            }
            other => other?,
        };
        let ServerMessage::Welcome { name, role, .. } = welcome else {
            return Err(Disconnect::Protocol);
        };
        let (link, queue) = Link::channel(name, role, server, QUEUE_MESSAGES);
        Ok((socket, link.with_certificate(certificate), queue))
    }
}

async fn answer(socket: &mut Socket) -> Result<ServerMessage, Disconnect> {
    match timeout(ANSWER_TIMEOUT, read(socket)).await {
        Ok(Ok(ServerMessage::Error { code, .. })) => Err(Disconnect::Refused(code)),
        Ok(answer) => answer,
        Err(_) => Err(Disconnect::Unreachable),
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

async fn read(socket: &mut Socket) -> Result<ServerMessage, Disconnect> {
    loop {
        match socket.next().await {
            Some(Ok(Message::Text(line))) => {
                return ServerMessage::decode(line.as_str()).map_err(|_| Disconnect::Protocol);
            }
            Some(Ok(Message::Binary(_))) => return Err(Disconnect::Protocol),
            Some(Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_))) => {}
            Some(Ok(Message::Close(_)) | Err(_)) | None => return Err(Disconnect::Closed),
        }
    }
}

async fn write(socket: &mut Socket, message: &ClientMessage) -> Result<(), Disconnect> {
    match timeout(ANSWER_TIMEOUT, socket.send(Message::text(message.encode()))).await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(_)) | Err(_) => Err(Disconnect::Closed),
    }
}

impl Open {
    async fn next(&mut self) -> Result<ServerMessage, Disconnect> {
        loop {
            tokio::select! {
                message = read(&mut self.socket) => return message,
                command = self.queue.recv() => {
                    let Some(message) = command else {
                        let _ = timeout(ANSWER_TIMEOUT, self.socket.close(None)).await;
                        return Err(Disconnect::Closed);
                    };
                    write(&mut self.socket, &message).await?;
                }
            }
        }
    }
}
