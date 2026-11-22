use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex, PoisonError};
use std::task::{Context, Poll};

use post_crypto::Fingerprint;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, verify_tls12_signature, verify_tls13_signature};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, SignatureScheme};
use rustls_platform_verifier::Verifier;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;
use tokio_rustls::client::TlsStream;

pub enum Transport {
    Plain(TcpStream),
    Tls(Box<TlsStream<TcpStream>>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Problem {
    UnknownCertificate(Fingerprint),
    ChangedCertificate(Fingerprint),
    NoConnection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    Public,
    Accepted,
    Refused,
}

#[derive(Debug)]
struct Pinned {
    platform: Verifier,
    provider: Arc<CryptoProvider>,
    accepted: Option<Fingerprint>,
    seen: Mutex<Option<(Fingerprint, Verdict)>>,
}

pub async fn secure(
    stream: TcpStream,
    host: &str,
    accepted: Option<Fingerprint>,
) -> Result<(Transport, Option<Fingerprint>), Problem> {
    let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
    let platform = Verifier::new(Arc::clone(&provider)).map_err(|_| Problem::NoConnection)?;
    let verifier = Arc::new(Pinned {
        platform,
        provider: Arc::clone(&provider),
        accepted,
        seen: Mutex::new(None),
    });
    let mut config = ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(|_| Problem::NoConnection)?
        .dangerous()
        .with_custom_certificate_verifier(Arc::clone(&verifier) as Arc<dyn ServerCertVerifier>)
        .with_no_client_auth();
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    let name = ServerName::try_from(host.to_owned()).map_err(|_| Problem::NoConnection)?;

    let connected = TlsConnector::from(Arc::new(config))
        .connect(name, stream)
        .await;
    match (connected, verifier.seen()) {
        (Ok(stream), Some((fingerprint, Verdict::Accepted))) => {
            Ok((Transport::Tls(Box::new(stream)), Some(fingerprint)))
        }
        (Ok(stream), _) => Ok((Transport::Tls(Box::new(stream)), None)),
        (Err(_), Some((fingerprint, Verdict::Refused))) if accepted.is_some() => {
            Err(Problem::ChangedCertificate(fingerprint))
        }
        (Err(_), Some((fingerprint, Verdict::Refused))) => {
            Err(Problem::UnknownCertificate(fingerprint))
        }
        (Err(_), _) => Err(Problem::NoConnection),
    }
}

impl Pinned {
    fn seen(&self) -> Option<(Fingerprint, Verdict)> {
        *self.seen.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl ServerCertVerifier for Pinned {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let fingerprint = Fingerprint::of_certificate(end_entity.as_ref());
        let public = self.platform.verify_server_cert(
            end_entity,
            intermediates,
            server_name,
            ocsp_response,
            now,
        );
        let (verdict, result) = match public {
            Ok(verified) => (Verdict::Public, Ok(verified)),
            Err(_) if self.accepted == Some(fingerprint) => {
                (Verdict::Accepted, Ok(ServerCertVerified::assertion()))
            }
            Err(error) => (Verdict::Refused, Err(error)),
        };
        *self.seen.lock().unwrap_or_else(PoisonError::into_inner) = Some((fingerprint, verdict));
        result
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

impl AsyncRead for Transport {
    fn poll_read(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Plain(stream) => Pin::new(stream).poll_read(context, buffer),
            Self::Tls(stream) => Pin::new(stream.as_mut()).poll_read(context, buffer),
        }
    }
}

impl AsyncWrite for Transport {
    fn poll_write(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.get_mut() {
            Self::Plain(stream) => Pin::new(stream).poll_write(context, buffer),
            Self::Tls(stream) => Pin::new(stream.as_mut()).poll_write(context, buffer),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Plain(stream) => Pin::new(stream).poll_flush(context),
            Self::Tls(stream) => Pin::new(stream.as_mut()).poll_flush(context),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Plain(stream) => Pin::new(stream).poll_shutdown(context),
            Self::Tls(stream) => Pin::new(stream.as_mut()).poll_shutdown(context),
        }
    }
}
