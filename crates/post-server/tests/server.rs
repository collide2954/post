use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::num::NonZeroU32;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use post_crypto::{Exchange, Fingerprint, Seed};
use post_proto::{
    ClientMessage, Code, ErrorCode, Key, MAX_LINE_BYTES, Name, Role, ServerMessage, Signature,
};
use post_server::config::{Config, Listen, Tls};
use post_server::store::StoreError;
use post_server::{Server, StartError, new_owner_link};
use post_testing::Directory;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, verify_tls12_signature, verify_tls13_signature};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, SignatureScheme};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tokio_rustls::TlsConnector;
use tokio_rustls::client::TlsStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, client_async, connect_async};

const PATIENCE: Duration = Duration::from_secs(10);

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;
type TlsSocket = WebSocketStream<TlsStream<TcpStream>>;

struct Running {
    port: u16,
    addresses: Vec<SocketAddr>,
    owner_link: Option<String>,
    fingerprint: Option<Fingerprint>,
    config: Config,
    data: Directory,
    stop: oneshot::Sender<()>,
    task: JoinHandle<std::io::Result<()>>,
}

async fn start() -> Running {
    start_with(Directory::new()).await
}

async fn start_with(data: Directory) -> Running {
    let config = Config {
        data: data.path().to_owned(),
        ..Config::development(0)
    };
    start_config(config, data).await
}

async fn start_config(config: Config, data: Directory) -> Running {
    let server = Server::bind(&config).await.expect("the server can bind");
    let addresses = server.addresses().to_vec();
    let owner_link = server.owner_link().map(str::to_owned);
    let fingerprint = server.fingerprint();
    let (stop, stopped) = oneshot::channel::<()>();
    let task = tokio::spawn(server.run(async {
        let _ = stopped.await;
    }));
    Running {
        port: addresses[0].port(),
        addresses,
        owner_link,
        fingerprint,
        config,
        data,
        stop,
        task,
    }
}

#[derive(Debug)]
struct AnyCertificate {
    provider: Arc<CryptoProvider>,
    seen: Mutex<Option<Fingerprint>>,
}

impl ServerCertVerifier for AnyCertificate {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let fingerprint = Fingerprint::of_certificate(end_entity.as_ref());
        *self.seen.lock().expect("no panic with the lock") = Some(fingerprint);
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        let algorithms = &self.provider.signature_verification_algorithms;
        verify_tls12_signature(message, cert, dss, algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        let algorithms = &self.provider.signature_verification_algorithms;
        verify_tls13_signature(message, cert, dss, algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

async fn connect_with_tls(port: u16) -> (TlsSocket, Fingerprint) {
    let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
    let verifier = Arc::new(AnyCertificate {
        provider: Arc::clone(&provider),
        seen: Mutex::new(None),
    });
    let config = ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])
        .expect("the provider has TLS 1.3")
        .dangerous()
        .with_custom_certificate_verifier(Arc::clone(&verifier) as Arc<dyn ServerCertVerifier>)
        .with_no_client_auth();
    let stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port))
        .await
        .expect("the server listens");
    let name = ServerName::try_from("localhost").expect("a valid name");
    let stream = timeout(
        PATIENCE,
        TlsConnector::from(Arc::new(config)).connect(name, stream),
    )
    .await
    .expect("the server answers in time")
    .expect("the server uses TLS 1.3");
    let (socket, _) = client_async(format!("wss://localhost:{port}/"), stream)
        .await
        .expect("the server accepts the connection");
    let fingerprint = verifier
        .seen
        .lock()
        .expect("no panic with the lock")
        .expect("the server showed a certificate");
    (socket, fingerprint)
}

impl Running {
    async fn stop(self) -> Directory {
        self.stop.send(()).expect("the server runs");
        timeout(PATIENCE, self.task)
            .await
            .expect("the server stops in time")
            .expect("the server task does not panic")
            .expect("the server stops without an error");
        self.data
    }
}

