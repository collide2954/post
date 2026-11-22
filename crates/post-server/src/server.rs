use std::fmt;
use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use axum::Router;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::header::{
    ACCEPT_LANGUAGE, CACHE_CONTROL, CONTENT_SECURITY_POLICY, CONTENT_TYPE, REFERRER_POLICY,
    X_CONTENT_TYPE_OPTIONS,
};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get};
use post_crypto::Fingerprint;
use post_proto::{Address, Code, Host, MAX_LINE_BYTES, Timestamp};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinSet;
use tokio::time::timeout;

use crate::config::{Config, Listen, Tls};
use crate::hub::{Hub, LIMITS, Random, Settings};
use crate::page;
use crate::sessions::Sessions;
use crate::store::{Store, StoreError};
use crate::tls::{self, TlsError, TlsListener};

const QUEUE_LINES: usize = 256;
const SEND_TIMEOUT: Duration = Duration::from_secs(10);

pub struct Server {
    listeners: Vec<Bound>,
    addresses: Vec<SocketAddr>,
    hub: Hub,
    owner_link: Option<String>,
    fingerprint: Option<Fingerprint>,
}

enum Bound {
    Plain(TcpListener),
    Tls(TlsListener),
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
    Tls(TlsError),
}

struct Shared {
    sessions: Mutex<Sessions>,
    stopped: watch::Receiver<bool>,
}

impl Server {
    pub async fn bind(config: &Config) -> Result<Self, StartError> {
        let (listeners, addresses) = listen(&config.listen).await?;
        let port = addresses[0].port();
        let public = config.public_addresses(port);
        let settings = Settings {
            addresses: public.iter().map(ToString::to_string).collect(),
            link: config.link(port),
            requires: Vec::new(),
        };
        let store = open_store(config)?;
        let identity = match &config.tls {
            Tls::Off | Tls::Proxy => None,
            Tls::Own => Some(tls::own(&config.data, &hosts(&public), Timestamp::now())?),
            Tls::Files { certificate, key } => Some(tls::from_files(certificate, key)?),
        };
        let mut hub = Hub::new(store, settings, Random::default())?;
        let owner_link = hub.owner_link(Timestamp::now())?;

        let fingerprint = identity.as_ref().map(|identity| identity.fingerprint);
        let listeners = listeners
            .into_iter()
            .zip(&addresses)
            .map(|(listener, address)| match &identity {
                Some(identity) => Bound::Tls(TlsListener::new(
                    listener,
                    *address,
                    identity.acceptor.clone(),
                )),
                None => Bound::Plain(listener),
            })
            .collect();
        Ok(Self {
            listeners,
            addresses,
            hub,
            owner_link,
            fingerprint,
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

    #[must_use]
    pub fn fingerprint(&self) -> Option<Fingerprint> {
        self.fingerprint
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
        let router = Router::new()
            .route("/", any(upgrade))
            .route("/i/{code}", get(invitation))
            .route("/assets/invite.css", get(style))
            .route("/assets/invite.js", get(script))
            .with_state(shared);

        let mut tasks = JoinSet::new();
        for listener in self.listeners {
            let router = router.clone();
            let stopped = wait_for_stop(stopped.clone());
            match listener {
                Bound::Plain(listener) => tasks.spawn(async move {
                    axum::serve(listener, router)
                        .with_graceful_shutdown(stopped)
                        .await
                }),
                Bound::Tls(listener) => tasks.spawn(async move {
                    axum::serve(listener, router)
                        .with_graceful_shutdown(stopped)
                        .await
                }),
            };
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

fn hosts(addresses: &[Address]) -> Vec<String> {
    let mut hosts: Vec<String> = Vec::new();
    for address in addresses {
        let host = match address.host() {
            Host::Name(name) => name.clone(),
            Host::Ip(ip) => ip.to_string(),
        };
        if !hosts.contains(&host) {
            hosts.push(host);
        }
    }
    hosts
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

impl From<TlsError> for StartError {
    fn from(error: TlsError) -> Self {
        Self::Tls(error)
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
            Self::Tls(error) => error.fmt(formatter),
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

async fn invitation(
    State(shared): State<Arc<Shared>>,
    Path(code): Path<String>,
    headers: HeaderMap,
) -> Response {
    let wanted = headers
        .get(ACCEPT_LANGUAGE)
        .and_then(|value| value.to_str().ok());
    let language = page::Language::from_header(wanted);
    let link = Code::parse(&code)
        .ok()
        .and_then(|code| shared.sessions().open_link(&code));
    let (status, body) = match link {
        Some(link) => (StatusCode::OK, page::invitation(language, &link)),
        None => (StatusCode::NOT_FOUND, page::not_valid(language)),
    };
    let headers = [
        (CONTENT_TYPE, "text/html; charset=utf-8"),
        (CONTENT_SECURITY_POLICY, page::POLICY),
        (X_CONTENT_TYPE_OPTIONS, "nosniff"),
        (REFERRER_POLICY, "no-referrer"),
        (CACHE_CONTROL, "no-store"),
    ];
    (status, headers, body).into_response()
}

async fn style() -> Response {
    asset("text/css; charset=utf-8", page::STYLE)
}

async fn script() -> Response {
    asset("text/javascript; charset=utf-8", page::SCRIPT)
}

fn asset(kind: &'static str, body: &'static str) -> Response {
    let headers = [
        (CONTENT_TYPE, kind),
        (X_CONTENT_TYPE_OPTIONS, "nosniff"),
        (CACHE_CONTROL, "max-age=3600"),
    ];
    (headers, body).into_response()
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
