//! p2p-sync integration: pairing/device/sync requests, joining a mesh (D7),
//! remote-change hooks (D10) and recovery reporting (D12).

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Weak, atomic::Ordering},
    time::Duration,
};

use async_trait::async_trait;
use automerge_repo::{ChangeOrigin, DocHandle, DocumentId, PeerId, RepoConfig};
use serde_json::{Value, json};
use wobook_core::{
    doc,
    interchange::{self, Format},
    model::Bookmark,
    now_ms,
    protocol::{ErrorCode, Request},
    store,
};
use wobook_sync::{
    EngineError,
    identity::DeviceId,
    pairing::{MeshHost, QrPayload, manager::PairError, payload::render_qr},
};

use crate::{
    hooks::PostJob,
    server::{Daemon, Failure},
};

/// Remote events arriving within this window form one sync batch.
const BATCH_WINDOW: Duration = Duration::from_millis(300);
const JOIN_WAIT: Duration = Duration::from_secs(120);

fn engine_failure(e: EngineError) -> Failure {
    let code = match e {
        EngineError::UnknownDevice => ErrorCode::UnknownDevice,
        EngineError::DeviceRevoked => ErrorCode::DeviceRevoked,
        EngineError::Invalid(_) => ErrorCode::InvalidRequest,
        _ => ErrorCode::Internal,
    };
    Failure(code, e.to_string())
}

fn pair_failure(e: PairError) -> Failure {
    let code = match e {
        PairError::WindowClosed => ErrorCode::PairWindowClosed,
        PairError::RateLimited => ErrorCode::PairRateLimited,
        PairError::PendingMissing => ErrorCode::PairPendingMissing,
        PairError::Invalid(_) => ErrorCode::InvalidRequest,
    };
    Failure(code, e.to_string())
}

/// `pair.*` needs the manager, which exists once the engine started.
fn manager(daemon: &Daemon) -> Result<wobook_sync::pairing::PairingManager, Failure> {
    daemon
        .sync
        .pairing()
        .ok_or_else(|| Failure(ErrorCode::Internal, "sync engine not started".into()))
}

impl Daemon {
    pub async fn sync_request(self: &Arc<Self>, request: Request) -> Result<Value, Failure> {
        let sync = &self.sync;
        match request {
            Request::PairStart => {
                let payload = manager(self)?.start().map_err(pair_failure)?;
                let text = payload.to_json();
                Ok(json!({
                    "payload": payload,
                    "qr_text": render_qr(&text),
                    "expires_at": payload.exp,
                }))
            }
            Request::PairJoin { payload } => {
                let payload: QrPayload = serde_json::from_value(payload)
                    .map_err(|e| Failure(ErrorCode::InvalidRequest, format!("payload: {e}")))?;
                let session = manager(self)?.join(&payload).map_err(pair_failure)?;
                Ok(json!({ "session": session }))
            }
            Request::PairPending => serde_json::to_value(manager(self)?.pending())
                .map_err(|e| Failure(ErrorCode::Internal, e.to_string())),
            Request::PairConfirm { session } => {
                manager(self)?
                    .decide(&session, true)
                    .await
                    .map_err(pair_failure)?;
                Ok(json!({ "session": session }))
            }
            Request::PairReject { session } => {
                manager(self)?
                    .decide(&session, false)
                    .await
                    .map_err(pair_failure)?;
                Ok(json!({ "session": session }))
            }
            Request::DevicesList => Ok(sync.devices()),
            Request::DevicesRename { id, name } => {
                let device = sync.resolve(&id).map_err(engine_failure)?;
                sync.rename_device(device, &name).map_err(engine_failure)?;
                Ok(json!({ "id": device.to_hex(), "name": name.trim() }))
            }
            Request::DevicesRevoke { id } => {
                let device = sync.resolve(&id).map_err(engine_failure)?;
                sync.revoke(device).await.map_err(engine_failure)?;
                Ok(json!({ "id": device.to_hex(), "revoked": true }))
            }
            Request::DevicesAddEndpoint { id, address } => {
                let device = sync.resolve(&id).map_err(engine_failure)?;
                let parsed = address.parse().map_err(|_| {
                    Failure(
                        ErrorCode::InvalidRequest,
                        format!("not an ip:port address: {address}"),
                    )
                })?;
                sync.add_endpoint(device, parsed).map_err(engine_failure)?;
                Ok(json!({ "id": device.to_hex(), "address": address }))
            }
            Request::SyncStatus => Ok(sync.sync_status()),
            Request::SyncNow => {
                sync.sync_now().await;
                Ok(json!({}))
            }
            Request::DeviceName { name: None } => Ok(json!({ "name": sync.device_name() })),
            Request::DeviceName { name: Some(name) } => {
                let name = sync.set_device_name(&name).await.map_err(engine_failure)?;
                Ok(json!({ "name": name }))
            }
            _ => Err(Failure(
                ErrorCode::InvalidRequest,
                "unsupported request".into(),
            )),
        }
    }

