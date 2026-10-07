//! wobook-sync: device identity, trust store, QR pairing, QUIC transport,
//! discovery and the `SyncEngine` facade used by `wobookd`.

pub mod control;
pub mod discovery;
pub mod endpoints;
pub mod identity;
pub mod pairing;
pub mod rotation;
pub mod transport;

use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};

use automerge_repo::{PeerSyncState, Repo};
use serde::Serialize;
use serde_json::{Value, json};
use tokio::sync::{Notify, broadcast::error::RecvError};

use crate::{
    control::{EndpointKind, SqliteControlStore, TrustState},
    discovery::Discovery,
    identity::{DeviceId, DeviceIdentity, SecureKeyStore},
    pairing::{MeshHost, PairingEvent, PairingManager, manager::platform},
    rotation::{ControlMessage, GroupWire},
    transport::{CLOSE_REVOKED, QuinnTransport, SessionEvent},
};

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error(transparent)]
    Control(#[from] control::ControlError),
    #[error(transparent)]
    Identity(#[from] identity::IdentityError),
    #[error(transparent)]
    Transport(#[from] transport::TransportError),
    #[error(
        "identity.key is missing but {0} records device {1}; peers pinned that key. \
         Move the data directory away and pair again"
    )]
    IdentityLost(String, String),
    #[error("unknown_device")]
    UnknownDevice,
    #[error("device_revoked")]
    DeviceRevoked,
    #[error("{0}")]
    Invalid(String),
}

#[derive(Debug, Clone, Default, Serialize)]
struct PeerStatus {
    in_progress: bool,
    heads_equal: bool,
    last_error: Option<String>,
}

pub struct SyncEngine {
    pub identity: Arc<DeviceIdentity>,
    pub store: Arc<SqliteControlStore>,
    pub transport: Arc<QuinnTransport>,
    pairing: Mutex<Option<PairingManager>>,
    discovery: Mutex<Option<Arc<Discovery>>>,
    status: Mutex<HashMap<DeviceId, PeerStatus>>,
    supervised: Mutex<HashSet<DeviceId>>,
    wake: Arc<Notify>,
    /// Networking paused (Android background): no discovery, no dialing.
    paused: std::sync::atomic::AtomicBool,
}

/// Valid device name: non-empty after trimming, at most 64 characters.
pub fn validate_name(name: &str) -> Result<String, EngineError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 64 || name.chars().any(char::is_control) {
        return Err(EngineError::Invalid(
            "device name must be 1 to 64 printable characters".into(),
        ));
    }
    Ok(name.to_string())
}

impl SyncEngine {
    /// Opens `control.sqlite`, loads or creates the identity and binds QUIC.
    /// `WOBOOK_SYNC_PORT` forces the port; `WOBOOK_DEVICE_NAME` sets the name.
    pub fn open(data_dir: &Path, key_store: &dyn SecureKeyStore) -> Result<Arc<Self>, EngineError> {
        let control_path = data_dir.join("control.sqlite");
        let store = Arc::new(SqliteControlStore::open(&control_path)?);
        let recorded = store.local_identity()?;
        if key_store.load_seed()?.is_none()
            && let Some((id, _)) = recorded
        {
            return Err(EngineError::IdentityLost(
                control_path.display().to_string(),
                id.to_hex(),
            ));
        }
        let (identity, _) = key_store.load_or_create()?;
        let now = wobook_core::now_ms();
        if recorded.map(|(id, _)| id) != Some(identity.id()) {
            store.set_local_identity(&identity.public_key(), now)?;
        }
        if let Ok(name) = std::env::var("WOBOOK_DEVICE_NAME")
            && let Ok(name) = validate_name(&name)
        {
            store.set_device_name(&name, now)?;
        }
        store.ensure_discovery_group()?;
        let identity = Arc::new(identity);
        let fixed = std::env::var("WOBOOK_SYNC_PORT")
            .ok()
            .and_then(|p| p.parse().ok());
        let transport = QuinnTransport::bind_in_range(
            fixed,
            store.sync_port()?,
            identity.clone(),
            store.clone(),
        )?;
        store.set_sync_port(transport.port())?;
        Ok(Arc::new(Self {
            identity,
            store,
            transport,
            pairing: Mutex::new(None),
            discovery: Mutex::new(None),
            status: Mutex::new(HashMap::new()),
            supervised: Mutex::new(HashSet::new()),
            wake: Arc::new(Notify::new()),
            paused: std::sync::atomic::AtomicBool::new(false),
        }))
    }

