//! wobook-ffi: UniFFI surface over the wobookd `Daemon` (wobook-core +
//! wobook-sync) for the Android app. One `WobookApp` per data directory owns
//! the repository, read model, identity and QUIC transport.

uniffi::setup_scaffolding!();

mod convert;
mod events;
mod keystore;

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use serde_json::Value;
use wobook_core::protocol::{ErrorCode, Request, Response};
use wobookd::Daemon;

pub use convert::display_url;
use keystore::KeyStoreAdapter;

// ---------------------------------------------------------------------------
// Errors

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum WobookError {
    #[error("{reason}")]
    InvalidUrl { reason: String },
    #[error("{reason}")]
    InvalidRequest { reason: String },
    #[error("{reason}")]
    NotFound { reason: String },
    #[error("{reason}")]
    Exists { reason: String },
    #[error("{reason}")]
    Io { reason: String },
    #[error("{reason}")]
    IdentityLost { reason: String },
    #[error("{reason}")]
    Pairing { reason: String },
    #[error("{reason}")]
    UnknownDevice { reason: String },
    #[error("{reason}")]
    Closed { reason: String },
    #[error("{reason}")]
    Internal { reason: String },
}

impl WobookError {
    fn internal(e: impl std::fmt::Display) -> Self {
        Self::Internal {
            reason: e.to_string(),
        }
    }
}

fn from_response(code: ErrorCode, reason: String) -> WobookError {
    match code {
        ErrorCode::InvalidUrl => WobookError::InvalidUrl { reason },
        ErrorCode::InvalidRequest | ErrorCode::HookRejected => {
            WobookError::InvalidRequest { reason }
        }
        ErrorCode::NotFound => WobookError::NotFound { reason },
        ErrorCode::Exists => WobookError::Exists { reason },
        ErrorCode::Io => WobookError::Io { reason },
        ErrorCode::UnknownDevice | ErrorCode::DeviceRevoked => {
            WobookError::UnknownDevice { reason }
        }
        ErrorCode::PairWindowClosed | ErrorCode::PairRateLimited | ErrorCode::PairPendingMissing => {
            WobookError::Pairing { reason }
        }
        _ => WobookError::Internal { reason },
    }
}

/// Error a `SecureKeyStore` implementation reports.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum SecureStoreError {
    /// Stored material cannot be decrypted (Keystore key lost, tampering).
    #[error("{reason}")]
    Corrupt { reason: String },
    #[error("{reason}")]
    Failed { reason: String },
}

impl From<uniffi::UnexpectedUniFFICallbackError> for SecureStoreError {
    fn from(e: uniffi::UnexpectedUniFFICallbackError) -> Self {
        Self::Failed { reason: e.reason }
    }
}

// ---------------------------------------------------------------------------
// Callback interfaces

/// Secret storage by kind: `device_key`, `discovery_secret`,
/// `discovery_secret_prev`. Android implements it with the Keystore.
#[uniffi::export(callback_interface)]
pub trait SecureKeyStore: Send + Sync {
    fn load(&self, kind: String) -> Result<Option<Vec<u8>>, SecureStoreError>;
    fn store(&self, kind: String, bytes: Vec<u8>) -> Result<(), SecureStoreError>;
    fn remove(&self, kind: String) -> Result<(), SecureStoreError>;
}

/// Events from background threads. Implementations must return quickly.
#[uniffi::export(callback_interface)]
pub trait AppListener: Send + Sync {
    fn on_data_changed(&self, change: DataChange);
    fn on_sync_status(&self, status: SyncStatus);
    fn on_pairing_event(&self, event: PairingEvent);
}

// ---------------------------------------------------------------------------
// Records

