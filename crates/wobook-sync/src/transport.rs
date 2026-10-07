//! Pinned-key QUIC transport implementing `automerge_repo::NetworkTransport` (D9).
//!
//! One endpoint serves two ALPNs. `wobook-sync/1` connections must present a
//! trusted key; `wobook-pair/1` connections are handed to the pairing manager,
//! which only talks while a window is open. Every certificate is a self-signed
//! Ed25519 certificate; the key is the identity.

use std::{
    collections::HashMap,
    fmt,
    net::{IpAddr, Ipv6Addr, SocketAddr},
    ops::RangeInclusive,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use automerge_repo::{
    PeerId,
    error::NetworkError,
    network::{NetworkEvent, NetworkTransport},
    protocol::MAX_BODY_LEN,
};
use bytes::Bytes;
use ed25519_dalek::pkcs8::EncodePrivateKey;
use quinn::{Connection, ConnectionError, Endpoint, RecvStream, SendStream, VarInt};
use rcgen::{CertificateParams, KeyPair, PKCS_ED25519};
use rustls::{
    CertificateError, DigitallySignedStruct, DistinguishedName, Error as TlsError, SignatureScheme,
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    crypto::{CryptoProvider, verify_tls12_signature, verify_tls13_signature},
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime},
    server::danger::{ClientCertVerified, ClientCertVerifier},
};
use tokio::sync::{broadcast, mpsc};
use x509_parser::prelude::{FromDer, X509Certificate};

use crate::{
    control::{TrustResolver, TrustState},
    identity::{DeviceId, DeviceIdentity, PublicDeviceKey},
    rotation::{ControlMessage, MAX_CONTROL_LINE},
};

pub const SYNC_ALPN: &[u8] = b"wobook-sync/1";
pub const PAIR_ALPN: &[u8] = b"wobook-pair/1";
pub const SERVER_NAME: &str = "wobook.invalid";
pub const PORT_RANGE: RangeInclusive<u16> = 47390..=47399;
pub const MAX_CONNECTIONS: usize = 8;
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(60);
pub const KEEPALIVE: Duration = Duration::from_secs(15);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const FRAME_STREAM: u8 = b'S';
const CONTROL_STREAM: u8 = b'C';
const ACK: u8 = 0x06;
const MAX_FRAME: usize = MAX_BODY_LEN + 64;

/// Application close codes.
pub const CLOSE_SHUTDOWN: u32 = 0;
pub const CLOSE_UNKNOWN: u32 = 1;
pub const CLOSE_REVOKED: u32 = 2;
pub const CLOSE_REPLACED: u32 = 3;
pub const CLOSE_LIMIT: u32 = 4;
pub const CLOSE_PAIRING: u32 = 5;
pub const CLOSE_PROTOCOL: u32 = 6;

#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
pub enum TransportError {
    #[error("certificate is malformed")]
    MalformedCertificate,
    #[error("certificate does not hold an Ed25519 key")]
    WrongKeyAlgorithm,
    #[error("unknown_device")]
    UnknownPeer,
    #[error("device_revoked")]
    Revoked,
    #[error("peer presented an unexpected key")]
    KeyMismatch,
    #[error("connection limit reached")]
    Limit,
    #[error("connection replaced")]
    Replaced,
    #[error("UDP port {0} is in use")]
    AddrInUse(u16),
    #[error("QUIC configuration: {0}")]
    Configuration(String),
    #[error("connect: {0}")]
    Connect(String),
    #[error("stream: {0}")]
    Stream(String),
    #[error("transport closed")]
    Closed,
}

impl TransportError {
    /// Short machine code for `sync.status.last_error`.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnknownPeer => "unknown_device",
            Self::Revoked => "device_revoked",
            Self::KeyMismatch => "key_mismatch",
            Self::Limit => "connection_limit",
            Self::Replaced => "replaced",
            Self::Connect(_) => "unreachable",
            _ => "error",
        }
    }
}