async fn connect(address: SocketAddr) -> Socket {
    let (socket, _) = timeout(PATIENCE, connect_async(format!("ws://{address}/")))
        .await
        .expect("the server answers in time")
        .expect("the server accepts the connection");
    socket
}

async fn connect_v4(port: u16) -> Socket {
    connect(SocketAddr::from((Ipv4Addr::LOCALHOST, port))).await
}

async fn write<S>(socket: &mut WebSocketStream<S>, message: &ClientMessage)
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    socket
        .send(Message::text(message.encode()))
        .await
        .expect("the socket is open");
}

async fn read<S>(socket: &mut WebSocketStream<S>) -> ServerMessage
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    loop {
        let frame = timeout(PATIENCE, socket.next())
            .await
            .expect("the server sends a message in time")
            .expect("the connection is open")
            .expect("the frame is valid");
        if let Message::Text(line) = frame {
            return ServerMessage::decode(line.as_str()).expect("a valid line");
        }
    }
}

async fn is_closed(socket: &mut Socket) -> bool {
    loop {
        match timeout(PATIENCE, socket.next()).await {
            Ok(None | Some(Err(_) | Ok(Message::Close(_)))) => return true,
            Ok(Some(Ok(_))) => {}
            Err(_) => return false,
        }
    }
}

fn name(text: &str) -> Name {
    Name::parse(text).expect(text)
}

async fn owner(server: &Running) -> Socket {
    let link = server
        .owner_link
        .clone()
        .expect("a new server has no owner");
    join_with(server.port, 7, "ana", &link).await
}

async fn join_with(port: u16, person: u8, who: &str, link: &str) -> Socket {
    let seed = Seed::from_bytes([person; 32]);
    let (mut socket, answer) = register_and_stay(port, &seed, who, link).await;
    assert!(
        matches!(answer, ServerMessage::Welcome { name: welcomed, .. } if welcomed == name(who))
    );
    assert_eq!(read(&mut socket).await, ServerMessage::Ready {});
    assert!(
        matches!(read(&mut socket).await, ServerMessage::Join { name: joined, .. } if joined == name(who))
    );
    socket
}

async fn invite(owner: &mut Socket) -> String {
    let invite = ClientMessage::Invite {
        reference: None,
        lifetime: None,
        uses: None,
    };
    write(owner, &invite).await;
    let ServerMessage::Invitations { list, .. } = read(owner).await else {
        panic!("the owner gets the open invitations");
    };
    list[0].link.clone()
}

async fn prove<S>(socket: &mut WebSocketStream<S>, address: &str, seed: &Seed) -> (Key, Signature)
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let client = post_crypto::nonce();
    let hello = ClientMessage::Hello {
        reference: None,
        protocol: 0,
        address: address.to_owned(),
        nonce: client,
        supports: Vec::new(),
    };
    write(socket, &hello).await;
    let ServerMessage::Challenge {
        key, nonce, proof, ..
    } = read(socket).await
    else {
        panic!("the server answers a hello message with a challenge");
    };
    let exchange = Exchange {
        address,
        client: &client,
        server: &nonce,
    };
    assert!(exchange.server_proof_is_valid(&key, &proof));
    let account = seed.account_key(&key);
    (account.public(), account.prove(&key, &exchange))
}

