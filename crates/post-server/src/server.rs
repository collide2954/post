use std::io;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use axum::Router;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use axum::routing::any;
use post_proto::{Address, MAX_LINE_BYTES, Timestamp};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinSet;
use tokio::time::timeout;

use crate::hub::{Hub, Random, Settings};
use crate::sessions::Sessions;
use crate::store::Store;

const QUEUE_LINES: usize = 256;
const SEND_TIMEOUT: Duration = Duration::from_secs(10);

pub struct Server {
    listeners: Vec<TcpListener>,
    addresses: Vec<SocketAddr>,
    hub: Hub,
    owner_link: Option<String>,
}

struct Shared {
    sessions: Mutex<Sessions>,
    stopped: watch::Receiver<bool>,
}

impl Server {
    pub async fn bind(port: u16) -> io::Result<Self> {
        let ipv4 = TcpListener::bind((Ipv4Addr::LOCALHOST, port)).await?;
        let ipv4_address = ipv4.local_addr()?;
        let mut listeners = vec![ipv4];
        let mut addresses = vec![ipv4_address];

        if let Ok(ipv6) = TcpListener::bind((Ipv6Addr::LOCALHOST, ipv4_address.port())).await {
            addresses.push(ipv6.local_addr()?);
            listeners.push(ipv6);
        }

        let port = ipv4_address.port();
        let settings = Settings {
            addresses: vec![
                Address::localhost(port).to_string(),
                Address::from_ip(Ipv4Addr::LOCALHOST.into(), port).to_string(),
                Address::from_ip(Ipv6Addr::LOCALHOST.into(), port).to_string(),
            ],
            link: format!("http://{}", Address::localhost(port)),
            requires: Vec::new(),
        };
        let store = Store::in_memory().map_err(io::Error::other)?;
        let mut hub = Hub::new(store, settings, Random::default()).map_err(io::Error::other)?;
        let owner_link = hub.owner_link(Timestamp::now()).map_err(io::Error::other)?;

        Ok(Self {
            listeners,
            addresses,
            hub,
            owner_link,
        })
    }

    #[must_use]
    pub fn addresses(&self) -> &[SocketAddr] {
        &self.addresses
    }

    #[must_use]
    pub fn owner_link(&self) -> Option<&str> {
        self.owner_link.as_deref()
    }

    pub async fn run(self, stop: impl Future<Output = ()> + Send + 'static) -> io::Result<()> {
        let (stop_sender, stopped) = watch::channel(false);
        tokio::spawn(async move {
            stop.await;
            let _ = stop_sender.send(true);
        });

        let shared = Arc::new(Shared {
            sessions: Mutex::new(Sessions::new(self.hub, Timestamp::now)),
            stopped: stopped.clone(),
        });
        let router = Router::new().route("/", any(upgrade)).with_state(shared);

        let mut tasks = JoinSet::new();
        for listener in self.listeners {
            let router = router.clone();
            let stopped = stopped.clone();
            tasks.spawn(async move {
                axum::serve(listener, router)
                    .with_graceful_shutdown(wait_for_stop(stopped))
                    .await
            });
        }
        while let Some(result) = tasks.join_next().await {
            result.map_err(io::Error::other)??;
        }
        Ok(())
    }
}

impl Shared {
    fn sessions(&self) -> MutexGuard<'_, Sessions> {
        self.sessions.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

async fn wait_for_stop(mut stopped: watch::Receiver<bool>) {
    let _ = stopped.wait_for(|stopped| *stopped).await;
}

async fn upgrade(State(shared): State<Arc<Shared>>, upgrade: WebSocketUpgrade) -> Response {
    upgrade
        .max_message_size(MAX_LINE_BYTES)
        .on_upgrade(|socket| connection(socket, shared))
}

async fn connection(mut socket: WebSocket, shared: Arc<Shared>) {
    let (queue, mut lines) = mpsc::channel(QUEUE_LINES);
    let id = shared.sessions().connect(queue);
    let stopped = wait_for_stop(shared.stopped.clone());
    tokio::pin!(stopped);

    loop {
        tokio::select! {
            incoming = socket.recv() => match incoming {
                Some(Ok(Message::Text(line))) => shared.sessions().receive(id, line.as_str()),
                Some(Ok(Message::Binary(_))) => shared.sessions().refuse(id),
                Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
                Some(Ok(Message::Close(_)) | Err(_)) | None => break,
            },
            line = lines.recv() => {
                let Some(line) = line else { break };
                let sent = timeout(SEND_TIMEOUT, socket.send(Message::Text(line.into()))).await;
                if !matches!(sent, Ok(Ok(()))) {
                    break;
                }
            }
            () = &mut stopped => break,
        }
    }

    shared.sessions().disconnect(id);
    let _ = timeout(SEND_TIMEOUT, socket.send(Message::Close(None))).await;
}
