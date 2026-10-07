//! Drives the pairing reducer over QUIC streams (D6) and owns the window.

use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use async_trait::async_trait;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use quinn::{Connection, RecvStream, SendStream, VarInt};
use serde::{Deserialize, Serialize};
use tokio::{io::BufReader, sync::mpsc};

use super::{
    payload::{PAIR_TTL_S, QrPayload, ValidPayload},
    proof::{FailureCounter, PairRateLimiter, fingerprint, pair_mac, verify_pair_mac},
    reducer::{Action, ErrorCode, Event, Role, State, reduce},
};
use crate::{
    control::{EndpointKind, SqliteControlStore},
    endpoints,
    identity::{DeviceId, DeviceIdentity, PublicDeviceKey},
    rotation::{DeviceWire, GroupWire},
    transport::{CLOSE_SHUTDOWN, PairingConnection, QuinnTransport},
};

pub const PAIR_WINDOW: Duration = Duration::from_secs(PAIR_TTL_S as u64);
pub const CONFIRM_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_LINE: usize = 64 * 1024;
/// Finished sessions stay listed this long so clients can read the outcome.
const KEEP_FINISHED: Duration = Duration::from_secs(60);
pub const PLATFORM: &str = "linux";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum PairMessage {
    #[serde(rename = "pair.begin")]
    Begin {
        name: String,
        platform: String,
        #[serde(default)]
        endpoints: Vec<String>,
    },
    #[serde(rename = "pair.nonce")]
    Nonce {
        nonce: String,
        name: String,
        platform: String,
    },
    #[serde(rename = "pair.complete")]
    Complete { mac: String },
    #[serde(rename = "pair.decision")]
    Decision { trusted: bool },
    #[serde(rename = "pair.provision")]
    Provision {
        root: String,
        devices: Vec<DeviceWire>,
        group: GroupWire,
        sync_port: u16,
    },
    #[serde(rename = "pair.done")]
    Done,
    #[serde(rename = "pair.error")]
    Error {
        code: ErrorCode,
        #[serde(default)]
        message: Option<String>,
    },
}

/// What the daemon provides to pairing: the root and the D7 join procedure.
#[async_trait]
pub trait MeshHost: Send + Sync + 'static {
    /// Current root document id, if any.
    fn root(&self) -> Option<String>;
    /// Joins `root` (export, quarantine, join, wait, import) unless already on it.
    async fn adopt_root(&self, root: String, offerer: DeviceId) -> Result<(), String>;
}

#[derive(Debug, Clone)]
pub enum PairingEvent {
    /// Both sides stored trust. `announce` is set on the offerer.
    Paired { device: DeviceId, announce: bool },
}

#[derive(Debug, Clone, Serialize)]
pub struct PendingPair {
    pub session: String,
    pub role: Role,
    pub peer_name: String,
    pub peer_platform: String,
    pub fingerprint: String,
    pub expires_at: i64,
    pub stage: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<&'static str>,
}

struct SessionEntry {
    info: PendingPair,
    decisions: mpsc::Sender<bool>,
    finished: Option<Instant>,
}

struct Window {
    secret: [u8; 32],
    expires: Instant,
    consumed: bool,
    failures: FailureCounter,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PairError {
    #[error("pairing window is closed")]
    WindowClosed,
    #[error("too many pairing attempts")]
    RateLimited,
    #[error("no pending pairing with that session id")]
    PendingMissing,
    #[error("{0}")]
    Invalid(String),
}

struct Inner {
    identity: Arc<DeviceIdentity>,
    store: Arc<SqliteControlStore>,
    transport: Arc<QuinnTransport>,
    host: Arc<dyn MeshHost>,
    window: Mutex<Option<Window>>,
    limiter: PairRateLimiter,
    sessions: Mutex<HashMap<String, SessionEntry>>,
    events: mpsc::UnboundedSender<PairingEvent>,
}

#[derive(Clone)]
pub struct PairingManager {
    inner: Arc<Inner>,
}

fn new_session_id() -> String {
    hex::encode(rand::random::<[u8; 8]>())
}

fn unix_s() -> i64 {
    wobook_core::now_ms() / 1000
}

fn device_wire(
    store: &SqliteControlStore,
    key: &PublicDeviceKey,
    name: &str,
    platform: &str,
) -> DeviceWire {
    let id = key.device_id();
    DeviceWire {
        id: id.to_hex(),
        key: URL_SAFE_NO_PAD.encode(key.as_bytes()),
        name: name.to_string(),
        platform: platform.to_string(),
        endpoints: store
            .endpoints(id)
            .unwrap_or_default()
            .into_iter()
            .take(8)
            .map(|e| e.address.to_string())
            .collect(),
    }
}

/// Decodes and checks a device record (key must hash to the id).
pub fn decode_device(wire: &DeviceWire) -> Option<PublicDeviceKey> {
    let bytes: [u8; 32] = URL_SAFE_NO_PAD.decode(&wire.key).ok()?.try_into().ok()?;
    let key = PublicDeviceKey::from_bytes(bytes).ok()?;
    (key.device_id().to_hex() == wire.id).then_some(key)
}

/// Stores `addresses` for `device`, classifying lan/tailnet.
pub fn store_endpoints<'a>(
    store: &SqliteControlStore,
    device: DeviceId,
    addresses: impl IntoIterator<Item = &'a String>,
) {
    for a in addresses {
        if let Ok(address) = a.parse::<SocketAddr>() {
            let _ = store.upsert_endpoint(device, address, EndpointKind::for_address(&address));
        }
    }
}