#[tokio::test]
async fn the_owner_makes_an_account_and_signs_in_again() {
    let server = start().await;
    let owner_link = server
        .owner_link
        .clone()
        .expect("a new server has no owner");
    let (start, code) = owner_link.rsplit_once('/').expect("a link has a code");
    assert_eq!(start, format!("http://localhost:{}/i", server.port));
    let seed = Seed::from_bytes([7; 32]);

    let mut first = connect_v4(server.port).await;
    let address = format!("localhost:{}", server.port);
    let (key, proof) = prove(&mut first, &address, &seed).await;
    let register = ClientMessage::Register {
        reference: None,
        key,
        proof,
        name: "ana".to_owned(),
        invitation: Code::parse(code).expect("a valid code"),
    };
    write(&mut first, &register).await;
    let welcome = ServerMessage::Welcome {
        reference: None,
        name: name("ana"),
        role: Role::Owner,
    };
    assert_eq!(read(&mut first).await, welcome);

    let mut second = connect(server.addresses[0]).await;
    let address = format!("127.0.0.1:{}", server.port);
    let (key, proof) = prove(&mut second, &address, &seed).await;
    let identify = ClientMessage::Identify {
        reference: None,
        key,
        proof,
        invitation: None,
    };
    write(&mut second, &identify).await;
    assert_eq!(read(&mut second).await, welcome);
}

async fn register(port: u16, seed: &Seed, who: &str, link: &str) -> ServerMessage {
    register_and_stay(port, seed, who, link).await.1
}

async fn register_and_stay(
    port: u16,
    seed: &Seed,
    who: &str,
    link: &str,
) -> (Socket, ServerMessage) {
    let (_, code) = link.rsplit_once('/').expect("a link has a code");
    let mut socket = connect_v4(port).await;
    let (key, proof) = prove(&mut socket, &format!("localhost:{port}"), seed).await;
    let register = ClientMessage::Register {
        reference: None,
        key,
        proof,
        name: who.to_owned(),
        invitation: Code::parse(code).expect("a valid code"),
    };
    write(&mut socket, &register).await;
    let answer = read(&mut socket).await;
    (socket, answer)
}

async fn identify(port: u16, seed: &Seed) -> ServerMessage {
    let mut socket = connect_v4(port).await;
    let (key, proof) = prove(&mut socket, &format!("localhost:{port}"), seed).await;
    let identify = ClientMessage::Identify {
        reference: None,
        key,
        proof,
        invitation: None,
    };
    write(&mut socket, &identify).await;
    read(&mut socket).await
}

fn welcome(who: &str, role: Role) -> ServerMessage {
    ServerMessage::Welcome {
        reference: None,
        name: name(who),
        role,
    }
}

#[tokio::test]
async fn keeps_the_accounts_when_it_starts_again() {
    let server = start().await;
    let owner_link = server
        .owner_link
        .clone()
        .expect("a new server has no owner");
    let seed = Seed::from_bytes([7; 32]);
    assert_eq!(
        register(server.port, &seed, "ana", &owner_link).await,
        welcome("ana", Role::Owner)
    );
    let data = server.stop().await;
    assert!(data.join("post.db").is_file());

    let server = start_with(data).await;

    assert_eq!(server.owner_link, None);
    assert_eq!(
        identify(server.port, &seed).await,
        welcome("ana", Role::Owner)
    );
    assert_eq!(
        identify(server.port, &Seed::from_bytes([8; 32])).await,
        ServerMessage::error(None, ErrorCode::UnknownAccount)
    );
}