fn from_connection_error(e: &ConnectionError) -> TransportError {
    match e {
        ConnectionError::ApplicationClosed(close) => {
            match u32::try_from(close.error_code.into_inner()) {
                Ok(CLOSE_UNKNOWN) => TransportError::UnknownPeer,
                Ok(CLOSE_REVOKED) => TransportError::Revoked,
                Ok(CLOSE_LIMIT) => TransportError::Limit,
                Ok(CLOSE_REPLACED) => TransportError::Replaced,
                _ => TransportError::Stream(e.to_string()),
            }
        }
        ConnectionError::TimedOut => TransportError::Connect("timed out".into()),
        ConnectionError::TransportError(t) if t.to_string().contains("certificate") => {
            TransportError::KeyMismatch
        }
        _ => TransportError::Connect(e.to_string()),
    }
}

pub struct TlsIdentity {
    pub certificate: CertificateDer<'static>,
    private_key: PrivatePkcs8KeyDer<'static>,
}

impl fmt::Debug for TlsIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TlsIdentity([REDACTED])")
    }
}

impl TlsIdentity {
    pub fn generate(identity: &DeviceIdentity) -> Result<Self, TransportError> {
        let cfg = |e: &dyn fmt::Display| TransportError::Configuration(e.to_string());
        let pkcs8 = identity
            .private_key()
            .signing_key()
            .to_pkcs8_der()
            .map_err(|e| cfg(&e))?;
        let private_key = PrivatePkcs8KeyDer::from(pkcs8.as_bytes().to_vec());
        let key_pair = KeyPair::from_pkcs8_der_and_sign_algo(&private_key, &PKCS_ED25519)
            .map_err(|e| cfg(&e))?;
        let mut params = CertificateParams::new(vec![SERVER_NAME.into()]).map_err(|e| cfg(&e))?;
        params
            .distinguished_name
            .push(rcgen::DnType::CommonName, identity.id().to_hex());
        let certificate = params.self_signed(&key_pair).map_err(|e| cfg(&e))?;
        let certificate = CertificateDer::from(certificate.der().to_vec());
        if !extract_public_key(&certificate)?.constant_time_eq(&identity.public_key()) {
            return Err(TransportError::KeyMismatch);
        }
        Ok(Self {
            certificate,
            private_key,
        })
    }

    fn private_key(&self) -> PrivateKeyDer<'static> {
        PrivateKeyDer::Pkcs8(self.private_key.clone_key())
    }
}

/// Ed25519 key of a self-signed certificate, with the signature verified.
pub fn extract_public_key(
    certificate: &CertificateDer<'_>,
) -> Result<PublicDeviceKey, TransportError> {
    let (rest, parsed) = X509Certificate::from_der(certificate.as_ref())
        .map_err(|_| TransportError::MalformedCertificate)?;
    if !rest.is_empty() {
        return Err(TransportError::MalformedCertificate);
    }
    parsed
        .verify_signature(None)
        .map_err(|_| TransportError::MalformedCertificate)?;
    let public = parsed.public_key();
    if public.algorithm.algorithm.to_id_string() != "1.3.101.112"
        || public.subject_public_key.unused_bits != 0
    {
        return Err(TransportError::WrongKeyAlgorithm);
    }
    let bytes: [u8; 32] = public
        .subject_public_key
        .data
        .as_ref()
        .try_into()
        .map_err(|_| TransportError::MalformedCertificate)?;
    PublicDeviceKey::from_bytes(bytes).map_err(|_| TransportError::MalformedCertificate)
}

fn bad_cert() -> TlsError {
    TlsError::InvalidCertificate(CertificateError::BadEncoding)
}