    /// Starts pairing, discovery, control handling and peer supervisors.
    pub fn start(self: &Arc<Self>, host: Arc<dyn MeshHost>) {
        let (pairing, mut pair_events) = PairingManager::new(
            self.identity.clone(),
            self.store.clone(),
            self.transport.clone(),
            host,
        );
        *self.pairing.lock().expect("pairing lock") = Some(pairing);
        if !self.is_paused() {
            self.start_discovery();
        }
        let engine = self.clone();
        tokio::spawn(async move {
            while let Some(PairingEvent::Paired { device, announce }) = pair_events.recv().await {
                engine.supervise(device);
                engine.wake.notify_waiters();
                if announce
                    && let Some(record) = engine.pairing().and_then(|p| p.device_record(device))
                {
                    for peer in engine.transport.connected() {
                        if peer != device {
                            engine
                                .transport
                                .send_control(
                                    peer,
                                    ControlMessage::Announce {
                                        device: record.clone(),
                                    },
                                )
                                .await;
                        }
                    }
                }
            }
        });
        if let Some(mut control) = self.transport.take_control() {
            let engine = self.clone();
            tokio::spawn(async move {
                while let Some((device, message)) = control.recv().await {
                    engine.on_control(device, message).await;
                }
            });
        }
        let mut sessions = self.transport.subscribe_sessions();
        let engine = self.clone();
        tokio::spawn(async move {
            loop {
                match sessions.recv().await {
                    Ok(event) => engine.on_session(event).await,
                    Err(RecvError::Lagged(_)) => {}
                    Err(RecvError::Closed) => break,
                }
            }
        });
        for peer in self.store.trusted_peers().unwrap_or_default() {
            self.supervise(peer.device_id);
        }
    }

    fn start_discovery(&self) {
        if !discovery::enabled() {
            return;
        }
        let Ok(mut slot) = self.discovery.lock() else {
            return;
        };
        if slot.is_some() {
            return;
        }
        match Discovery::start(
            self.identity.id(),
            self.transport.port(),
            self.store.clone(),
        ) {
            Ok(d) => *slot = Some(d),
            Err(e) => eprintln!("wobook-sync: discovery disabled: {e}"),
        }
    }

    #[must_use]
    pub fn is_paused(&self) -> bool {
        self.paused.load(std::sync::atomic::Ordering::Acquire)
    }

    /// Pauses (`false`) or resumes (`true`) networking: discovery, dialing and
    /// open peer connections. The QUIC socket stays bound.
    pub fn set_network(self: &Arc<Self>, enabled: bool) {
        self.paused
            .store(!enabled, std::sync::atomic::Ordering::Release);
        if enabled {
            self.start_discovery();
            for peer in self.store.trusted_peers().unwrap_or_default() {
                self.supervise(peer.device_id);
            }
            self.wake.notify_waiters();
        } else {
            if let Some(d) = self.discovery.lock().ok().and_then(|mut d| d.take()) {
                d.shutdown();
            }
            for device in self.transport.connected() {
                self.transport
                    .close_device(device, transport::CLOSE_SHUTDOWN);
            }
        }
    }

    /// Follows the repository's per-peer sync progress. Call after every
    /// (re)open of the repository.
    pub fn watch_repo(self: &Arc<Self>, repo: &Repo) {
        let mut progress = repo.subscribe_peer_sync();
        let engine = self.clone();
        tokio::spawn(async move {
            loop {
                let snapshot = progress.borrow_and_update().clone();
                let now = wobook_core::now_ms();
                if let Ok(mut status) = engine.status.lock() {
                    for (peer, p) in &snapshot {
                        let Ok(device) = peer.as_str().parse::<DeviceId>() else {
                            continue;
                        };
                        let entry = status.entry(device).or_default();
                        let synced = p.state == PeerSyncState::Synced;
                        entry.in_progress = p.state == PeerSyncState::Syncing;
                        entry.heads_equal = synced;
                        if synced {
                            let _ = engine.store.synced(device, now);
                        }
                    }
                }
                if progress.changed().await.is_err() {
                    break;
                }
            }
        });
    }

    #[must_use]
    pub fn pairing(&self) -> Option<PairingManager> {
        self.pairing.lock().ok()?.clone()
    }

    fn set_error(&self, device: DeviceId, error: Option<String>) {
        if let Ok(mut s) = self.status.lock() {
            s.entry(device).or_default().last_error = error;
        }
    }