#[tokio::test]
async fn a_person_joins_with_an_invitation_of_the_owner() {
    let server = start().await;
    let owner_link = server
        .owner_link
        .clone()
        .expect("a new server has no owner");
    let (mut ana, answer) =
        register_and_stay(server.port, &Seed::from_bytes([7; 32]), "ana", &owner_link).await;
    assert_eq!(answer, welcome("ana", Role::Owner));
    assert_eq!(read(&mut ana).await, ServerMessage::Ready {});
    assert!(matches!(read(&mut ana).await, ServerMessage::Join { .. }));

    let invite = ClientMessage::Invite {
        reference: Some(1),
        lifetime: NonZeroU32::new(3600),
        uses: NonZeroU32::new(1),
    };
    write(&mut ana, &invite).await;
    let ServerMessage::Invitations {
        reference: Some(1),
        list,
    } = read(&mut ana).await
    else {
        panic!("the owner gets the open invitations");
    };
    let [invitation] = &list[..] else {
        panic!("the server has one open invitation, not {list:?}");
    };
    assert_eq!(
        invitation.link,
        format!("http://localhost:{}/i/{}", server.port, invitation.code)
    );
    assert_eq!((invitation.uses, invitation.used), (Some(1), 0));

    let ben = Seed::from_bytes([8; 32]);
    assert_eq!(
        register(server.port, &ben, "ben", &invitation.link).await,
        welcome("ben", Role::Member)
    );
    assert!(
        matches!(read(&mut ana).await, ServerMessage::Join { name: joined, .. } if joined == name("ben"))
    );
    let cy = Seed::from_bytes([9; 32]);
    assert_eq!(
        register(server.port, &cy, "cy", &invitation.link).await,
        ServerMessage::error(None, ErrorCode::InvalidInvitation)
    );
}

#[tokio::test]
async fn makes_a_new_owner_link_for_a_server_that_does_not_run() {
    let server = start().await;
    let owner_link = server
        .owner_link
        .clone()
        .expect("a new server has no owner");
    let ana = Seed::from_bytes([7; 32]);
    register(server.port, &ana, "ana", &owner_link).await;
    let config = Config {
        listen: Listen::Loopback(4321),
        ..server.config.clone()
    };
    assert!(matches!(
        new_owner_link(&config),
        Err(StartError::Store(StoreError::InUse))
    ));

    let data = server.stop().await;
    let link = new_owner_link(&config).expect("the storage is free");

    assert!(link.starts_with("http://localhost:4321/i/"));
    assert_ne!(link, owner_link);
    let server = start_with(data).await;
    assert_eq!(server.owner_link, None);
    let ben = Seed::from_bytes([8; 32]);
    assert_eq!(
        register(server.port, &ben, "ben", &link).await,
        welcome("ben", Role::Owner)
    );
    assert_eq!(
        identify(server.port, &ana).await,
        welcome("ana", Role::Member)
    );
}

#[tokio::test]
async fn tells_why_it_cannot_start() {
    let data = Directory::new();
    let development = Config {
        data: data.path().to_owned(),
        ..Config::development(0)
    };

    let without_files = Config {
        tls: Tls::Files {
            certificate: data.join("missing.pem"),
            key: data.join("missing-key.pem"),
        },
        ..development.clone()
    };
    let error = Server::bind(&without_files).await.err().expect("an error");
    assert!(matches!(error, StartError::Tls(_)));
    assert!(
        error
            .to_string()
            .ends_with("does not contain a certificate")
    );

    let first = Server::bind(&development)
        .await
        .expect("the server can bind");
    let taken = Config {
        listen: Listen::Given(first.addresses().to_vec()),
        ..development.clone()
    };
    let error = Server::bind(&taken).await.err().expect("an error");
    assert!(matches!(error, StartError::Listen { .. }));
    assert!(error.to_string().starts_with("cannot listen on 127.0.0.1:"));

    let file = data.join("a-file");
    std::fs::write(&file, "text").expect("a file");
    let in_a_file = Config {
        data: file.join("data"),
        ..Config::development(0)
    };
    let error = Server::bind(&in_a_file).await.err().expect("an error");
    assert!(matches!(error, StartError::Data { .. }));
    assert!(
        error
            .to_string()
            .starts_with("cannot make the data directory")
    );

    let damaged = Directory::new();
    std::fs::write(damaged.join("post.db"), "This file is not a database.").expect("a file");
    let with_damage = Config {
        data: damaged.path().to_owned(),
        ..Config::development(0)
    };
    let error = Server::bind(&with_damage).await.err().expect("an error");
    assert!(matches!(error, StartError::Store(_)));
    assert!(error.to_string().starts_with("the database has a problem"));
}