/// Client side: the server must hold the key whose hash is `expected`.
#[derive(Debug)]
struct PinnedServerVerifier {
    expected: DeviceId,
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for PinnedServerVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, TlsError> {
        if !intermediates.is_empty() {
            return Err(bad_cert());
        }
        let key = extract_public_key(end_entity).map_err(|_| bad_cert())?;
        if !key.device_id().constant_time_eq(&self.expected) {
            return Err(TlsError::InvalidCertificate(
                CertificateError::ApplicationVerificationFailure,
            ));
        }
        Ok(ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }
    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// Server side: any well-formed self-signed Ed25519 certificate. Trust is
/// decided per ALPN right after the handshake, before any application byte.
#[derive(Debug)]
struct SelfSignedClientVerifier {
    provider: Arc<CryptoProvider>,
}

impl ClientCertVerifier for SelfSignedClientVerifier {
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        &[]
    }
    fn verify_client_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        _now: UnixTime,
    ) -> Result<ClientCertVerified, TlsError> {
        if !intermediates.is_empty() {
            return Err(bad_cert());
        }
        extract_public_key(end_entity).map_err(|_| bad_cert())?;
        Ok(ClientCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }
    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

fn transport_config() -> quinn::TransportConfig {
    let mut t = quinn::TransportConfig::default();
    t.max_idle_timeout(Some(
        quinn::IdleTimeout::try_from(IDLE_TIMEOUT).expect("idle timeout fits"),
    ))
    .keep_alive_interval(Some(KEEPALIVE))
    .max_concurrent_bidi_streams(4_u8.into())
    .max_concurrent_uni_streams(0_u8.into());
    t
}

/// The peer key of an established connection.
pub fn peer_key(connection: &Connection) -> Result<PublicDeviceKey, TransportError> {
    let identity = connection
        .peer_identity()
        .ok_or(TransportError::MalformedCertificate)?;
    let certs = identity
        .downcast::<Vec<CertificateDer<'static>>>()
        .map_err(|_| TransportError::MalformedCertificate)?;
    extract_public_key(certs.first().ok_or(TransportError::MalformedCertificate)?)
}

fn alpn(connection: &Connection) -> Option<Vec<u8>> {
    connection
        .handshake_data()?
        .downcast::<quinn::crypto::rustls::HandshakeData>()
        .ok()?
        .protocol
}

/// An inbound pairing-ALPN connection for the pairing manager.
pub struct PairingConnection {
    pub connection: Connection,
    pub peer_key: PublicDeviceKey,
    pub remote: SocketAddr,
}

#[derive(Debug, Clone)]
pub enum SessionEvent {
    Connected {
        device: DeviceId,
        remote: SocketAddr,
        inbound: bool,
    },
    Disconnected {
        device: DeviceId,
        reason: Option<String>,
    },
}

struct Session {
    generation: u64,
    connection: Connection,
    dialer: DeviceId,
    frames: mpsc::Sender<Bytes>,
    control: mpsc::Sender<ControlMessage>,
}

pub struct QuinnTransport {
    endpoint: Endpoint,
    identity: Arc<DeviceIdentity>,
    tls: TlsIdentity,
    provider: Arc<CryptoProvider>,
    trust: Arc<dyn TrustResolver>,
    events: Mutex<(
        mpsc::Sender<NetworkEvent>,
        Option<mpsc::Receiver<NetworkEvent>>,
    )>,
    sessions: Mutex<HashMap<DeviceId, Session>>,
    generation: AtomicU64,
    closed: AtomicBool,
    pairing_tx: mpsc::Sender<PairingConnection>,
    pairing_rx: Mutex<Option<mpsc::Receiver<PairingConnection>>>,
    control_tx: mpsc::Sender<(DeviceId, ControlMessage)>,
    control_rx: Mutex<Option<mpsc::Receiver<(DeviceId, ControlMessage)>>>,
    session_events: broadcast::Sender<SessionEvent>,
}

impl fmt::Debug for QuinnTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QuinnTransport")
            .field("local_addr", &self.endpoint.local_addr().ok())
            .finish_non_exhaustive()
    }
}

const EVENT_CAPACITY: usize = 256;