impl PairingManager {
    #[must_use]
    pub fn new(
        identity: Arc<DeviceIdentity>,
        store: Arc<SqliteControlStore>,
        transport: Arc<QuinnTransport>,
        host: Arc<dyn MeshHost>,
    ) -> (Self, mpsc::UnboundedReceiver<PairingEvent>) {
        let (events, rx) = mpsc::unbounded_channel();
        let manager = Self {
            inner: Arc::new(Inner {
                identity,
                store,
                transport: transport.clone(),
                host,
                window: Mutex::new(None),
                limiter: PairRateLimiter::default(),
                sessions: Mutex::new(HashMap::new()),
                events,
            }),
        };
        if let Some(mut incoming) = transport.take_pairing() {
            let m = manager.clone();
            tokio::spawn(async move {
                while let Some(conn) = incoming.recv().await {
                    let m = m.clone();
                    tokio::spawn(async move { m.inner.clone().accept(conn).await });
                }
            });
        }
        (manager, rx)
    }

    /// Opens a new window (replacing any previous one) and returns its payload.
    pub fn start(&self) -> Result<QrPayload, PairError> {
        let secret: [u8; 32] = rand::random();
        let name = self
            .inner
            .store
            .device_name()
            .map_err(|e| PairError::Invalid(e.to_string()))?;
        let hints = endpoints::current_hints(self.inner.transport.port());
        let payload = QrPayload::build(&name, self.inner.identity.id(), &hints, &secret, unix_s());
        *self
            .inner
            .window
            .lock()
            .map_err(|_| PairError::WindowClosed)? = Some(Window {
            secret,
            expires: Instant::now() + PAIR_WINDOW,
            consumed: false,
            failures: FailureCounter::default(),
        });
        Ok(payload)
    }

    /// Validates the payload and starts the joiner side; returns the session id.
    pub fn join(&self, payload: &QrPayload) -> Result<String, PairError> {
        let valid = payload
            .validate(unix_s())
            .map_err(|e| PairError::Invalid(e.to_string()))?;
        if valid.id == self.inner.identity.id() {
            return Err(PairError::Invalid(
                "cannot pair with this device itself".into(),
            ));
        }
        let session = new_session_id();
        let (tx, rx) = mpsc::channel(4);
        self.inner.insert(
            &session,
            PendingPair {
                session: session.clone(),
                role: Role::Joiner,
                peer_name: valid.name.clone(),
                peer_platform: String::new(),
                fingerprint: String::new(),
                expires_at: valid.exp,
                stage: State::Start.name(),
                error: None,
            },
            tx,
        );
        let inner = self.inner.clone();
        let id = session.clone();
        tokio::spawn(async move { inner.run_joiner(id, valid, rx).await });
        Ok(session)
    }

    #[must_use]
    pub fn pending(&self) -> Vec<PendingPair> {
        let Ok(mut sessions) = self.inner.sessions.lock() else {
            return Vec::new();
        };
        sessions.retain(|_, s| s.finished.is_none_or(|t| t.elapsed() < KEEP_FINISHED));
        let mut list: Vec<PendingPair> = sessions.values().map(|s| s.info.clone()).collect();
        list.sort_by(|a, b| a.session.cmp(&b.session));
        list
    }