fn with_own_certificate(data: &Directory) -> Config {
    Config {
        tls: Tls::Own,
        data: data.path().to_owned(),
        ..Config::development(0)
    }
}

#[tokio::test]
async fn uses_tls_with_its_own_certificate() {
    let data = Directory::new();
    let server = start_config(with_own_certificate(&data), data).await;
    let fingerprint = server
        .fingerprint
        .expect("a server with TLS has a certificate");
    let owner_link = server
        .owner_link
        .clone()
        .expect("a new server has no owner");
    let (start, code) = owner_link.rsplit_once('/').expect("a link has a code");
    assert_eq!(start, format!("https://localhost:{}/i", server.port));

    let (mut socket, seen) = connect_with_tls(server.port).await;

    assert_eq!(seen, fingerprint);
    let address = format!("localhost:{}", server.port);
    let (key, proof) = prove(&mut socket, &address, &Seed::from_bytes([7; 32])).await;
    let register = ClientMessage::Register {
        reference: None,
        key,
        proof,
        name: "ana".to_owned(),
        invitation: Code::parse(code).expect("a valid code"),
    };
    write(&mut socket, &register).await;
    assert_eq!(read(&mut socket).await, welcome("ana", Role::Owner));

    let without_tls = format!("ws://127.0.0.1:{}/", server.port);
    let plain = timeout(PATIENCE, connect_async(without_tls))
        .await
        .expect("the server answers in time");
    assert!(plain.is_err(), "the server accepts only TLS");
}

#[tokio::test]
async fn keeps_its_certificate_and_can_use_certificate_files() {
    let data = Directory::new();
    let own = with_own_certificate(&data);
    let first = start_config(own.clone(), data).await;
    let fingerprint = first.fingerprint.expect("a certificate");
    let data = first.stop().await;

    let second = start_config(own.clone(), data).await;
    assert_eq!(second.fingerprint, Some(fingerprint));
    let data = second.stop().await;

    let files = Config {
        tls: Tls::Files {
            certificate: data.join("certificate.pem"),
            key: data.join("certificate-key.pem"),
        },
        ..own
    };
    let third = start_config(files, data).await;
    assert_eq!(third.fingerprint, Some(fingerprint));
    let (_socket, seen) = connect_with_tls(third.port).await;
    assert_eq!(seen, fingerprint);
}

#[tokio::test]
async fn listens_on_the_addresses_that_the_configuration_gives() {
    let data = Directory::new();
    let config = Config {
        listen: Listen::Given(vec![SocketAddr::from((Ipv4Addr::LOCALHOST, 0))]),
        data: data.path().to_owned(),
        ..Config::development(0)
    };

    let server = Server::bind(&config).await.expect("the server can bind");

    assert_eq!(server.addresses().len(), 1);
    assert!(server.addresses()[0].ip().is_loopback());
}

#[tokio::test]
async fn two_users_exchange_text() {
    let server = start().await;
    let mut ana = owner(&server).await;
    let link = invite(&mut ana).await;
    let mut ben = join_with(server.port, 8, "ben", &link).await;
    assert!(
        matches!(read(&mut ana).await, ServerMessage::Join { channel, name: joined, .. } if channel == "lobby" && joined == name("ben"))
    );

    write(
        &mut ana,
        &ClientMessage::Send {
            reference: Some(1),
            channel: "lobby".to_owned(),
            text: "Hello".to_owned(),
        },
    )
    .await;

    let ServerMessage::Message {
        reference,
        channel,
        from,
        time,
        text,
    } = read(&mut ana).await
    else {
        panic!("the sender gets the message");
    };
    assert_eq!(reference, Some(1));
    assert_eq!(channel, "lobby");
    assert_eq!(from, name("ana"));
    assert_eq!(text, "Hello");
    assert_eq!(
        read(&mut ben).await,
        ServerMessage::Message {
            reference: None,
            channel,
            from,
            time,
            text,
        }
    );

    ben.close(None).await.expect("the socket can close");
    assert!(
        matches!(read(&mut ana).await, ServerMessage::Leave { channel, name: left, .. } if channel == "lobby" && left == name("ben"))
    );
}