    /// Starts the reconnect loop for a trusted peer (once).
    pub fn supervise(self: &Arc<Self>, device: DeviceId) {
        if device == self.identity.id()
            || !self
                .supervised
                .lock()
                .map(|mut s| s.insert(device))
                .unwrap_or(false)
        {
            return;
        }
        let engine = self.clone();
        tokio::spawn(async move {
            engine.clone().supervisor(device).await;
            if let Ok(mut s) = engine.supervised.lock() {
                s.remove(&device);
            }
        });
    }

    async fn supervisor(self: Arc<Self>, device: DeviceId) {
        let mut failures: u32 = 0;
        let mut sessions = self.transport.subscribe_sessions();
        loop {
            match self.store.peer(device) {
                Ok(Some(p)) if p.state == TrustState::Trusted => {}
                _ => return,
            }
            if self.transport.is_connected(device) {
                failures = 0;
                // Wait for this peer to disconnect, or re-check periodically.
                let wait = async {
                    loop {
                        match sessions.recv().await {
                            Ok(SessionEvent::Disconnected { device: d, .. }) if d == device => {
                                break;
                            }
                            Err(RecvError::Closed) => break,
                            _ => {}
                        }
                    }
                };
                tokio::select! {
                    () = wait => {},
                    () = tokio::time::sleep(Duration::from_secs(30)) => {},
                }
                continue;
            }
            if self.is_paused() {
                tokio::select! {
                    () = self.wake.notified() => {},
                    () = tokio::time::sleep(Duration::from_secs(30)) => {},
                }
                continue;
            }
            let endpoints: Vec<SocketAddr> = self
                .store
                .endpoints(device)
                .unwrap_or_default()
                .into_iter()
                .map(|e| e.address)
                .collect();
            if !endpoints.is_empty() {
                let transport = self.transport.clone();
                let result = endpoints::race(endpoints, move |addr| {
                    let transport = transport.clone();
                    async move { transport.dial_sync(device, addr).await }
                })
                .await;
                let now = wobook_core::now_ms();
                match result {
                    Ok((addr, (), failed)) => {
                        let _ = self.store.record_success(device, addr, now);
                        for (a, _) in failed {
                            let _ = self.store.record_failure(device, a, now);
                        }
                        self.set_error(device, None);
                        failures = 0;
                        continue;
                    }
                    Err(failed) => {
                        let code = failed
                            .iter()
                            .map(|(_, e)| e.code())
                            .find(|c| *c != "unreachable")
                            .or_else(|| failed.first().map(|(_, e)| e.code()));
                        for (a, _) in &failed {
                            let _ = self.store.record_failure(device, *a, now);
                        }
                        if !self.transport.is_connected(device) {
                            self.set_error(device, code.map(str::to_string));
                        }
                    }
                }
            }
            if self.transport.is_connected(device) {
                continue;
            }
            let delay = endpoints::backoff(failures);
            failures = failures.saturating_add(1);
            tokio::select! {
                () = tokio::time::sleep(delay) => {},
                () = self.wake.notified() => { failures = 0; },
            }
        }
    }

    fn hello(&self) -> ControlMessage {
        let group = self.store.ensure_discovery_group().ok();
        ControlMessage::Hello {
            name: self.store.device_name().unwrap_or_default(),
            platform: platform().into(),
            epoch: group.as_ref().map_or(0, |g| g.epoch),
            group: group.as_ref().map(GroupWire::from),
            endpoints: endpoints::current_hints(self.transport.port())
                .iter()
                .map(ToString::to_string)
                .collect(),
        }
    }

    async fn on_session(self: &Arc<Self>, event: SessionEvent) {
        match event {
            SessionEvent::Connected {
                device,
                remote,
                inbound,
            } => {
                let now = wobook_core::now_ms();
                let remote = canonical(remote);
                if inbound {
                    let _ = self.store.upsert_endpoint(
                        device,
                        remote,
                        EndpointKind::for_address(&remote),
                    );
                    let _ = self.store.record_success(device, remote, now);
                }
                let _ = self.store.seen(device, None, None, now);
                self.set_error(device, None);
                self.supervise(device);
                self.transport.send_control(device, self.hello()).await;
            }
            SessionEvent::Disconnected { device, reason } => {
                if let Ok(mut s) = self.status.lock() {
                    let e = s.entry(device).or_default();
                    e.in_progress = false;
                    e.heads_equal = false;
                    if reason.as_deref() == Some("device_revoked") {
                        e.last_error = reason;
                    }
                }
            }
        }
    }

    fn adopt_group(&self, group: &GroupWire) -> bool {
        let Some(g) = group.decode() else {
            return false;
        };
        let adopted = self
            .store
            .adopt_discovery_group(&g, wobook_core::now_ms())
            .unwrap_or(false);
        if adopted && let Some(d) = self.discovery.lock().ok().and_then(|d| d.clone()) {
            let _ = d.refresh();
        }
        adopted
    }

