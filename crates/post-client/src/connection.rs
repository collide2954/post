use std::time::Duration;

use futures_util::{SinkExt, Stream, StreamExt, stream};
use post_proto::{ClientMessage, ErrorCode, MAX_LINE_BYTES, Name, PROTOCOL_VERSION, ServerMessage};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::{WebSocketStream, client_async_with_config};

use crate::ServerAddress;

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
}

#[derive(Debug, Clone)]
pub struct Link {
    name: Name,
    queue: mpsc::Sender<ClientMessage>,
}

impl Link {
    #[must_use]
    pub fn name(&self) -> &Name {
        &self.name
    }

    #[must_use]
    pub fn send(&self, message: ClientMessage) -> bool {
        self.queue.try_send(message).is_ok()
    }
}

enum State {
    Start { address: ServerAddress, name: Name },
    Open(Box<Open>),
    End,
}

struct Open {
    socket: Socket,
    queue: mpsc::Receiver<ClientMessage>,
}

pub fn connect(address: ServerAddress, name: Name) -> impl Stream<Item = Event> + Send + 'static {
    stream::unfold(State::Start { address, name }, step)
}

async fn step(state: State) -> Option<(Event, State)> {
    match state {
        State::Start { address, name } => Some(match open(&address, &name).await {
            Ok((socket, name)) => {
                let (sender, queue) = mpsc::channel(QUEUE_MESSAGES);
                (
                    Event::Connected(Link {
                        name,
                        queue: sender,
                    }),
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
    let hello = ClientMessage::Hello {
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