#[tokio::test]
async fn tells_each_user_about_a_new_name() {
    let server = start().await;
    let mut ana = owner(&server).await;
    let link = invite(&mut ana).await;
    let mut ben = join_with(server.port, 8, "ben", &link).await;
    assert!(matches!(read(&mut ana).await, ServerMessage::Join { .. }));

    let rename = ClientMessage::Rename {
        reference: Some(3),
        name: "benito".to_owned(),
    };
    write(&mut ben, &rename).await;

    let ServerMessage::Rename {
        reference: Some(3),
        old,
        new,
        time,
    } = read(&mut ben).await
    else {
        panic!("the user gets the new name with the number of the request");
    };
    assert_eq!((&old, &new), (&name("ben"), &name("benito")));
    assert_eq!(
        read(&mut ana).await,
        ServerMessage::Rename {
            reference: None,
            old,
            new,
            time,
        }
    );
}
#[tokio::test]
async fn keeps_the_connection_open_after_an_error() {
    let server = start().await;
    let mut socket = connect_v4(server.port).await;

    socket
        .send(Message::text("this is not a message"))
        .await
        .expect("the socket is open");
    assert_eq!(
        read(&mut socket).await,
        ServerMessage::error(None, ErrorCode::BadMessage)
    );

    socket
        .send(Message::binary(vec![1, 2, 3]))
        .await
        .expect("the socket is open");
    assert_eq!(
        read(&mut socket).await,
        ServerMessage::error(None, ErrorCode::BadMessage)
    );

    let address = format!("localhost:{}", server.port);
    prove(&mut socket, &address, &Seed::from_bytes([7; 32])).await;
}

#[tokio::test]
async fn closes_a_connection_that_sends_a_line_that_is_too_long() {
    let server = start().await;
    let mut socket = connect_v4(server.port).await;

    socket
        .send(Message::text("x".repeat(MAX_LINE_BYTES + 1)))
        .await
        .expect("the socket is open");

    assert!(is_closed(&mut socket).await);
}

#[tokio::test]
async fn listens_only_on_the_loopback_addresses() {
    let server = start().await;

    assert_eq!(
        server.addresses[0],
        SocketAddr::from((Ipv4Addr::LOCALHOST, server.port))
    );
    for address in &server.addresses {
        assert!(address.ip().is_loopback(), "{address}");
        assert_eq!(address.port(), server.port);
    }
    assert!(server.addresses.len() <= 2);
    assert_eq!(server.fingerprint, None);
}

#[tokio::test]
async fn accepts_a_connection_on_the_ipv6_loopback_address_if_the_computer_has_one() {
    let server = start().await;
    let ipv6 = SocketAddr::from((Ipv6Addr::LOCALHOST, server.port));

    if server.addresses.contains(&ipv6) {
        let mut socket = connect(ipv6).await;
        let address = format!("[::1]:{}", server.port);
        prove(&mut socket, &address, &Seed::from_bytes([7; 32])).await;
    }
}

#[tokio::test]
async fn stops_and_closes_each_connection_when_the_stop_signal_comes() {
    let server = start().await;
    let mut ana = owner(&server).await;

    let stopped = tokio::spawn(server.stop());

    assert!(is_closed(&mut ana).await);
    stopped.await.expect("the server stops");
}

async fn get(port: u16, path: &str, language: Option<&str>) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port))
        .await
        .expect("the server listens");
    let language = language.map_or_else(String::new, |language| {
        format!("Accept-Language: {language}\r\n")
    });
    let request =
        format!("GET {path} HTTP/1.1\r\nHost: localhost\r\n{language}Connection: close\r\n\r\n");
    stream
        .write_all(request.as_bytes())
        .await
        .expect("the stream is open");
    let mut answer = String::new();
    timeout(PATIENCE, stream.read_to_string(&mut answer))
        .await
        .expect("the server answers in time")
        .expect("the answer is text");
    answer
}