    pub async fn decide(&self, session: &str, trusted: bool) -> Result<(), PairError> {
        let sender = self
            .inner
            .sessions
            .lock()
            .map_err(|_| PairError::PendingMissing)?
            .get(session)
            .filter(|s| s.finished.is_none())
            .map(|s| s.decisions.clone())
            .ok_or(PairError::PendingMissing)?;
        sender
            .send(trusted)
            .await
            .map_err(|_| PairError::PendingMissing)
    }
}

/// One JSON-lines pairing stream.
struct Wire {
    send: SendStream,
    recv: BufReader<RecvStream>,
}

impl Wire {
    async fn send(&mut self, message: &PairMessage) -> Result<(), ErrorCode> {
        let mut line = serde_json::to_vec(message).map_err(|_| ErrorCode::Protocol)?;
        line.push(b'\n');
        self.send
            .write_all(&line)
            .await
            .map_err(|_| ErrorCode::Protocol)
    }
    async fn recv(&mut self) -> Result<PairMessage, ErrorCode> {
        match wobook_core::protocol::read_line_capped(&mut self.recv, MAX_LINE).await {
            Ok(Some(line)) => serde_json::from_str(&line).map_err(|_| ErrorCode::Protocol),
            _ => Err(ErrorCode::Protocol),
        }
    }
    async fn fail(mut self, code: ErrorCode, conn: &Connection) {
        let _ = self
            .send(&PairMessage::Error {
                code,
                message: None,
            })
            .await;
        let _ = self.send.finish();
        // Give the peer a moment to read the error before closing.
        let _ = tokio::time::timeout(Duration::from_secs(2), self.send.stopped()).await;
        conn.close(VarInt::from_u32(CLOSE_SHUTDOWN), code.as_str().as_bytes());
    }
}

impl Message {
    fn event(&self) -> Event {
        match self.0 {
            PairMessage::Begin { .. } => Event::Begin,
            PairMessage::Nonce { .. } => Event::Nonce,
            PairMessage::Complete { .. } => Event::Complete { valid: false },
            PairMessage::Decision { trusted } => Event::RemoteDecision(trusted),
            PairMessage::Provision { .. } => Event::Provision,
            PairMessage::Done => Event::Done,
            PairMessage::Error { code, .. } => Event::Error(code),
        }
    }
}

struct Message(PairMessage);

/// Per-session data the action executor needs.
struct Ctx {
    role: Role,
    session: String,
    conn: Connection,
    peer_key: PublicDeviceKey,
    peer_name: String,
    peer_platform: String,
    peer_endpoints: Vec<String>,
    nonce: [u8; 32],
    secret: [u8; 32],
    provision: Option<PairMessage>,
    adopt: Option<String>,
}

impl Inner {
    fn insert(&self, session: &str, info: PendingPair, decisions: mpsc::Sender<bool>) {
        if let Ok(mut s) = self.sessions.lock() {
            s.insert(
                session.to_string(),
                SessionEntry {
                    info,
                    decisions,
                    finished: None,
                },
            );
        }
    }

    fn update(&self, session: &str, f: impl FnOnce(&mut SessionEntry)) {
        if let Ok(mut s) = self.sessions.lock()
            && let Some(entry) = s.get_mut(session)
        {
            f(entry);
        }
    }

    fn set_state(&self, session: &str, state: &State) {
        let stage = state.name();
        let terminal = state.is_terminal();
        let error = match state {
            State::Failed { code } => Some(code.as_str()),
            _ => None,
        };
        self.update(session, |e| {
            e.info.stage = stage;
            e.info.error = error;
            if terminal {
                e.finished = Some(Instant::now());
            }
        });
    }

    fn local_name(&self) -> String {
        self.store.device_name().unwrap_or_else(|_| "wobook".into())
    }