#[derive(Debug, Clone, uniffi::Record)]
pub struct AppConfig {
    pub data_dir: String,
    /// Applied when non-empty.
    pub device_name: String,
    /// Start with networking on (foreground).
    pub enable_network: bool,
    pub allow_tailnet: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Bookmark {
    pub url: String,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub created_ms: i64,
    pub updated_ms: i64,
    pub deleted: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct DataChange {
    /// Monotonic counter of read-model rebuilds.
    pub revision: u64,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct AddRequest {
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub tags: Vec<String>,
    /// Fetch title/description when no title is given.
    pub fetch: bool,
    /// Merge into an existing bookmark instead of failing with `Exists`.
    pub merge: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FetchOutcome {
    Ok,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct AddResult {
    pub bookmark: Bookmark,
    pub created: bool,
    pub merged: bool,
    pub restored: bool,
    pub fetch: FetchOutcome,
}

#[derive(Debug, Clone, Default, uniffi::Record)]
pub struct UpdateRequest {
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    pub add_tags: Option<Vec<String>>,
    pub remove_tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, uniffi::Record)]
pub struct ListQuery {
    pub tags: Vec<String>,
    pub include_deleted: bool,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Default, uniffi::Record)]
pub struct SearchQuery {
    pub query: String,
    pub tags: Vec<String>,
    pub include_deleted: bool,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct Hit {
    pub bookmark: Bookmark,
    pub score: u32,
    /// Char offsets of matched characters within `bookmark.title`.
    pub title_indices: Vec<u32>,
    /// Char offsets of matched characters within `display_url`.
    pub url_indices: Vec<u32>,
    /// Host + path form shown in rows (see `display_url`).
    pub display_url: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct TagCount {
    pub tag: String,
    pub count: i64,
}

#[derive(Debug, Clone, Default, uniffi::Record)]
pub struct Metadata {
    pub title: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum InterchangeFormat {
    Jsonl,
    Netscape,
    Buku,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ImportReport {
    pub added: u64,
    pub merged: u64,
    pub skipped: u64,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Reachability {
    Lan,
    Tailnet,
    Unreachable,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub platform: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct DeviceView {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub reachability: Reachability,
    pub last_synced_ms: Option<i64>,
    pub syncing: bool,
    pub revoked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum SyncState {
    UpToDate,
    Syncing { device_name: String },
    NoPeerReachable,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SyncStatus {
    pub state: SyncState,
    pub last_sync_ms: Option<i64>,
    pub peer_count: u32,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct PairingOffer {
    pub qr_payload_json: String,
    pub expires_at_ms: i64,
    pub device_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PairingConfirmation {
    pub id: String,
    pub peer_name: String,
    pub peer_platform: String,
    pub fingerprint_groups: Vec<String>,
}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum PairingEvent {
    Connecting { session: String, peer: String, via: String, address: String },
    ConfirmRequired { confirmation: PairingConfirmation },
    Completed { session: String, device: Device },
    Expired { session: String },
    Rejected { session: String },
    Unreachable { session: String, tried: Vec<String> },
    Failed { session: String, message: String },
}

// ---------------------------------------------------------------------------
// Free functions

/// Normalizes a URL with the same rules as the daemon.
#[uniffi::export]
pub fn normalize_url(input: String) -> Result<String, WobookError> {
    wobook_core::url::normalize(&input)
        .map(|u| u.into_string())
        .map_err(|e| WobookError::InvalidUrl {
            reason: e.to_string(),
        })
}

/// Parses comma-separated tags into normalized, deduplicated tags.
#[uniffi::export]
pub fn parse_tags(input: String) -> Vec<String> {
    wobook_core::tags::parse(&input)
        .into_iter()
        .map(|t| t.as_str().to_string())
        .collect()
}

/// Host + path form used in result rows.
#[uniffi::export(name = "displayUrl")]
pub fn display_url_ffi(url: String) -> String {
    display_url(&url)
}

// ---------------------------------------------------------------------------
// The app object

#[derive(uniffi::Object)]
pub struct WobookApp {
    rt: tokio::runtime::Runtime,
    daemon: Mutex<Option<Arc<Daemon>>>,
    listener: Arc<dyn AppListener>,
    events: Arc<events::EventState>,
}

impl WobookApp {
    fn daemon(&self) -> Result<Arc<Daemon>, WobookError> {
        self.daemon
            .lock()
            .map_err(WobookError::internal)?
            .clone()
            .ok_or_else(|| WobookError::Closed {
                reason: "app is shut down".into(),
            })
    }

    /// Dispatches a daemon request on the internal runtime.
    async fn call(&self, request: Request) -> Result<Value, WobookError> {
        let daemon = self.daemon()?;
        let response = self
            .rt
            .spawn(async move { daemon.handle(request).await })
            .await
            .map_err(WobookError::internal)?;
        unwrap(response)
    }

    /// Blocking dispatch for fast read-model queries.
    fn call_blocking(&self, request: Request) -> Result<Value, WobookError> {
        let daemon = self.daemon()?;
        unwrap(self.rt.block_on(async move { daemon.handle(request).await }))
    }

    fn sync_status_now(&self) -> Result<SyncStatus, WobookError> {
        let daemon = self.daemon()?;
        Ok(convert::sync_status(
            &daemon.sync.sync_status(),
            daemon.sync.is_paused(),
        ))
    }
}

fn unwrap(response: Response) -> Result<Value, WobookError> {
    if response.ok {
        Ok(response.result.unwrap_or(Value::Null))
    } else {
        let body = response.error.ok_or_else(|| WobookError::Internal {
            reason: "error without body".into(),
        })?;
        Err(from_response(body.code, body.message))
    }
}

fn parse<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, WobookError> {
    serde_json::from_value(value).map_err(WobookError::internal)
}

const ORIGIN: &str = "android";

#[uniffi::export]
impl WobookApp {
    /// Opens (or initializes) the library under `config.data_dir`. Blocking;
    /// call from a background thread.
    #[uniffi::constructor]
    pub fn open(
        config: AppConfig,
        key_store: Box<dyn SecureKeyStore>,
        listener: Box<dyn AppListener>,
    ) -> Result<Arc<Self>, WobookError> {
        wobook_sync::pairing::manager::set_platform("android");
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("wobook")
            .enable_all()
            .build()
            .map_err(WobookError::internal)?;
        let data_dir = PathBuf::from(&config.data_dir);
        std::fs::create_dir_all(&data_dir).map_err(|e| WobookError::Io {
            reason: format!("{}: {e}", data_dir.display()),
        })?;
        let adapter = KeyStoreAdapter::new(key_store);
        let options = wobookd::OpenOptions {
            data_dir: data_dir.clone(),
            socket: PathBuf::new(),
            hooks_dir: data_dir.join("hooks"),
        };
        let daemon = rt
            .block_on(wobookd::open(options, &adapter))
            .map_err(|e| {
                if adapter.corrupt() || is_identity_lost(&e) {
                    WobookError::IdentityLost {
                        reason: format!("{e:#}"),
                    }
                } else {
                    WobookError::internal(format!("{e:#}"))
                }
            })?;
        if !config.device_name.trim().is_empty()
            && daemon.sync.device_name() != config.device_name.trim()
        {
            let sync = daemon.sync.clone();
            let name = config.device_name.clone();
            rt.block_on(async move { sync.set_device_name(&name).await })
                .map_err(|e| WobookError::InvalidRequest {
                    reason: e.to_string(),
                })?;
        }
        {
            let sync = daemon.sync.clone();
            let enabled = config.enable_network;
            rt.block_on(async move { sync.set_network(enabled) });
        }
        let listener: Arc<dyn AppListener> = Arc::from(listener);
        let events = Arc::new(events::EventState::default());
        let app = Arc::new(Self {
            rt,
            daemon: Mutex::new(Some(daemon.clone())),
            listener: listener.clone(),
            events: events.clone(),
        });
        events::spawn(&app.rt, daemon, listener, events);
        Ok(app)
    }

    // -- bookmarks ---------------------------------------------------------

    pub async fn add(&self, request: AddRequest) -> Result<AddResult, WobookError> {
        let value = self
            .call(Request::Add {
                url: request.url,
                title: request.title,
                description: request.description,
                tags: request.tags,
                fetch: request.fetch,
                merge: request.merge,
                origin: ORIGIN.into(),
            })
            .await?;
        convert::add_result(value)
    }

    pub async fn update(&self, request: UpdateRequest) -> Result<Bookmark, WobookError> {
        let value = self
            .call(Request::Update {
                url: request.url,
                title: request.title,
                description: request.description,
                tags: request.tags,
                add_tags: request.add_tags,
                remove_tags: request.remove_tags,
                origin: ORIGIN.into(),
            })
            .await?;
        convert::bookmark(value)
    }

    /// Moves a bookmark to a new URL; returns the bookmark under `to`.
    pub async fn rename(&self, from: String, to: String) -> Result<Bookmark, WobookError> {
        let value = self
            .call(Request::Rename {
                from,
                to,
                origin: ORIGIN.into(),
            })
            .await?;
        convert::bookmark(value.get("bookmark").cloned().unwrap_or(Value::Null))
    }

    pub async fn delete(&self, url: String) -> Result<Bookmark, WobookError> {
        let value = self
            .call(Request::Delete {
                url,
                origin: ORIGIN.into(),
            })
            .await?;
        convert::bookmark(value)
    }

    pub async fn restore(&self, url: String) -> Result<Bookmark, WobookError> {
        let value = self
            .call(Request::Restore {
                url,
                origin: ORIGIN.into(),
            })
            .await?;
        convert::bookmark(value)
    }

    /// The bookmark (tombstones included), or `None` when never saved.
    pub fn get(&self, url: String) -> Result<Option<Bookmark>, WobookError> {
        match self.call_blocking(Request::Get { url }) {
            Ok(value) => convert::bookmark(value).map(Some),
            Err(WobookError::NotFound { .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn list(&self, query: ListQuery) -> Result<Vec<Bookmark>, WobookError> {
        let value = self.call_blocking(Request::List {
            tags: query.tags,
            include_deleted: query.include_deleted,
            limit: query.limit.map(|l| l as usize),
        })?;
        let list: Vec<wobook_core::model::Bookmark> = parse(value)?;
        Ok(list.into_iter().map(Bookmark::from).collect())
    }

    pub fn search(&self, query: SearchQuery) -> Result<Vec<Hit>, WobookError> {
        let value = self.call_blocking(Request::Search {
            query: query.query,
            tags: query.tags,
            include_deleted: query.include_deleted,
            limit: query.limit.map(|l| l as usize),
        })?;
        let hits: Vec<wobook_core::search::Hit> = parse(value)?;
        Ok(hits.into_iter().map(convert::hit).collect())
    }

    /// Tags by usage, most used first.
    pub fn tags(&self) -> Result<Vec<TagCount>, WobookError> {
        let value = self.call_blocking(Request::Tags)?;
        let mut tags: Vec<TagCount> = value
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|t| TagCount {
                        tag: t["tag"].as_str().unwrap_or_default().to_string(),
                        count: t["count"].as_i64().unwrap_or(0),
                    })
                    .collect()
            })
            .unwrap_or_default();
        tags.sort_by(|a, b| b.count.cmp(&a.count).then(a.tag.cmp(&b.tag)));
        Ok(tags)
    }

    /// Number of live (non-deleted) bookmarks.
    pub fn library_size(&self) -> Result<u64, WobookError> {
        let daemon = self.daemon()?;
        let (live, _) = daemon
            .model
            .lock()
            .map_err(WobookError::internal)?
            .counts()
            .map_err(WobookError::internal)?;
        Ok(u64::try_from(live).unwrap_or(0))
    }

    pub async fn fetch_metadata(&self, url: String) -> Result<Metadata, WobookError> {
        let url = normalize_url(url)?;
        let result = self
            .rt
            .spawn(async move {
                wobook_core::fetch::fetch_metadata(&url, wobook_core::fetch::FetchLimits::default())
                    .await
            })
            .await
            .map_err(WobookError::internal)?;
        match result {
            Ok(meta) => Ok(Metadata {
                title: meta.title,
                description: meta.description,
            }),
            Err(e) => Err(WobookError::Io {
                reason: e.to_string(),
            }),
        }
    }

    pub async fn import(
        &self,
        format: InterchangeFormat,
        path: String,
    ) -> Result<ImportReport, WobookError> {
        let value = self
            .call(Request::Import {
                format: Some(convert::format(format)),
                path,
            })
            .await?;
        let report: wobook_core::interchange::ImportReport = parse(value)?;
        Ok(ImportReport {
            added: report.added as u64,
            merged: report.merged as u64,
            skipped: report.skipped as u64,
            errors: report
                .errors
                .into_iter()
                .map(|e| format!("line {}: {}", e.line, e.message))
                .collect(),
        })
    }

    /// Writes the library to `path`; returns the number of records.
    pub async fn export(
        &self,
        format: InterchangeFormat,
        path: String,
        include_deleted: bool,
    ) -> Result<u64, WobookError> {
        if matches!(format, InterchangeFormat::Buku) {
            return Err(WobookError::InvalidRequest {
                reason: "cannot export to buku".into(),
            });
        }
        let value = self
            .call(Request::Export {
                format: convert::format(format),
                path: Some(path),
                include_deleted,
            })
            .await?;
        Ok(value["count"].as_u64().unwrap_or(0))
    }

    // -- devices and sync --------------------------------------------------

    pub fn this_device(&self) -> Result<Device, WobookError> {
        let daemon = self.daemon()?;
        Ok(Device {
            id: daemon.sync.identity.id().to_hex(),
            name: daemon.sync.device_name(),
            platform: "android".into(),
        })
    }

    pub async fn set_device_name(&self, name: String) -> Result<String, WobookError> {
        let value = self.call(Request::DeviceName { name: Some(name) }).await?;
        Ok(value["name"].as_str().unwrap_or_default().to_string())
    }

    /// Trusted (and revoked) peers, excluding this device.
    pub fn devices(&self) -> Result<Vec<DeviceView>, WobookError> {
        let daemon = self.daemon()?;
        Ok(convert::devices(
            &daemon.sync.devices(),
            &daemon.sync.sync_status(),
        ))
    }

    pub async fn rename_device(&self, id: String, name: String) -> Result<(), WobookError> {
        self.call(Request::DevicesRename { id, name }).await?;
        self.listener.on_data_changed(DataChange { revision: 0 });
        Ok(())
    }

    pub async fn revoke_device(&self, id: String) -> Result<(), WobookError> {
        self.call(Request::DevicesRevoke { id }).await?;
        Ok(())
    }

    pub fn sync_status(&self) -> Result<SyncStatus, WobookError> {
        self.sync_status_now()
    }

    /// Dials every known peer now.
    pub async fn sync_now(&self) -> Result<(), WobookError> {
        self.call(Request::SyncNow).await?;
        Ok(())
    }

    /// Foreground: discovery, listener and dialing on. Background: off and
    /// peer connections closed.
    pub fn set_foreground(&self, foreground: bool) -> Result<(), WobookError> {
        let daemon = self.daemon()?;
        let sync = daemon.sync.clone();
        self.rt.block_on(async move { sync.set_network(foreground) });
        let status = self.sync_status_now()?;
        self.listener.on_sync_status(status);
        Ok(())
    }

    // -- pairing -----------------------------------------------------------

    /// Opens a 120 s pairing window ("Show my QR").
    pub fn start_pairing_offer(&self) -> Result<PairingOffer, WobookError> {
        let value = self.call_blocking(Request::PairStart)?;
        let payload = value["payload"].clone();
        Ok(PairingOffer {
            qr_payload_json: payload.to_string(),
            expires_at_ms: value["expires_at"].as_i64().unwrap_or(0) * 1000,
            device_name: payload["name"].as_str().unwrap_or_default().to_string(),
        })
    }

    /// Validates a scanned or pasted payload and starts the joiner side.
    /// Returns the session id; progress arrives as `PairingEvent`s.
    pub fn join_pairing(&self, payload_json: String) -> Result<String, WobookError> {
        let payload: wobook_sync::pairing::QrPayload =
            serde_json::from_str(payload_json.trim()).map_err(|e| {
                WobookError::InvalidRequest {
                    reason: format!("not a pairing code: {e}"),
                }
            })?;
        let now_s = wobook_core::now_ms() / 1000;
        let valid = payload
            .validate(now_s)
            .map_err(|e| WobookError::InvalidRequest {
                reason: e.to_string(),
            })?;
        let value = self.call_blocking(Request::PairJoin {
            payload: serde_json::to_value(&payload).map_err(WobookError::internal)?,
        })?;
        let session = value["session"].as_str().unwrap_or_default().to_string();
        let tried: Vec<String> = valid.endpoints.iter().map(ToString::to_string).collect();
        self.events.joined(&session, tried.clone());
        if let Some(first) = valid.endpoints.first() {
            let via = if wobook_sync::endpoints::is_tailnet_ip(first.ip()) {
                "Tailscale"
            } else {
                "LAN"
            };
            self.listener.on_pairing_event(PairingEvent::Connecting {
                session: session.clone(),
                peer: valid.name.clone(),
                via: via.into(),
                address: first.to_string(),
            });
        }
        Ok(session)
    }

    pub fn pending_confirmations(&self) -> Result<Vec<PairingConfirmation>, WobookError> {
        let value = self.call_blocking(Request::PairPending)?;
        Ok(value
            .as_array()
            .map(|a| {
                a.iter()
                    .filter(|p| p["stage"] == "confirming")
                    .map(convert::confirmation)
                    .collect()
            })
            .unwrap_or_default())
    }

    pub async fn confirm_pairing(&self, id: String, trust: bool) -> Result<(), WobookError> {
        let request = if trust {
            Request::PairConfirm { session: id }
        } else {
            Request::PairReject { session: id }
        };
        self.call(request).await?;
        Ok(())
    }

    /// Flushes and stops networking. Further calls fail with `Closed`.
    pub fn shutdown(&self) -> Result<(), WobookError> {
        let daemon = self.daemon.lock().map_err(WobookError::internal)?.take();
        self.events.stop();
        if let Some(daemon) = daemon {
            self.rt
                .block_on(async move {
                    let result = wobookd::close(&daemon).await;
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    result
                })
                .map_err(WobookError::internal)?;
        }
        Ok(())
    }
}

fn is_identity_lost(e: &anyhow::Error) -> bool {
    e.chain().any(|c| {
        matches!(
            c.downcast_ref::<wobook_sync::EngineError>(),
            Some(wobook_sync::EngineError::IdentityLost(..))
        )
    })
}
