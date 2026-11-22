use std::fmt;
use std::fs;
use std::io::{self, Write as _};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use axum::serve::Listener;
use post_crypto::Fingerprint;
use post_proto::Timestamp;
use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair, date_time_ymd};
use rustls::ServerConfig;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Semaphore, mpsc};
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tokio_rustls::TlsAcceptor;
use tokio_rustls::server::TlsStream;

pub const CERTIFICATE: &str = "certificate.pem";
pub const KEY: &str = "certificate-key.pem";
const VALID_YEARS: i32 = 10;
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const HANDSHAKES: usize = 256;
const RETRY: Duration = Duration::from_millis(50);

#[derive(Clone)]
pub struct Identity {
    pub acceptor: TlsAcceptor,
    pub fingerprint: Fingerprint,
}

#[derive(Debug)]
pub enum TlsError {
    File { path: PathBuf, error: io::Error },
    Certificate(PathBuf),
    Key(PathBuf),
    Generate(rcgen::Error),
    Config(rustls::Error),
}

pub struct TlsListener {
    connections: mpsc::Receiver<(TlsStream<TcpStream>, SocketAddr)>,
    address: SocketAddr,
    task: JoinHandle<()>,
}

pub fn own(data: &Path, hosts: &[String], today: Timestamp) -> Result<Identity, TlsError> {
    let certificate = data.join(CERTIFICATE);
    let key = data.join(KEY);
    if !certificate.is_file() || !key.is_file() {
        make(&certificate, &key, hosts, today)?;
    }
    from_files(&certificate, &key)
}

pub fn from_files(certificate: &Path, key: &Path) -> Result<Identity, TlsError> {
    let chain = CertificateDer::pem_file_iter(certificate)
        .and_then(Iterator::collect::<Result<Vec<_>, _>>)
        .map_err(|_| TlsError::Certificate(certificate.to_owned()))?;
    let first = chain
        .first()
        .ok_or_else(|| TlsError::Certificate(certificate.to_owned()))?;
    let fingerprint = Fingerprint::of_certificate(first.as_ref());
    let secret = PrivateKeyDer::from_pem_file(key).map_err(|_| TlsError::Key(key.to_owned()))?;

    let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
    let mut config = ServerConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(TlsError::Config)?
        .with_no_client_auth()
        .with_single_cert(chain, secret)
        .map_err(TlsError::Config)?;
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(Identity {
        acceptor: TlsAcceptor::from(Arc::new(config)),
        fingerprint,
    })
}

fn make(
    certificate: &Path,
    key: &Path,
    hosts: &[String],
    today: Timestamp,
) -> Result<(), TlsError> {
    let mut params = CertificateParams::new(hosts.to_vec()).map_err(TlsError::Generate)?;
    let ((year, month, day), end) = validity(today);
    params.not_before = date_time_ymd(year, month, day);
    params.not_after = date_time_ymd(end.0, end.1, end.2);
    let mut name = DistinguishedName::new();
    name.push(DnType::CommonName, "Post server");
    params.distinguished_name = name;
    let pair = KeyPair::generate().map_err(TlsError::Generate)?;
    let made = params.self_signed(&pair).map_err(TlsError::Generate)?;
    write(key, &pair.serialize_pem(), true)?;
    write(certificate, &made.pem(), false)
}

type Date = (i32, u8, u8);

fn validity(today: Timestamp) -> (Date, Date) {
    let text = today.to_string();
    let number = |range: std::ops::Range<usize>| text.get(range).and_then(|part| part.parse().ok());
    let year: i32 = number(0..4).unwrap_or(1970);
    let month = number(5..7).map_or(1, |month: i32| u8::try_from(month).unwrap_or(1));
    let day = number(8..10).map_or(1, |day: i32| u8::try_from(day).unwrap_or(1));
    ((year, month, day), (year + VALID_YEARS, month, day.min(28)))
}

fn write(path: &Path, text: &str, private: bool) -> Result<(), TlsError> {
    let failed = |error| TlsError::File {
        path: path.to_owned(),
        error,
    };
    let mut options = fs::File::options();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    if private {
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    }
    let _ = private;
    let mut file = options.open(path).map_err(failed)?;
    file.write_all(text.as_bytes()).map_err(failed)?;
    file.sync_all().map_err(failed)
}

impl TlsListener {
    pub fn new(listener: TcpListener, address: SocketAddr, acceptor: TlsAcceptor) -> Self {
        let (sender, connections) = mpsc::channel(HANDSHAKES);
        let limit = Arc::new(Semaphore::new(HANDSHAKES));
        let task = tokio::spawn(async move {
            loop {
                let Ok(permit) = Arc::clone(&limit).acquire_owned().await else {
                    return;
                };
                let Ok((stream, peer)) = listener.accept().await else {
                    tokio::time::sleep(RETRY).await;
                    continue;
                };
                let _ = stream.set_nodelay(true);
                let acceptor = acceptor.clone();
                let sender = sender.clone();
                tokio::spawn(async move {
                    let _permit = permit;
                    let handshake = timeout(HANDSHAKE_TIMEOUT, acceptor.accept(stream)).await;
                    if let Ok(Ok(stream)) = handshake {
                        let _ = sender.send((stream, peer)).await;
                    }
                });
            }
        });
        Self {
            connections,
            address,
            task,
        }
    }
}

