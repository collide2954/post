use std::time::Duration;

use futures_util::{SinkExt, Stream, StreamExt, stream};
use post_crypto::{Exchange, Seed};
use post_proto::{
    Address, ClientMessage, ErrorCode, Key, MAX_LINE_BYTES, Name, PROTOCOL_VERSION, Role,
    ServerMessage,
};
use tokio::net::{TcpStream, lookup_host};
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::{WebSocketStream, client_async_with_config};

use crate::ServerAddress;
use crate::destination::{Destination, Security};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const ANSWER_TIMEOUT: Duration = Duration::from_secs(10);
const QUEUE_MESSAGES: usize = 64;

type Socket = WebSocketStream<TcpStream>;

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
    NoTls,
}

pub struct Identity {
    pub seed: Seed,
    pub name: Option<Name>,
}

#[derive(Debug, Clone)]
pub struct Link {
    name: Name,
    role: Role,
    server: Option<Key>,
    queue: mpsc::Sender<ClientMessage>,
}

impl Link {
    #[must_use]
    pub fn channel(name: Name, capacity: usize) -> (Self, mpsc::Receiver<ClientMessage>) {
        let (queue, messages) = mpsc::channel(capacity);
        let link = Self {
            name,
            role: Role::Member,
            server: None,
            queue,
        };
        (link, messages)
    }

    #[must_use]
    pub fn of_account(mut self, role: Role, server: Key) -> Self {
        self.role = role;
        self.server = Some(server);
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
    pub fn server(&self) -> Option<&Key> {
        self.server.as_ref()
    }

    #[must_use]
    pub fn send(&self, message: ClientMessage) -> bool {
        self.queue.try_send(message).is_ok()
    }
}

enum State {
    Start { address: ServerAddress, name: Name },
    SignIn(Box<SignIn>),
    Open(Box<Open>),
    End,
}

struct SignIn {
    destination: Destination,
    identity: Identity,
    known: Option<Key>,
}

struct Account {
    name: Name,
    role: Role,
    server: Key,
}

struct Open {
    socket: Socket,
    queue: mpsc::Receiver<ClientMessage>,
}

pub fn connect(address: ServerAddress, name: Name) -> impl Stream<Item = Event> + Send + 'static {
    stream::unfold(State::Start { address, name }, step)
}

pub fn sign_in(
    destination: Destination,
    identity: Identity,
    known: Option<Key>,
) -> impl Stream<Item = Event> + Send + 'static {
    let start = SignIn {
        destination,
        identity,
        known,
    };
    stream::unfold(State::SignIn(Box::new(start)), step)
}

async fn step(state: State) -> Option<(Event, State)> {
    match state {
        State::SignIn(start) => Some(match start.open().await {
            Ok((socket, account)) => {
                let (link, queue) = Link::channel(account.name, QUEUE_MESSAGES);
                (
                    Event::Connected(link.of_account(account.role, account.server)),
                    State::Open(Box::new(Open { socket, queue })),
                )
            }
            Err(reason) => (Event::Disconnected(reason), State::End),
        }),
        State::Start { address, name } => Some(match open(&address, &name).await {
            Ok((socket, name)) => {
                let (link, queue) = Link::channel(name, QUEUE_MESSAGES);
                (
                    Event::Connected(link),
                    State::Open(Box::new(Open { socket, queue })),
                )
            }
            Err(reason) => (Event::Disconnected(reason), State::End),
        }),
        State::Open(mut open) => Some(match open.next().await {
            Ok(message) => (Event::Received(message), State::Open(open)),
            Err(reason) => (Event::Disconnected(reason), State::End),
        }),
        State::End => None,
    }
}

async fn open(address: &ServerAddress, name: &Name) -> Result<(Socket, Name), Disconnect> {
    let mut socket = dial(address).await.ok_or(Disconnect::Unreachable)?;
    let hello = ClientMessage::Guest {
        reference: None,
        protocol: PROTOCOL_VERSION,
        name: name.as_str().to_owned(),
    };
    write(&mut socket, &hello).await?;
    match timeout(ANSWER_TIMEOUT, read(&mut socket)).await {
        Ok(Ok(ServerMessage::Welcome { name, .. })) => Ok((socket, name)),
        Ok(Ok(ServerMessage::Error { code, .. })) => Err(Disconnect::Refused(code)),
        Ok(Ok(_)) => Err(Disconnect::Protocol),
        Ok(Err(reason)) => Err(reason),
        Err(_) => Err(Disconnect::Unreachable),
    }
}

impl SignIn {
    async fn open(&self) -> Result<(Socket, Account), Disconnect> {
        if self.destination.security() == Security::Tls {
            return Err(Disconnect::NoTls);
        }
        let address = self.destination.address().to_string();
        let mut socket = dial_address(self.destination.address())
            .await
            .ok_or(Disconnect::Unreachable)?;
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
        if self.known.as_ref().is_some_and(|known| *known != server) {
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
        Ok((socket, Account { name, role, server }))
    }
}

async fn answer(socket: &mut Socket) -> Result<ServerMessage, Disconnect> {
    match timeout(ANSWER_TIMEOUT, read(socket)).await {
        Ok(Ok(ServerMessage::Error { code, .. })) => Err(Disconnect::Refused(code)),
        Ok(answer) => answer,
        Err(_) => Err(Disconnect::Unreachable),
    }
}

async fn dial_address(address: &Address) -> Option<Socket> {
    let config = WebSocketConfig::default()
        .max_message_size(Some(MAX_LINE_BYTES))
        .max_frame_size(Some(MAX_LINE_BYTES));
    let candidates = lookup_host(address.to_string()).await.ok()?;
    for candidate in candidates {
        let attempt = async {
            let stream = TcpStream::connect(candidate).await.ok()?;
            let url = format!("ws://{address}/");
            let (socket, _) = client_async_with_config(url, stream, Some(config))
                .await
                .ok()?;
            Some(socket)
        };
        if let Ok(Some(socket)) = timeout(CONNECT_TIMEOUT, attempt).await {
            return Some(socket);
        }
    }
    None
}

async fn dial(address: &ServerAddress) -> Option<Socket> {
    let config = WebSocketConfig::default()
        .max_message_size(Some(MAX_LINE_BYTES))
        .max_frame_size(Some(MAX_LINE_BYTES));
    for candidate in address.candidates() {
        let attempt = async {
            let stream = TcpStream::connect(candidate).await.ok()?;
            let url = format!("ws://{address}/");
            let (socket, _) = client_async_with_config(url, stream, Some(config))
                .await
                .ok()?;
            Some(socket)
        };
        if let Ok(Some(socket)) = timeout(CONNECT_TIMEOUT, attempt).await {
            return Some(socket);
        }
    }
    None
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
