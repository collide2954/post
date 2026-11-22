use std::fmt;
use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use axum::Router;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use axum::routing::any;
use post_proto::{MAX_LINE_BYTES, Timestamp};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinSet;
use tokio::time::timeout;

use crate::config::{Config, Listen, Tls};
use crate::hub::{Hub, LIMITS, Random, Settings};
use crate::sessions::Sessions;
use crate::store::{Store, StoreError};

const QUEUE_LINES: usize = 256;
const SEND_TIMEOUT: Duration = Duration::from_secs(10);

pub struct Server {
    listeners: Vec<TcpListener>,
    addresses: Vec<SocketAddr>,
    hub: Hub,
    owner_link: Option<String>,
}

#[derive(Debug)]
pub enum StartError {
    Listen {
        address: SocketAddr,
        error: io::Error,
    },
    Data {
        path: PathBuf,
        error: io::Error,
    },
    Store(StoreError),
    NoTls,
}

struct Shared {
    sessions: Mutex<Sessions>,
    stopped: watch::Receiver<bool>,
}

impl Server {
    pub async fn bind(config: &Config) -> Result<Self, StartError> {
        if matches!(config.tls, Tls::Own | Tls::Files { .. }) {
            return Err(StartError::NoTls);
        }
        let (listeners, addresses) = listen(&config.listen).await?;
        let port = addresses[0].port();
        let settings = Settings {
            addresses: config
                .public_addresses(port)
                .iter()
                .map(ToString::to_string)
                .collect(),
            link: config.link(port),
            requires: Vec::new(),
        };
        let mut hub = Hub::new(open_store(config)?, settings, Random::default())?;
        let owner_link = hub.owner_link(Timestamp::now())?;

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

pub fn new_owner_link(config: &Config) -> Result<String, StartError> {
    let code = post_crypto::code();
    let mut store = open_store(config)?;
    let mut rules = store.rules(LIMITS)?;
    rules
        .new_owner_code(&mut store, &code, Timestamp::now())?
        .map_err(|_| StoreError::CodeInUse)?;
    Ok(format!("{}/i/{code}", config.link(config.port())))
}

fn open_store(config: &Config) -> Result<Store, StartError> {
    std::fs::create_dir_all(&config.data).map_err(|error| StartError::Data {
        path: config.data.clone(),
        error,
    })?;
    Ok(Store::open(&config.database())?)
}

async fn listen(listen: &Listen) -> Result<(Vec<TcpListener>, Vec<SocketAddr>), StartError> {
    let mut listeners = Vec::new();
    match listen {
        Listen::Loopback(port) => {
            let local = [Ipv4Addr::LOCALHOST.into(), Ipv6Addr::LOCALHOST.into()];
            listen_on_two(&mut listeners, local, *port).await?;
        }
        Listen::All(port) => {
            let all = [Ipv4Addr::UNSPECIFIED.into(), Ipv6Addr::UNSPECIFIED.into()];
            listen_on_two(&mut listeners, all, *port).await?;
        }
        Listen::Given(addresses) => {
            for address in addresses {
                listeners.push(listen_on(*address).await?);
            }
        }
    }
    let addresses = listeners.iter().map(|(_, address)| *address).collect();
    let listeners = listeners
        .into_iter()
        .map(|(listener, _)| listener)
        .collect();
    Ok((listeners, addresses))
}

async fn listen_on_two(
    listeners: &mut Vec<(TcpListener, SocketAddr)>,
    [required, optional]: [IpAddr; 2],
    port: u16,
) -> Result<(), StartError> {
    let first = listen_on(SocketAddr::new(required, port)).await?;
    let port = first.1.port();
    listeners.push(first);
    if let Ok(second) = listen_on(SocketAddr::new(optional, port)).await {
        listeners.push(second);
    }
    Ok(())
}

async fn listen_on(address: SocketAddr) -> Result<(TcpListener, SocketAddr), StartError> {
    let failed = |error| StartError::Listen { address, error };
    let listener = TcpListener::bind(address).await.map_err(failed)?;
    let address = listener.local_addr().map_err(failed)?;
    Ok((listener, address))
}

impl From<StoreError> for StartError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

impl fmt::Display for StartError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Listen { address, error } => {
                write!(formatter, "cannot listen on {address}: {error}")
            }
            Self::Data { path, error } => write!(
                formatter,
                "cannot make the data directory {}: {error}",
                path.display()
            ),
            Self::Store(error) => error.fmt(formatter),
            Self::NoTls => formatter.write_str(
                "this version cannot use TLS yet: use the TLS mode \"proxy\" or no configuration file",
            ),
        }
    }
}

impl std::error::Error for StartError {}

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