impl Drop for TlsListener {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Listener for TlsListener {
    type Io = TlsStream<TcpStream>;
    type Addr = SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        match self.connections.recv().await {
            Some(connection) => connection,
            None => std::future::pending().await,
        }
    }

    fn local_addr(&self) -> io::Result<Self::Addr> {
        Ok(self.address)
    }
}

impl fmt::Display for TlsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::File { path, error } => {
                write!(formatter, "cannot write {}: {error}", path.display())
            }
            Self::Certificate(path) => write!(
                formatter,
                "the file {} does not contain a certificate",
                path.display()
            ),
            Self::Key(path) => write!(
                formatter,
                "the file {} does not contain a private key",
                path.display()
            ),
            Self::Generate(error) => write!(formatter, "cannot make a certificate: {error}"),
            Self::Config(error) => write!(formatter, "cannot use the certificate: {error}"),
        }
    }
}

impl std::error::Error for TlsError {}

#[cfg(test)]
mod tests {
    use std::fs;

    use post_proto::Timestamp;
    use post_testing::Directory;

    use super::{CERTIFICATE, KEY, TlsError, from_files, own, validity};

    fn today() -> Timestamp {
        Timestamp::parse("2026-11-22T10:00:00.000Z").expect("valid")
    }

    fn hosts() -> Vec<String> {
        vec!["localhost".to_owned(), "127.0.0.1".to_owned()]
    }

    #[test]
    fn makes_a_certificate_at_the_first_start_and_keeps_it() {
        let data = Directory::new();

        let first = own(data.path(), &hosts(), today()).expect("a certificate");
        let second = own(data.path(), &hosts(), today()).expect("the same certificate");

        assert_eq!(first.fingerprint, second.fingerprint);
        let certificate = fs::read_to_string(data.join(CERTIFICATE)).expect("a file");
        assert!(certificate.starts_with("-----BEGIN CERTIFICATE-----"));
        let key = fs::read_to_string(data.join(KEY)).expect("a file");
        assert!(key.starts_with("-----BEGIN PRIVATE KEY-----"));
    }

    #[test]
    fn makes_a_different_certificate_for_each_server() {
        let first = Directory::new();
        let second = Directory::new();

        let first = own(first.path(), &hosts(), today()).expect("a certificate");
        let second = own(second.path(), &hosts(), today()).expect("a certificate");

        assert_ne!(first.fingerprint, second.fingerprint);
    }

    #[cfg(unix)]
    #[test]
    fn lets_only_the_user_read_the_key() {
        use std::os::unix::fs::PermissionsExt;

        let data = Directory::new();
        own(data.path(), &hosts(), today()).expect("a certificate");

        let key = fs::metadata(data.join(KEY)).expect("a key file");
        assert_eq!(key.permissions().mode() & 0o777, 0o600);
    }

    #[test]
    fn makes_a_certificate_that_is_valid_for_ten_years() {
        assert_eq!(validity(today()), ((2026, 11, 22), (2036, 11, 22)));
        let leap_day = Timestamp::parse("2028-02-29T23:59:59.999Z").expect("valid");
        assert_eq!(validity(leap_day), ((2028, 2, 29), (2038, 2, 28)));
        let last_day = Timestamp::parse("2026-01-31T00:00:00.000Z").expect("valid");
        assert_eq!(validity(last_day), ((2026, 1, 31), (2036, 1, 28)));
    }

    #[test]
    fn uses_certificate_files_of_the_operator() {
        let made = Directory::new();
        let expected = own(made.path(), &hosts(), today()).expect("a certificate");

        let loaded = from_files(&made.join(CERTIFICATE), &made.join(KEY)).expect("valid files");

        assert_eq!(loaded.fingerprint, expected.fingerprint);
    }

    #[test]
    fn tells_which_file_is_not_valid() {
        let data = Directory::new();
        own(data.path(), &hosts(), today()).expect("a certificate");
        let certificate = data.join(CERTIFICATE);
        let key = data.join(KEY);
        let text = data.join("note.txt");
        fs::write(&text, "This file is not a certificate.\n").expect("a file");
        let missing = data.join("missing.pem");

        for not_a_certificate in [&text, &missing, &key] {
            let error = from_files(not_a_certificate, &key).err().expect("an error");
            assert!(
                matches!(&error, TlsError::Certificate(path) if path == not_a_certificate),
                "{error}"
            );
            assert!(
                error
                    .to_string()
                    .ends_with("does not contain a certificate")
            );
        }
        for not_a_key in [&text, &missing, &certificate] {
            let error = from_files(&certificate, not_a_key).err().expect("an error");
            assert!(
                matches!(&error, TlsError::Key(path) if path == not_a_key),
                "{error}"
            );
            assert!(
                error
                    .to_string()
                    .ends_with("does not contain a private key")
            );
        }
    }

    #[test]
    fn refuses_a_key_that_is_not_the_key_of_the_certificate() {
        let first = Directory::new();
        let second = Directory::new();
        own(first.path(), &hosts(), today()).expect("a certificate");
        own(second.path(), &hosts(), today()).expect("a certificate");

        let error = from_files(&first.join(CERTIFICATE), &second.join(KEY))
            .err()
            .expect("an error");

        assert!(matches!(error, TlsError::Config(_)), "{error}");
        assert!(error.to_string().starts_with("cannot use the certificate"));
    }
}