    /// Offerer side of an inbound pairing connection.
    async fn accept(self: Arc<Self>, incoming: PairingConnection) {
        let PairingConnection {
            connection,
            peer_key,
            remote,
        } = incoming;
        let remote = crate::canonical(remote);
        let Ok(Ok((send, recv))) =
            tokio::time::timeout(Duration::from_secs(10), connection.accept_bi()).await
        else {
            connection.close(VarInt::from_u32(CLOSE_SHUTDOWN), b"no stream");
            return;
        };
        let wire = Wire {
            send,
            recv: BufReader::new(recv),
        };
        if !self.limiter.allow(remote.ip(), Instant::now()) {
            return wire.fail(ErrorCode::RateLimited, &connection).await;
        }
        let gate = {
            let window = self.window.lock().ok();
            match window.as_ref().and_then(|w| w.as_ref()) {
                None => Err(ErrorCode::Expired),
                Some(w) if w.consumed || Instant::now() >= w.expires => Err(ErrorCode::Expired),
                Some(w) if w.failures.locked(peer_key.as_bytes(), remote.ip()) => {
                    Err(ErrorCode::RateLimited)
                }
                Some(w) => Ok(w.secret),
            }
        };
        let secret = match gate {
            Ok(secret) => secret,
            Err(code) => return wire.fail(code, &connection).await,
        };
        let session = new_session_id();
        let (tx, rx) = mpsc::channel(4);
        let ctx = Ctx {
            role: Role::Offerer,
            session: session.clone(),
            conn: connection.clone(),
            peer_key,
            peer_name: String::new(),
            peer_platform: String::new(),
            peer_endpoints: vec![remote.to_string()],
            nonce: rand::random(),
            secret,
            provision: None,
            adopt: None,
        };
        self.drive(ctx, wire, rx, Some(tx), remote).await;
    }

    async fn run_joiner(
        self: Arc<Self>,
        session: String,
        payload: ValidPayload,
        rx: mpsc::Receiver<bool>,
    ) {
        let transport = self.transport.clone();
        let expected = payload.id;
        let dialed = endpoints::race(payload.endpoints.clone(), move |addr| {
            let transport = transport.clone();
            async move { transport.dial_pair(expected, addr).await }
        })
        .await;
        let (remote, connection) = match dialed {
            Ok((addr, conn, _)) => (addr, conn),
            Err(_) => {
                self.set_state(
                    &session,
                    &State::Failed {
                        code: ErrorCode::Expired,
                    },
                );
                self.update(&session, |e| e.info.error = Some("unreachable"));
                return;
            }
        };
        let Ok((send, recv)) = connection.open_bi().await else {
            self.set_state(
                &session,
                &State::Failed {
                    code: ErrorCode::Protocol,
                },
            );
            return;
        };
        let Ok(peer_key) = crate::transport::peer_key(&connection) else {
            self.set_state(
                &session,
                &State::Failed {
                    code: ErrorCode::Protocol,
                },
            );
            return;
        };
        self.update(&session, |e| {
            e.info.fingerprint = fingerprint(peer_key.as_bytes());
        });
        let ctx = Ctx {
            role: Role::Joiner,
            session,
            conn: connection,
            peer_key,
            peer_name: payload.name.clone(),
            peer_platform: String::new(),
            peer_endpoints: payload.endpoints.iter().map(ToString::to_string).collect(),
            nonce: [0; 32],
            secret: payload.secret,
            provision: None,
            adopt: None,
        };
        let wire = Wire {
            send,
            recv: BufReader::new(recv),
        };
        self.drive(ctx, wire, rx, None, remote).await;
    }