    /// `status.recovery`: absent when healthy.
    pub fn recovery_status(&self) -> Option<Value> {
        let repo = self.repo();
        let record = repo.recovery()?;
        if !self.joining.load(Ordering::Acquire) {
            return None;
        }
        let root = record.root().map(|r| r.to_string()).unwrap_or_default();
        let attempts = recovery_attempts_file(&self.data_dir, &root);
        let _ = self.sync.store.set_recovery_attempts(&root, attempts);
        Some(json!({
            "state": "recovering",
            "reason": format!("{:?}", record.reason),
            "attempts": attempts,
        }))
    }

    /// Installs a (re)opened repository and starts its watchers.
    pub fn install_store(self: &Arc<Self>, opened: store::Store) {
        self.joining.store(opened.joining, Ordering::Release);
        self.sync.watch_repo(&opened.repo);
        let handle = opened.handle.clone();
        *self.store.write().expect("store lock") = (opened.repo, opened.handle);
        spawn_watcher(self.clone(), handle.clone());
        if opened.joining {
            let daemon = self.clone();
            tokio::spawn(async move {
                if handle.ready().await.is_ok() {
                    daemon.joining.store(false, Ordering::Release);
                    let _ = daemon.repo().flush().await;
                    if let Err(e) = daemon.reconcile().await {
                        eprintln!("wobookd: reconcile after join failed: {}", e.1);
                    }
                }
            });
        }
    }

    /// D7: export, quarantine, join `root`, wait for the first sync, import.
    async fn adopt_root(
        self: &Arc<Self>,
        root: DocumentId,
        offerer: DeviceId,
    ) -> Result<(), String> {
        let _guard = self.writes.lock().await;
        if self.doc().id() == root {
            return Ok(());
        }
        let ts = now_ms();
        let export_path = self.data_dir.join(format!("pre-join-{ts}.jsonl"));
        let mine: Vec<Bookmark> = self
            .doc()
            .read(doc::read_all)
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter(|b| !b.deleted)
            .collect();
        std::fs::write(&export_path, interchange::write_jsonl(&mine)).map_err(|e| e.to_string())?;

        let repo = self.repo();
        repo.flush().await.map_err(|e| e.to_string())?;
        repo.shutdown().await.map_err(|e| e.to_string())?;
        quarantine_document(&self.data_dir, &format!("pre-join-{ts}"))
            .map_err(|e| e.to_string())?;

        let opened = store::join_with_transport(
            &self.data_dir,
            self.sync.transport.clone(),
            RepoConfig::default(),
            root,
        )
        .await
        .map_err(|e| e.to_string())?;
        let handle = opened.handle.clone();
        self.install_store(opened);
        self.sync.supervise(offerer);
        self.sync.sync_now().await;
        tokio::time::timeout(JOIN_WAIT, handle.ready())
            .await
            .map_err(|_| "timed out waiting for the first sync".to_string())?
            .map_err(|e| e.to_string())?;
        self.joining.store(false, Ordering::Release);
        self.reconcile().await.map_err(|e| e.1)?;
        drop(_guard);
        let report = self
            .import(Some(Format::Jsonl), &export_path.to_string_lossy())
            .await
            .map_err(|e| e.1)?;
        eprintln!(
            "wobookd: joined mesh {root}; merged {} local bookmarks ({report})",
            mine.len()
        );
        Ok(())
    }

    fn peer_name(&self, peer: &PeerId) -> String {
        peer.as_str()
            .parse::<DeviceId>()
            .ok()
            .and_then(|id| self.sync.store.peer(id).ok().flatten())
            .map_or_else(
                || peer.as_str()[..8.min(peer.as_str().len())].to_string(),
                |p| p.name,
            )
    }