impl QuinnTransport {
    /// Binds `[::]:port` (dual stack) or `0.0.0.0:port`.
    pub fn bind(
        port: u16,
        identity: Arc<DeviceIdentity>,
        trust: Arc<dyn TrustResolver>,
    ) -> Result<Arc<Self>, TransportError> {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let tls = TlsIdentity::generate(&identity)?;
        let cfg = |e: &dyn fmt::Display| TransportError::Configuration(e.to_string());
        let mut server_tls = rustls::ServerConfig::builder_with_provider(provider.clone())
            .with_protocol_versions(&[&rustls::version::TLS13])
            .map_err(|e| cfg(&e))?
            .with_client_cert_verifier(Arc::new(SelfSignedClientVerifier {
                provider: provider.clone(),
            }))
            .with_single_cert(vec![tls.certificate.clone()], tls.private_key())
            .map_err(|e| cfg(&e))?;
        server_tls.alpn_protocols = vec![SYNC_ALPN.to_vec(), PAIR_ALPN.to_vec()];
        let crypto =
            quinn::crypto::rustls::QuicServerConfig::try_from(server_tls).map_err(|e| cfg(&e))?;
        let mut server = quinn::ServerConfig::with_crypto(Arc::new(crypto));
        server.transport_config(Arc::new(transport_config()));
        let bind_v6 = SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), port);
        let endpoint = match Endpoint::server(server.clone(), bind_v6) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                return Err(TransportError::AddrInUse(port));
            }
            Err(_) => {
                Endpoint::server(server, SocketAddr::from(([0, 0, 0, 0], port))).map_err(|e| {
                    if e.kind() == std::io::ErrorKind::AddrInUse {
                        TransportError::AddrInUse(port)
                    } else {
                        cfg(&e)
                    }
                })?
            }
        };
        let (events_tx, events_rx) = mpsc::channel(EVENT_CAPACITY);
        let (pairing_tx, pairing_rx) = mpsc::channel(16);
        let (control_tx, control_rx) = mpsc::channel(EVENT_CAPACITY);
        let (session_events, _) = broadcast::channel(EVENT_CAPACITY);
        let transport = Arc::new(Self {
            endpoint,
            identity,
            tls,
            provider,
            trust,
            events: Mutex::new((events_tx, Some(events_rx))),
            sessions: Mutex::new(HashMap::new()),
            generation: AtomicU64::new(1),
            closed: AtomicBool::new(false),
            pairing_tx,
            pairing_rx: Mutex::new(Some(pairing_rx)),
            control_tx,
            control_rx: Mutex::new(Some(control_rx)),
            session_events,
        });
        let accept = transport.clone();
        tokio::spawn(async move { accept.accept_loop().await });
        Ok(transport)
    }

    /// Binds `fixed` when given, else the persisted port, else the first free
    /// port of [`PORT_RANGE`].
    pub fn bind_in_range(
        fixed: Option<u16>,
        persisted: Option<u16>,
        identity: Arc<DeviceIdentity>,
        trust: Arc<dyn TrustResolver>,
    ) -> Result<Arc<Self>, TransportError> {
        if let Some(port) = fixed {
            return Self::bind(port, identity, trust);
        }
        let candidates = persisted.into_iter().chain(PORT_RANGE);
        let mut last = TransportError::Configuration("no port".into());
        for port in candidates {
            match Self::bind(port, identity.clone(), trust.clone()) {
                Ok(t) => return Ok(t),
                Err(e @ TransportError::AddrInUse(_)) => last = e,
                Err(e) => return Err(e),
            }
        }
        Err(last)
    }

    #[must_use]
    pub fn port(&self) -> u16 {
        self.endpoint.local_addr().map(|a| a.port()).unwrap_or(0)
    }

    #[must_use]
    pub fn local_id(&self) -> DeviceId {
        self.identity.id()
    }

    pub fn take_pairing(&self) -> Option<mpsc::Receiver<PairingConnection>> {
        self.pairing_rx.lock().ok()?.take()
    }

    pub fn take_control(&self) -> Option<mpsc::Receiver<(DeviceId, ControlMessage)>> {
        self.control_rx.lock().ok()?.take()
    }

    #[must_use]
    pub fn subscribe_sessions(&self) -> broadcast::Receiver<SessionEvent> {
        self.session_events.subscribe()
    }

    #[must_use]
    pub fn connected(&self) -> Vec<DeviceId> {
        self.sessions
            .lock()
            .map(|s| s.keys().copied().collect())
            .unwrap_or_default()
    }

    #[must_use]
    pub fn is_connected(&self, device: DeviceId) -> bool {
        self.sessions.lock().is_ok_and(|s| s.contains_key(&device))
    }

    /// Remote address of a live session.
    #[must_use]
    pub fn remote_address(&self, device: DeviceId) -> Option<SocketAddr> {
        self.sessions
            .lock()
            .ok()?
            .get(&device)
            .map(|s| s.connection.remote_address())
    }

    fn client_config(
        &self,
        expected: DeviceId,
        alpn: &[u8],
    ) -> Result<quinn::ClientConfig, TransportError> {
        let cfg = |e: &dyn fmt::Display| TransportError::Configuration(e.to_string());
        let mut tls = rustls::ClientConfig::builder_with_provider(self.provider.clone())
            .with_protocol_versions(&[&rustls::version::TLS13])
            .map_err(|e| cfg(&e))?
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(PinnedServerVerifier {
                expected,
                provider: self.provider.clone(),
            }))
            .with_client_auth_cert(vec![self.tls.certificate.clone()], self.tls.private_key())
            .map_err(|e| cfg(&e))?;
        tls.alpn_protocols = vec![alpn.to_vec()];
        let crypto = quinn::crypto::rustls::QuicClientConfig::try_from(tls).map_err(|e| cfg(&e))?;
        let mut client = quinn::ClientConfig::new(Arc::new(crypto));
        client.transport_config(Arc::new(transport_config()));
        Ok(client)
    }

    async fn connect(
        &self,
        expected: DeviceId,
        address: SocketAddr,
        alpn: &[u8],
    ) -> Result<Connection, TransportError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(TransportError::Closed);
        }
        let address = match (self.endpoint.local_addr(), address) {
            // A v6 socket reaches v4 peers through mapped addresses.
            (Ok(local), SocketAddr::V4(v4)) if local.is_ipv6() => {
                SocketAddr::new(IpAddr::V6(v4.ip().to_ipv6_mapped()), v4.port())
            }
            _ => address,
        };
        let connecting = self
            .endpoint
            .connect_with(self.client_config(expected, alpn)?, address, SERVER_NAME)
            .map_err(|e| TransportError::Connect(e.to_string()))?;
        match tokio::time::timeout(HANDSHAKE_TIMEOUT, connecting).await {
            Ok(Ok(c)) => Ok(c),
            Ok(Err(e)) => Err(from_connection_error(&e)),
            Err(_) => Err(TransportError::Connect("handshake timed out".into())),
        }
    }

    /// Opens a pairing connection to the device whose id is `expected`.
    pub async fn dial_pair(
        &self,
        expected: DeviceId,
        address: SocketAddr,
    ) -> Result<Connection, TransportError> {
        self.connect(expected, address, PAIR_ALPN).await
    }

    /// Opens a sync session to a trusted device. Ok when a session exists
    /// afterwards (this dial or a concurrent one).
    pub async fn dial_sync(
        self: &Arc<Self>,
        device: DeviceId,
        address: SocketAddr,
    ) -> Result<(), TransportError> {
        let connection = self.connect(device, address, SYNC_ALPN).await?;
        let result = self.establish_outbound(device, connection.clone()).await;
        if let Err(e) = &result {
            connection.close(VarInt::from_u32(CLOSE_PROTOCOL), e.to_string().as_bytes());
            if *e == TransportError::Replaced && self.is_connected(device) {
                return Ok(());
            }
        }
        result
    }

    async fn establish_outbound(
        self: &Arc<Self>,
        device: DeviceId,
        connection: Connection,
    ) -> Result<(), TransportError> {
        self.check_trust(device)?;
        let stream_err = |e: &dyn fmt::Display| TransportError::Stream(e.to_string());
        let setup = async {
            let (mut fs, mut fr) = connection
                .open_bi()
                .await
                .map_err(|e| from_connection_error(&e))?;
            fs.write_all(&[FRAME_STREAM])
                .await
                .map_err(|e| stream_err(&e))?;
            let (mut cs, cr) = connection
                .open_bi()
                .await
                .map_err(|e| from_connection_error(&e))?;
            cs.write_all(&[CONTROL_STREAM])
                .await
                .map_err(|e| stream_err(&e))?;
            let mut ack = [0u8; 1];
            match fr.read_exact(&mut ack).await {
                Ok(()) if ack[0] == ACK => Ok((fs, fr, cs, cr)),
                Ok(()) => Err(TransportError::Stream("bad ack".into())),
                Err(_) => Err(connection.close_reason().map_or_else(
                    || TransportError::Stream("no ack".into()),
                    |e| from_connection_error(&e),
                )),
            }
        };
        let (fs, fr, cs, cr) = match tokio::time::timeout(HANDSHAKE_TIMEOUT, setup).await {
            Ok(r) => r?,
            Err(_) => return Err(TransportError::Stream("session setup timed out".into())),
        };
        self.install(
            device,
            self.identity.id(),
            connection,
            fs,
            fr,
            cs,
            cr,
            false,
        )
    }

    fn check_trust(&self, device: DeviceId) -> Result<(), TransportError> {
        match self.trust.peer_trust(device) {
            Ok(Some(r)) if r.state == TrustState::Trusted => Ok(()),
            Ok(Some(_)) => Err(TransportError::Revoked),
            Ok(None) => Err(TransportError::UnknownPeer),
            Err(e) => Err(TransportError::Configuration(e)),
        }
    }

    async fn accept_loop(self: Arc<Self>) {
        while let Some(incoming) = self.endpoint.accept().await {
            if self.closed.load(Ordering::Acquire) {
                incoming.refuse();
                continue;
            }
            let transport = self.clone();
            tokio::spawn(async move {
                let Ok(connecting) = incoming.accept() else {
                    return;
                };
                let Ok(Ok(connection)) = tokio::time::timeout(HANDSHAKE_TIMEOUT, connecting).await
                else {
                    return;
                };
                transport.handle_inbound(connection).await;
            });
        }
    }

    async fn handle_inbound(self: Arc<Self>, connection: Connection) {
        let Ok(key) = peer_key(&connection) else {
            connection.close(VarInt::from_u32(CLOSE_PROTOCOL), b"bad certificate");
            return;
        };
        let device = key.device_id();
        let protocol = alpn(&connection);
        if protocol.as_deref() == Some(PAIR_ALPN) {
            let remote = connection.remote_address();
            if self
                .pairing_tx
                .try_send(PairingConnection {
                    connection: connection.clone(),
                    peer_key: key,
                    remote,
                })
                .is_err()
            {
                connection.close(VarInt::from_u32(CLOSE_PAIRING), b"pairing busy");
            }
            return;
        }
        if protocol.as_deref() != Some(SYNC_ALPN) {
            connection.close(VarInt::from_u32(CLOSE_PROTOCOL), b"alpn");
            return;
        }
        // Trust is decided before any application byte is read.
        if let Err(e) = self.check_trust(device) {
            let code = if e == TransportError::Revoked {
                CLOSE_REVOKED
            } else {
                CLOSE_UNKNOWN
            };
            connection.close(VarInt::from_u32(code), e.code().as_bytes());
            return;
        }
        let setup = async {
            let mut frames = None;
            let mut control = None;
            while frames.is_none() || control.is_none() {
                let (send, mut recv): (SendStream, RecvStream) =
                    connection.accept_bi().await.ok()?;
                let mut kind = [0u8; 1];
                recv.read_exact(&mut kind).await.ok()?;
                match kind[0] {
                    FRAME_STREAM => frames = Some((send, recv)),
                    CONTROL_STREAM => control = Some((send, recv)),
                    _ => return None,
                }
            }
            Some((frames?, control?))
        };
        let Ok(Some(((fs, fr), (cs, cr)))) = tokio::time::timeout(HANDSHAKE_TIMEOUT, setup).await
        else {
            connection.close(VarInt::from_u32(CLOSE_PROTOCOL), b"setup");
            return;
        };
        match self.install(device, device, connection.clone(), fs, fr, cs, cr, true) {
            Ok(()) => {}
            Err(e) => {
                let code = match e {
                    TransportError::Limit => CLOSE_LIMIT,
                    _ => CLOSE_REPLACED,
                };
                connection.close(VarInt::from_u32(code), e.to_string().as_bytes());
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn install(
        self: &Arc<Self>,
        device: DeviceId,
        dialer: DeviceId,
        connection: Connection,
        mut frame_send: SendStream,
        frame_recv: RecvStream,
        control_send: SendStream,
        control_recv: RecvStream,
        inbound: bool,
    ) -> Result<(), TransportError> {
        let generation = self.generation.fetch_add(1, Ordering::AcqRel);
        let peer = PeerId::new(device.to_hex());
        let (frames_tx, frames_rx) = mpsc::channel::<Bytes>(64);
        let (control_tx, control_rx) = mpsc::channel::<ControlMessage>(32);
        let events = self.events_sender();
        {
            let mut sessions = self.sessions.lock().map_err(|_| TransportError::Closed)?;
            if let Some(existing) = sessions.get(&device) {
                let local = self.identity.id();
                let preferred = if local < device { local } else { device };
                let dead = existing.connection.close_reason().is_some();
                let replace = dead || existing.dialer == dialer || dialer == preferred;
                if !replace {
                    return Err(TransportError::Replaced);
                }
                let old = sessions.remove(&device).expect("present");
                old.connection
                    .close(VarInt::from_u32(CLOSE_REPLACED), b"replaced");
                let _ = events.try_send(NetworkEvent::PeerDisconnected(peer.clone()));
            } else if sessions.len() >= MAX_CONNECTIONS {
                return Err(TransportError::Limit);
            }
            sessions.insert(
                device,
                Session {
                    generation,
                    connection: connection.clone(),
                    dialer,
                    frames: frames_tx,
                    control: control_tx,
                },
            );
            let _ = events.try_send(NetworkEvent::PeerConnected(peer.clone()));
        }
        if inbound {
            // Tells the dialer the session is installed.
            let ack = async move {
                frame_send.write_all(&[ACK]).await.ok()?;
                Some(frame_send)
            };
            let transport = self.clone();
            tokio::spawn(async move {
                if let Some(send) = ack.await {
                    transport
                        .run_session(
                            device,
                            generation,
                            send,
                            frame_recv,
                            control_send,
                            control_recv,
                            frames_rx,
                            control_rx,
                        )
                        .await;
                }
            });
        } else {
            let transport = self.clone();
            tokio::spawn(async move {
                transport
                    .run_session(
                        device,
                        generation,
                        frame_send,
                        frame_recv,
                        control_send,
                        control_recv,
                        frames_rx,
                        control_rx,
                    )
                    .await;
            });
        }
        let _ = self.session_events.send(SessionEvent::Connected {
            device,
            remote: connection.remote_address(),
            inbound,
        });
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_session(
        self: Arc<Self>,
        device: DeviceId,
        generation: u64,
        mut frame_send: SendStream,
        mut frame_recv: RecvStream,
        mut control_send: SendStream,
        control_recv: RecvStream,
        mut frames_rx: mpsc::Receiver<Bytes>,
        mut control_rx: mpsc::Receiver<ControlMessage>,
    ) {
        let peer = PeerId::new(device.to_hex());
        let writer = async {
            while let Some(frame) = frames_rx.recv().await {
                let len = u32::try_from(frame.len()).unwrap_or(u32::MAX);
                if frame_send.write_all(&len.to_be_bytes()).await.is_err()
                    || frame_send.write_all(&frame).await.is_err()
                {
                    break;
                }
            }
        };
        let control_writer = async {
            while let Some(message) = control_rx.recv().await {
                if control_send.write_all(&message.encode()).await.is_err() {
                    break;
                }
            }
        };
        let reader = async {
            loop {
                let mut len = [0u8; 4];
                if frame_recv.read_exact(&mut len).await.is_err() {
                    break;
                }
                let len = u32::from_be_bytes(len) as usize;
                if len == 0 || len > MAX_FRAME {
                    break;
                }
                let mut buf = vec![0u8; len];
                if frame_recv.read_exact(&mut buf).await.is_err() {
                    break;
                }
                let Some(events) = self.current_sender(device, generation) else {
                    break;
                };
                if events
                    .send(NetworkEvent::Message {
                        peer: peer.clone(),
                        bytes: Bytes::from(buf),
                    })
                    .await
                    .is_err()
                {
                    break;
                }
            }
        };
        let control_reader = async {
            let mut reader = tokio::io::BufReader::new(control_recv);
            while let Ok(Some(line)) =
                wobook_core::protocol::read_line_capped(&mut reader, MAX_CONTROL_LINE).await
            {
                if let Ok(message) = ControlMessage::decode(line.as_bytes()) {
                    let _ = self.control_tx.send((device, message)).await;
                }
            }
            // Keep the session alive when the peer simply has nothing to say.
            std::future::pending::<()>().await;
        };
        tokio::select! {
            () = writer => {},
            () = control_writer => {},
            () = reader => {},
            () = control_reader => {},
        }
        self.drop_session(device, generation, None);
    }

    fn events_sender(&self) -> mpsc::Sender<NetworkEvent> {
        self.events.lock().expect("events lock").0.clone()
    }

    fn current_sender(
        &self,
        device: DeviceId,
        generation: u64,
    ) -> Option<mpsc::Sender<NetworkEvent>> {
        let sessions = self.sessions.lock().ok()?;
        (sessions.get(&device)?.generation == generation).then(|| self.events_sender())
    }

    fn drop_session(&self, device: DeviceId, generation: u64, code: Option<u32>) {
        let removed = {
            let Ok(mut sessions) = self.sessions.lock() else {
                return;
            };
            match sessions.get(&device) {
                Some(s) if s.generation == generation => sessions.remove(&device),
                _ => None,
            }
        };
        if let Some(session) = removed {
            let reason = session
                .connection
                .close_reason()
                .map(|e| from_connection_error(&e).code().to_string());
            session
                .connection
                .close(VarInt::from_u32(code.unwrap_or(CLOSE_SHUTDOWN)), b"closed");
            let _ = self
                .events_sender()
                .try_send(NetworkEvent::PeerDisconnected(PeerId::new(device.to_hex())));
            let _ = self
                .session_events
                .send(SessionEvent::Disconnected { device, reason });
        }
    }

    /// Closes the session with `device`, telling it why.
    pub fn close_device(&self, device: DeviceId, code: u32) {
        let generation = self
            .sessions
            .lock()
            .ok()
            .and_then(|s| s.get(&device).map(|s| s.generation));
        if let Some(generation) = generation {
            self.drop_session(device, generation, Some(code));
        }
    }

    pub async fn send_control(&self, device: DeviceId, message: ControlMessage) -> bool {
        let sender = self
            .sessions
            .lock()
            .ok()
            .and_then(|s| s.get(&device).map(|s| s.control.clone()));
        match sender {
            Some(sender) => sender.send(message).await.is_ok(),
            None => false,
        }
    }

    pub async fn broadcast_control(&self, message: ControlMessage) {
        for device in self.connected() {
            self.send_control(device, message.clone()).await;
        }
    }
}

#[async_trait]
impl NetworkTransport for QuinnTransport {
    /// Each call hands out a fresh receiver: the repository is reopened when a
    /// device joins a mesh. Live sessions carry the old repository's sync
    /// state, so they are closed first and re-dialed by the supervisors.
    fn take_events(&self) -> Result<mpsc::Receiver<NetworkEvent>, NetworkError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(NetworkError::Closed);
        }
        if let Some(rx) = self
            .events
            .lock()
            .map_err(|_| NetworkError::Closed)?
            .1
            .take()
        {
            return Ok(rx);
        }
        for device in self.connected() {
            self.close_device(device, CLOSE_SHUTDOWN);
        }
        let (tx, rx) = mpsc::channel(EVENT_CAPACITY);
        self.events.lock().map_err(|_| NetworkError::Closed)?.0 = tx;
        Ok(rx)
    }

    async fn send(&self, peer: &PeerId, frame: Bytes) -> Result<(), NetworkError> {
        let fail = |message: &str| NetworkError::Transport {
            peer: peer.clone(),
            message: message.into(),
        };
        let device: DeviceId = peer.as_str().parse().map_err(|_| fail("bad peer id"))?;
        let sender = self
            .sessions
            .lock()
            .map_err(|_| NetworkError::Closed)?
            .get(&device)
            .map(|s| s.frames.clone())
            .ok_or_else(|| fail("not connected"))?;
        sender.send(frame).await.map_err(|_| fail("session closed"))
    }

    async fn close_peer(&self, peer: &PeerId) -> Result<(), NetworkError> {
        if let Ok(device) = peer.as_str().parse() {
            self.close_device(device, CLOSE_SHUTDOWN);
        }
        Ok(())
    }

    async fn close(&self) -> Result<(), NetworkError> {
        // The repository closes on shutdown and on reopen; the endpoint itself
        // is closed through `shutdown`.
        Ok(())
    }
}

impl QuinnTransport {
    pub async fn shutdown(&self) {
        self.closed.store(true, Ordering::Release);
        let devices = self.connected();
        for device in devices {
            self.close_device(device, CLOSE_SHUTDOWN);
        }
        self.endpoint
            .close(VarInt::from_u32(CLOSE_SHUTDOWN), b"shutdown");
        let _ = tokio::time::timeout(Duration::from_secs(1), self.endpoint.wait_idle()).await;
    }
}