    #[allow(clippy::too_many_lines)]
    async fn drive(
        self: &Arc<Self>,
        mut ctx: Ctx,
        mut wire: Wire,
        mut decisions: mpsc::Receiver<bool>,
        register: Option<mpsc::Sender<bool>>,
        remote: SocketAddr,
    ) {
        let mut register = register;
        let mut state = State::Start;
        let mut deadline = tokio::time::Instant::now() + PAIR_WINDOW;
        let mut queue = vec![Event::Begin];
        if ctx.role == Role::Offerer {
            // The offerer's first event comes from the wire.
            queue.clear();
        }
        loop {
            let event = if let Some(e) = queue.pop() {
                e
            } else {
                let decisions_open = matches!(state, State::Confirming { local: None, .. });
                tokio::select! {
                    msg = wire.recv() => match msg {
                        Ok(message) => self.absorb(&mut ctx, Message(message)),
                        Err(code) => Event::Error(code),
                    },
                    d = decisions.recv(), if decisions_open => match d {
                        Some(d) => Event::LocalDecision(d),
                        None => Event::LocalDecision(false),
                    },
                    () = tokio::time::sleep_until(deadline) => Event::Timeout,
                }
            };
            let (next, actions) = reduce(ctx.role, &state, event);
            state = next;
            let mut failed = None;
            for action in actions {
                if let Err(code) = self
                    .execute(
                        &mut ctx,
                        &mut wire,
                        action,
                        &mut register,
                        remote,
                        &mut deadline,
                    )
                    .await
                {
                    failed = Some(code);
                    break;
                }
            }
            if let Some(code) = failed {
                state = State::Failed { code };
                self.set_state(&ctx.session, &state);
                return wire.fail(code, &ctx.conn).await;
            }
            self.set_state(&ctx.session, &state);
            match &state {
                State::Done => break,
                State::Failed { code } => {
                    if *code == ErrorCode::BadProof
                        && let Ok(mut w) = self.window.lock()
                        && let Some(window) = w.as_mut()
                        && window.failures.fail(*ctx.peer_key.as_bytes(), remote.ip())
                    {
                        *w = None;
                    }
                    let _ = wire.send.finish();
                    let _ = tokio::time::timeout(Duration::from_secs(2), wire.send.stopped()).await;
                    ctx.conn
                        .close(VarInt::from_u32(CLOSE_SHUTDOWN), code.as_str().as_bytes());
                    return;
                }
                _ => {}
            }
        }
        let _ = wire.send.finish();
        let _ = tokio::time::timeout(Duration::from_secs(2), wire.send.stopped()).await;
        ctx.conn.close(VarInt::from_u32(CLOSE_SHUTDOWN), b"paired");
        let device = ctx.peer_key.device_id();
        if let Some(root) = ctx.adopt.take() {
            let host = self.host.clone();
            let events = self.events.clone();
            tokio::spawn(async move {
                if let Err(e) = host.adopt_root(root, device).await {
                    eprintln!("wobook-sync: joining mesh failed: {e}");
                }
                let _ = events.send(PairingEvent::Paired {
                    device,
                    announce: false,
                });
            });
        } else {
            let _ = self.events.send(PairingEvent::Paired {
                device,
                announce: ctx.role == Role::Offerer,
            });
        }
    }

    /// Records message content in the context and maps it to a reducer event.
    fn absorb(&self, ctx: &mut Ctx, message: Message) -> Event {
        let event = message.event();
        match message.0 {
            PairMessage::Begin {
                name,
                platform,
                endpoints,
            } => {
                ctx.peer_name = name;
                ctx.peer_platform = platform;
                ctx.peer_endpoints.extend(endpoints);
            }
            PairMessage::Nonce {
                nonce,
                name,
                platform,
            } => {
                match URL_SAFE_NO_PAD
                    .decode(nonce)
                    .ok()
                    .and_then(|b| <[u8; 32]>::try_from(b).ok())
                {
                    Some(n) => ctx.nonce = n,
                    None => return Event::Error(ErrorCode::Protocol),
                }
                ctx.peer_name = name;
                ctx.peer_platform = platform;
            }
            PairMessage::Complete { mac } => {
                let valid = hex::decode(mac).is_ok_and(|mac| {
                    verify_pair_mac(
                        &ctx.secret,
                        self.identity.public_key().as_bytes(),
                        ctx.peer_key.as_bytes(),
                        &ctx.nonce,
                        &mac,
                    )
                });
                if valid {
                    // One success per window.
                    let consumed = self.window.lock().ok().and_then(|mut w| {
                        let window = w.as_mut()?;
                        if window.consumed || Instant::now() >= window.expires {
                            return None;
                        }
                        window.consumed = true;
                        Some(())
                    });
                    if consumed.is_none() {
                        return Event::Error(ErrorCode::Expired);
                    }
                }
                return Event::Complete { valid };
            }
            p @ PairMessage::Provision { .. } => ctx.provision = Some(p),
            _ => {}
        }
        event
    }