    /// Reconciles after a batch of remote changes and runs per-bookmark hooks
    /// with `remote:<peer>` origins, then one `post-sync`.
    async fn apply_remote(self: &Arc<Self>, peer: PeerId) {
        let _guard = self.writes.lock().await;
        let snapshot = |d: &Daemon| -> HashMap<String, Bookmark> {
            d.model
                .lock()
                .ok()
                .and_then(|m| m.list(&[], true).ok())
                .unwrap_or_default()
                .into_iter()
                .map(|b| (b.url.clone(), b))
                .collect()
        };
        let before = snapshot(self);
        if let Err(e) = self.reconcile().await {
            eprintln!("wobookd: reconcile failed: {}", e.1);
            return;
        }
        let after = snapshot(self);
        let name = self.peer_name(&peer);
        let origin = format!("remote:{name}");
        let mut changed = 0usize;
        let mut urls: Vec<&String> = after.keys().collect();
        urls.sort();
        for url in urls {
            let now = &after[url];
            let previous = before.get(url);
            let event = match previous {
                None if now.deleted => continue,
                None => "post-add",
                Some(old) if old == now => continue,
                Some(old) if now.deleted && !old.deleted => "post-delete",
                Some(_) => "post-update",
            };
            changed += 1;
            self.post_remote(event, &origin, &name, now, previous);
        }
        if changed == 0 {
            return;
        }
        let heads = self.doc().read(doc::heads_string).await.unwrap_or_default();
        let payload = json!({
            "event": "post-sync",
            "origin": origin,
            "peer": { "id": peer.as_str(), "name": name },
            "changed": changed,
            "heads": heads,
        });
        let _ = self.post.send(PostJob {
            event: "post-sync",
            origin,
            url: String::new(),
            payload,
            peer: Some(name),
        });
    }

    fn post_remote(
        &self,
        event: &'static str,
        origin: &str,
        peer: &str,
        bookmark: &Bookmark,
        previous: Option<&Bookmark>,
    ) {
        let to_json = |b: &Bookmark| serde_json::to_value(b).unwrap_or(Value::Null);
        let payload = crate::hooks::payload(
            event,
            origin,
            &to_json(bookmark),
            previous.map(to_json).as_ref(),
        );
        let _ = self.post.send(PostJob {
            event,
            origin: origin.to_string(),
            url: bookmark.url.clone(),
            payload,
            peer: Some(peer.to_string()),
        });
    }
}

/// Follows one document handle until it closes: local events reconcile,
/// remote events are batched per peer and run hooks.
fn spawn_watcher(daemon: Arc<Daemon>, handle: DocHandle) {
    use tokio::sync::broadcast::error::RecvError;
    let mut events = handle.subscribe();
    tokio::spawn(async move {
        loop {
            let first = match events.recv().await {
                Ok(e) => Some(e),
                Err(RecvError::Lagged(_)) => None,
                Err(RecvError::Closed) => break,
            };
            let mut remote: Vec<PeerId> = Vec::new();
            if let Some(ChangeOrigin::Remote(p)) = first.map(|e| e.origin) {
                remote.push(p);
                // Collect the rest of this sync burst.
                let deadline = tokio::time::Instant::now() + BATCH_WINDOW;
                while let Ok(Ok(e)) = tokio::time::timeout_at(deadline, events.recv()).await {
                    if let ChangeOrigin::Remote(p) = e.origin
                        && !remote.contains(&p)
                    {
                        remote.push(p);
                    }
                }
            }
            if daemon.doc().id() != handle.id() {
                break;
            }
            match remote.pop() {
                Some(peer) => daemon.apply_remote(peer).await,
                None => {
                    if let Err(e) = daemon.reconcile().await {
                        eprintln!("wobookd: reconcile failed: {}", e.1);
                    }
                }
            }
        }
    });
}

fn recovery_attempts_file(data_dir: &Path, root: &str) -> u32 {
    std::fs::read_to_string(
        data_dir
            .join("control")
            .join(format!("recovery-{root}.txt")),
    )
    .ok()
    .and_then(|s| s.trim().parse().ok())
    .unwrap_or(0)
}

/// Moves the document directory and bootstrap record under
/// `quarantine/<name>/`. Nothing is deleted.
fn quarantine_document(data_dir: &Path, name: &str) -> std::io::Result<PathBuf> {
    let target = data_dir.join("quarantine").join(name);
    std::fs::create_dir_all(&target)?;
    std::fs::rename(data_dir.join("automerge"), target.join("automerge"))?;
    let control = data_dir.join("control");
    if control.exists() {
        std::fs::rename(&control, target.join("control"))?;
    }
    Ok(target)
}

/// The daemon as seen by the pairing manager.
pub struct Host(pub Weak<Daemon>);

#[async_trait]
impl MeshHost for Host {
    fn root(&self) -> Option<String> {
        Some(self.0.upgrade()?.doc().id().to_string())
    }

    async fn adopt_root(&self, root: String, offerer: DeviceId) -> Result<(), String> {
        let daemon = self.0.upgrade().ok_or("daemon stopped")?;
        let root: DocumentId = root.parse().map_err(|_| "bad root id".to_string())?;
        daemon.adopt_root(root, offerer).await
    }
}