#[tokio::test]
async fn shows_a_page_for_an_invite_link_in_a_browser() {
    let server = start().await;
    let link = server
        .owner_link
        .clone()
        .expect("a new server has no owner");
    let path = link
        .strip_prefix(&format!("http://localhost:{}", server.port))
        .expect("a link of this server");

    let english = get(server.port, path, None).await;
    let spanish = get(server.port, path, Some("es-MX,es;q=0.9,en;q=0.8")).await;

    assert!(english.starts_with("HTTP/1.1 200"), "{english}");
    let lower = english.to_lowercase();
    for header in [
        "content-type: text/html; charset=utf-8",
        "content-security-policy: default-src 'none'; style-src 'self'; script-src 'self'",
        "x-content-type-options: nosniff",
        "referrer-policy: no-referrer",
        "cache-control: no-store",
    ] {
        assert!(lower.contains(header), "{header}\n{english}");
    }
    assert!(english.contains(&format!("value=\"{link}\"")), "{english}");
    assert!(english.contains("This link is an invitation to a Post server."));
    assert!(spanish.starts_with("HTTP/1.1 200"), "{spanish}");
    assert!(spanish.contains("Este enlace es una invitación a un servidor de Post."));
}

#[tokio::test]
async fn tells_in_a_browser_that_an_invitation_is_not_valid() {
    let server = start().await;
    let link = server
        .owner_link
        .clone()
        .expect("a new server has no owner");
    let used = link
        .strip_prefix(&format!("http://localhost:{}", server.port))
        .expect("a link of this server")
        .to_owned();
    register(server.port, &Seed::from_bytes([7; 32]), "ana", &link).await;

    for path in [
        used.as_str(),
        "/i/AAAAAAAAAAAA",
        "/i/short",
        "/i/%3Cscript%3E",
    ] {
        let answer = get(server.port, path, None).await;
        assert!(answer.starts_with("HTTP/1.1 404"), "{path}\n{answer}");
        assert!(
            answer.contains("This invitation is not valid. Ask for a new invite link."),
            "{path}\n{answer}"
        );
        assert!(!answer.contains("<input"), "{path}");
    }
    let spanish = get(server.port, "/i/AAAAAAAAAAAA", Some("es")).await;
    assert!(
        spanish.contains("Esta invitación no es válida."),
        "{spanish}"
    );
    let other = get(server.port, "/i/AAAAAAAAAAAA/more", None).await;
    assert!(other.starts_with("HTTP/1.1 404"), "{other}");
}

#[tokio::test]
async fn gives_the_style_and_the_script_of_the_invite_page() {
    let server = start().await;

    let style = get(server.port, "/assets/invite.css", None).await;
    let script = get(server.port, "/assets/invite.js", None).await;

    assert!(style.starts_with("HTTP/1.1 200"), "{style}");
    assert!(style.to_lowercase().contains("content-type: text/css"));
    assert!(style.contains("prefers-color-scheme"));
    assert!(script.starts_with("HTTP/1.1 200"), "{script}");
    assert!(
        script
            .to_lowercase()
            .contains("content-type: text/javascript")
    );
    assert!(script.contains("navigator.clipboard"));
}

#[tokio::test]
async fn answers_a_request_that_is_not_a_websocket_request_with_an_http_error() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let server = start().await;
    let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, server.port))
        .await
        .expect("the server listens");
    stream
        .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .expect("the stream is open");

    let mut answer = String::new();
    timeout(PATIENCE, stream.read_to_string(&mut answer))
        .await
        .expect("the server answers in time")
        .expect("the answer is text");
    assert!(answer.starts_with("HTTP/1.1 4"), "{answer}");
}