    async fn on_control(self: &Arc<Self>, device: DeviceId, message: ControlMessage) {
        match message {
            ControlMessage::Hello {
                name,
                platform,
                group,
                endpoints,
                ..
            } => {
                let name = validate_name(&name).ok();
                let _ = self.store.seen(
                    device,
                    name.as_deref(),
                    Some(platform.as_str()),
                    wobook_core::now_ms(),
                );
                pairing::manager::store_endpoints(&self.store, device, endpoints.iter());
                if let Some(group) = group {
                    self.adopt_group(&group);
                }
            }
            ControlMessage::DiscoveryUpdate { group } => {
                self.adopt_group(&group);
                self.transport
                    .send_control(device, ControlMessage::DiscoveryAck { epoch: group.epoch })
                    .await;
            }
            ControlMessage::DiscoveryAck { .. } | ControlMessage::Nudge => {}
            ControlMessage::Announce { device: record } => {
                let Some(key) = pairing::manager::decode_device(&record) else {
                    return;
                };
                if key.device_id() == self.identity.id() {
                    return;
                }
                if self
                    .store
                    .trust_announced(&key, &record.name, &record.platform, wobook_core::now_ms())
                    .unwrap_or(false)
                {
                    pairing::manager::store_endpoints(
                        &self.store,
                        key.device_id(),
                        record.endpoints.iter(),
                    );
                    self.supervise(key.device_id());
                }
            }
        }
    }

    /// Dials every unconnected trusted peer now and nudges connected ones.
    pub async fn sync_now(self: &Arc<Self>) {
        for peer in self.store.trusted_peers().unwrap_or_default() {
            self.supervise(peer.device_id);
        }
        self.wake.notify_waiters();
        self.transport
            .broadcast_control(ControlMessage::Nudge)
            .await;
    }

    pub fn device_name(&self) -> String {
        self.store.device_name().unwrap_or_default()
    }

    pub async fn set_device_name(&self, name: &str) -> Result<String, EngineError> {
        let name = validate_name(name)?;
        self.store.set_device_name(&name, wobook_core::now_ms())?;
        self.transport.broadcast_control(self.hello()).await;
        Ok(name)
    }

    fn known(
        &self,
        device: DeviceId,
        allow_revoked: bool,
    ) -> Result<control::PeerTrustRecord, EngineError> {
        let peer = self.store.peer(device)?.ok_or(EngineError::UnknownDevice)?;
        if !allow_revoked && peer.state == TrustState::Revoked {
            return Err(EngineError::DeviceRevoked);
        }
        Ok(peer)
    }

    pub fn rename_device(&self, device: DeviceId, name: &str) -> Result<(), EngineError> {
        self.known(device, false)?;
        let name = validate_name(name)?;
        self.store.rename_peer(device, &name)?;
        Ok(())
    }

    pub fn add_endpoint(
        self: &Arc<Self>,
        device: DeviceId,
        address: SocketAddr,
    ) -> Result<(), EngineError> {
        self.known(device, false)?;
        self.store
            .upsert_endpoint(device, address, EndpointKind::Manual)?;
        self.supervise(device);
        self.wake.notify_waiters();
        Ok(())
    }

    /// Revokes, closes the connection, forgets endpoints and rotates the
    /// discovery secret.
    pub async fn revoke(&self, device: DeviceId) -> Result<(), EngineError> {
        self.known(device, false)?;
        self.store.revoke(device)?;
        self.transport.close_device(device, CLOSE_REVOKED);
        let group = self.store.rotate_discovery_group(wobook_core::now_ms())?;
        if let Some(d) = self.discovery.lock().ok().and_then(|d| d.clone()) {
            let _ = d.refresh();
        }
        self.transport
            .broadcast_control(ControlMessage::DiscoveryUpdate {
                group: GroupWire::from(&group),
            })
            .await;
        Ok(())
    }

