//! Background delivery of `AppListener` events. Callbacks run on one
//! dedicated dispatcher thread so a slow listener never stalls Rust work.

use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

use serde_json::Value;
use wobookd::Daemon;

use crate::{AppListener, DataChange, Device, PairingEvent, SyncStatus, convert};

const STATUS_POLL: Duration = Duration::from_millis(500);
const PAIRING_POLL: Duration = Duration::from_millis(250);

#[derive(Default)]
pub struct EventState {
    stopped: AtomicBool,
    /// Joiner sessions and the endpoints their payload listed.
    tried: Mutex<HashMap<String, Vec<String>>>,
}

impl EventState {
    pub fn stop(&self) {
        self.stopped.store(true, Ordering::Release);
    }
    fn stopped(&self) -> bool {
        self.stopped.load(Ordering::Acquire)
    }
    pub fn joined(&self, session: &str, tried: Vec<String>) {
        if let Ok(mut t) = self.tried.lock() {
            t.insert(session.to_string(), tried);
        }
    }
    fn tried(&self, session: &str) -> Vec<String> {
        self.tried
            .lock()
            .ok()
            .and_then(|t| t.get(session).cloned())
            .unwrap_or_default()
    }
}

enum Event {
    Data(DataChange),
    Status(SyncStatus),
    Pairing(PairingEvent),
}

pub fn spawn(
    rt: &tokio::runtime::Runtime,
    daemon: Arc<Daemon>,
    listener: Arc<dyn AppListener>,
    state: Arc<EventState>,
) {
    let (tx, rx) = mpsc::channel::<Event>();
    std::thread::Builder::new()
        .name("wobook-events".into())
        .spawn(move || {
            for event in rx {
                match event {
                    Event::Data(c) => listener.on_data_changed(c),
                    Event::Status(s) => listener.on_sync_status(s),
                    Event::Pairing(p) => listener.on_pairing_event(p),
                }
            }
        })
        .expect("spawn event thread");

    // Data changes: every read-model rebuild.
    {
        let mut changes = daemon.changes.subscribe();
        let (tx, state) = (tx.clone(), state.clone());
        rt.spawn(async move {
            while changes.changed().await.is_ok() {
                if state.stopped() {
                    break;
                }
                let revision = *changes.borrow_and_update();
                if tx.send(Event::Data(DataChange { revision })).is_err() {
                    break;
                }
            }
        });
    }

    // Sync status transitions.
    {
        let (tx, state, daemon) = (tx.clone(), state.clone(), daemon.clone());
        rt.spawn(async move {
            let mut last: Option<SyncStatus> = None;
            while !state.stopped() {
                let status =
                    convert::sync_status(&daemon.sync.sync_status(), daemon.sync.is_paused());
                if last.as_ref() != Some(&status) {
                    if tx.send(Event::Status(status.clone())).is_err() {
                        break;
                    }
                    last = Some(status);
                }
                tokio::time::sleep(STATUS_POLL).await;
            }
        });
    }

    // Pairing progress, derived from the manager's session table.
    rt.spawn(async move {
        let mut seen: HashMap<String, (String, Option<String>)> = HashMap::new();
        while !state.stopped() {
            if let Some(manager) = daemon.sync.pairing() {
                for p in manager.pending() {
                    let key = (p.stage.to_string(), p.error.map(str::to_string));
                    if seen.get(&p.session) == Some(&key) {
                        continue;
                    }
                    seen.insert(p.session.clone(), key);
                    let value = serde_json::to_value(&p).unwrap_or(Value::Null);
                    if let Some(event) = pairing_event(&daemon, &state, &value)
                        && tx.send(Event::Pairing(event)).is_err()
                    {
                        return;
                    }
                }
            }
            tokio::time::sleep(PAIRING_POLL).await;
        }
    });
}

fn pairing_event(daemon: &Daemon, state: &EventState, p: &Value) -> Option<PairingEvent> {
    let session = p["session"].as_str().unwrap_or_default().to_string();
    let stage = p["stage"].as_str().unwrap_or_default();
    let error = p["error"].as_str();
    if error == Some("unreachable") {
        return Some(PairingEvent::Unreachable {
            tried: state.tried(&session),
            session,
        });
    }
    match stage {
        "confirming" => Some(PairingEvent::ConfirmRequired {
            confirmation: convert::confirmation(p),
        }),
        "done" => {
            let name = p["peer_name"].as_str().unwrap_or_default().to_string();
            let device = daemon
                .sync
                .devices()
                .as_array()
                .and_then(|list| {
                    list.iter()
                        .filter(|d| d["name"] == name.as_str() && d["revoked"] != true)
                        .max_by_key(|d| d["paired_at_ms"].as_i64().unwrap_or(0))
                        .cloned()
                })
                .map_or_else(
                    || Device {
                        id: String::new(),
                        name: name.clone(),
                        platform: p["peer_platform"].as_str().unwrap_or_default().to_string(),
                    },
                    |d| Device {
                        id: d["id"].as_str().unwrap_or_default().to_string(),
                        name: d["name"].as_str().unwrap_or_default().to_string(),
                        platform: d["platform"].as_str().unwrap_or_default().to_string(),
                    },
                );
            Some(PairingEvent::Completed { session, device })
        }
        "failed" => Some(match error {
            Some("expired") => PairingEvent::Expired { session },
            Some("rejected") => PairingEvent::Rejected { session },
            other => PairingEvent::Failed {
                session,
                message: other.unwrap_or("pairing failed").to_string(),
            },
        }),
        _ => None,
    }
}