    async fn execute(
        self: &Arc<Self>,
        ctx: &mut Ctx,
        wire: &mut Wire,
        action: Action,
        register: &mut Option<mpsc::Sender<bool>>,
        remote: SocketAddr,
        deadline: &mut tokio::time::Instant,
    ) -> Result<(), ErrorCode> {
        match action {
            Action::SendBegin => {
                let hints = endpoints::current_hints(self.transport.port());
                wire.send(&PairMessage::Begin {
                    name: self.local_name(),
                    platform: PLATFORM.into(),
                    endpoints: hints.iter().map(ToString::to_string).collect(),
                })
                .await
            }
            Action::SendNonce => {
                wire.send(&PairMessage::Nonce {
                    nonce: URL_SAFE_NO_PAD.encode(ctx.nonce),
                    name: self.local_name(),
                    platform: PLATFORM.into(),
                })
                .await
            }
            Action::SendProof => {
                let mac = pair_mac(
                    &ctx.secret,
                    ctx.peer_key.as_bytes(),
                    self.identity.public_key().as_bytes(),
                    &ctx.nonce,
                );
                wire.send(&PairMessage::Complete {
                    mac: hex::encode(mac),
                })
                .await
            }
            Action::SendDecision(trusted) => wire.send(&PairMessage::Decision { trusted }).await,
            Action::SendProvision => {
                let host_root = self.host.root().ok_or(ErrorCode::Protocol)?;
                let joiner = ctx.peer_key.device_id();
                let devices = self
                    .store
                    .trusted_peers()
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|p| p.device_id != joiner)
                    .map(|p| device_wire(&self.store, &p.public_key, &p.name, &p.platform))
                    .collect();
                let group = self
                    .store
                    .ensure_discovery_group()
                    .map_err(|_| ErrorCode::Protocol)?;
                wire.send(&PairMessage::Provision {
                    root: host_root,
                    devices,
                    group: GroupWire::from(&group),
                    sync_port: self.transport.port(),
                })
                .await
            }
            Action::SendDone => wire.send(&PairMessage::Done).await,
            Action::SendError(code) => {
                let _ = wire
                    .send(&PairMessage::Error {
                        code,
                        message: None,
                    })
                    .await;
                Ok(())
            }
            Action::ExposePending => {
                *deadline = tokio::time::Instant::now() + CONFIRM_TIMEOUT;
                let info = PendingPair {
                    session: ctx.session.clone(),
                    role: ctx.role,
                    peer_name: ctx.peer_name.clone(),
                    peer_platform: ctx.peer_platform.clone(),
                    fingerprint: fingerprint(ctx.peer_key.as_bytes()),
                    expires_at: unix_s() + CONFIRM_TIMEOUT.as_secs() as i64,
                    stage: "confirming",
                    error: None,
                };
                if let Some(tx) = register.take() {
                    self.insert(&ctx.session, info, tx);
                } else {
                    self.update(&ctx.session, |e| e.info = info);
                }
                Ok(())
            }
            Action::Join => {
                let Some(PairMessage::Provision { root, .. }) = &ctx.provision else {
                    return Err(ErrorCode::Protocol);
                };
                let current = self.host.root();
                let meshed = !self.store.trusted_peers().unwrap_or_default().is_empty();
                if current.as_deref() != Some(root.as_str()) {
                    if meshed {
                        return Err(ErrorCode::Protocol);
                    }
                    ctx.adopt = Some(root.clone());
                }
                Ok(())
            }
            Action::PersistTrust => {
                let now = wobook_core::now_ms();
                let device = ctx.peer_key.device_id();
                self.store
                    .trust_peer(&ctx.peer_key, &ctx.peer_name, &ctx.peer_platform, now)
                    .map_err(|_| ErrorCode::Protocol)?;
                store_endpoints(&self.store, device, ctx.peer_endpoints.iter());
                let _ =
                    self.store
                        .upsert_endpoint(device, remote, EndpointKind::for_address(&remote));
                if let Some(PairMessage::Provision { devices, group, .. }) = &ctx.provision {
                    if let Some(g) = group.decode() {
                        let _ = self.store.set_discovery_group(&g);
                    }
                    for d in devices {
                        let Some(key) = decode_device(d) else {
                            continue;
                        };
                        if key.device_id() == self.identity.id() {
                            continue;
                        }
                        let _ = self.store.trust_announced(&key, &d.name, &d.platform, now);
                        store_endpoints(&self.store, key.device_id(), d.endpoints.iter());
                    }
                }
                Ok(())
            }
            Action::Fail(code) => {
                let _ = code;
                Ok(())
            }
        }
    }
}

impl PairingManager {
    /// A device record for announcing `device` to other peers.
    #[must_use]
    pub fn device_record(&self, device: DeviceId) -> Option<DeviceWire> {
        let peer = self.inner.store.peer(device).ok()??;
        Some(device_wire(
            &self.inner.store,
            &peer.public_key,
            &peer.name,
            &peer.platform,
        ))
    }
}