    fn reachability(&self, device: DeviceId) -> &'static str {
        match self.transport.remote_address(device) {
            Some(addr) => EndpointKind::for_address(&canonical(addr)).as_str(),
            None => "unreachable",
        }
    }

    #[must_use]
    pub fn devices(&self) -> Value {
        let peers = self.store.peers().unwrap_or_default();
        Value::Array(
            peers
                .into_iter()
                .map(|p| {
                    let endpoints: Vec<Value> = self
                        .store
                        .endpoints(p.device_id)
                        .unwrap_or_default()
                        .into_iter()
                        .map(|e| {
                            json!({
                                "address": e.address.to_string(),
                                "kind": e.kind,
                                "last_success_ms": e.last_success_ms,
                            })
                        })
                        .collect();
                    json!({
                        "id": p.device_id.to_hex(),
                        "name": p.name,
                        "platform": p.platform,
                        "paired_at_ms": p.paired_at_ms,
                        "last_seen_ms": p.last_seen_ms,
                        "last_sync_ms": p.last_sync_ms,
                        "reachability": self.reachability(p.device_id),
                        "endpoints": endpoints,
                        "revoked": p.state == TrustState::Revoked,
                    })
                })
                .collect(),
        )
    }

    #[must_use]
    pub fn device_json(&self) -> Value {
        json!({
            "id": self.identity.id().to_hex(),
            "name": self.device_name(),
            "port": self.transport.port(),
        })
    }

    #[must_use]
    pub fn peer_counts(&self) -> Value {
        let trusted = self.store.trusted_peers().map(|p| p.len()).unwrap_or(0);
        json!({ "trusted": trusted, "connected": self.transport.connected().len() })
    }

    #[must_use]
    pub fn sync_status(&self) -> Value {
        let status = self.status.lock().map(|s| s.clone()).unwrap_or_default();
        let peers: Vec<Value> = self
            .store
            .peers()
            .unwrap_or_default()
            .into_iter()
            .map(|p| {
                let s = status.get(&p.device_id).cloned().unwrap_or_default();
                let connected = self.transport.is_connected(p.device_id);
                let last_error = if p.state == TrustState::Revoked {
                    Some("device_revoked".to_string())
                } else if connected {
                    None
                } else {
                    s.last_error
                };
                json!({
                    "id": p.device_id.to_hex(),
                    "name": p.name,
                    "reachability": self.reachability(p.device_id),
                    "connected": connected,
                    "in_progress": connected && s.in_progress,
                    "last_sync_ms": p.last_sync_ms,
                    "heads_equal": connected && s.heads_equal,
                    "last_error": last_error,
                    "revoked": p.state == TrustState::Revoked,
                })
            })
            .collect();
        let group = self.store.ensure_discovery_group().ok();
        json!({
            "device": self.device_json(),
            "peers": peers,
            "discovery": {
                "enabled": self.discovery.lock().is_ok_and(|d| d.is_some()),
                "epoch": group.as_ref().map(|g| g.epoch),
                "service_type": group.as_ref().map(|g| discovery::service_type(&g.secret)),
            },
        })
    }

    /// Resolves an id, unique id prefix (8+ chars) or unique case-insensitive name.
    pub fn resolve(&self, needle: &str) -> Result<DeviceId, EngineError> {
        let peers = self.store.peers()?;
        if let Ok(id) = needle.parse::<DeviceId>() {
            return peers
                .iter()
                .find(|p| p.device_id == id)
                .map(|p| p.device_id)
                .ok_or(EngineError::UnknownDevice);
        }
        let lower = needle.to_lowercase();
        let by_name: Vec<_> = peers
            .iter()
            .filter(|p| p.state == TrustState::Trusted && p.name.to_lowercase() == lower)
            .collect();
        match by_name.len() {
            1 => return Ok(by_name[0].device_id),
            n if n > 1 => {
                return Err(EngineError::Invalid(format!(
                    "{n} devices are named {needle}; use an id prefix"
                )));
            }
            _ => {}
        }
        let by_prefix: Vec<_> = peers
            .iter()
            .filter(|p| needle.len() >= 4 && p.device_id.to_hex().starts_with(&lower))
            .collect();
        match by_prefix.len() {
            1 => Ok(by_prefix[0].device_id),
            0 => Err(EngineError::UnknownDevice),
            _ => Err(EngineError::Invalid(format!(
                "id prefix {needle} is ambiguous"
            ))),
        }
    }

    pub async fn shutdown(&self) {
        if let Some(d) = self.discovery.lock().ok().and_then(|mut d| d.take()) {
            d.shutdown();
        }
        self.transport.shutdown().await;
    }
}

/// Unmaps `::ffff:a.b.c.d` to `a.b.c.d`.
#[must_use]
pub fn canonical(addr: SocketAddr) -> SocketAddr {
    match addr {
        SocketAddr::V6(v6) => match v6.ip().to_ipv4_mapped() {
            Some(v4) => SocketAddr::new(v4.into(), v6.port()),
            None => addr,
        },
        SocketAddr::V4(_) => addr,
    }
}
