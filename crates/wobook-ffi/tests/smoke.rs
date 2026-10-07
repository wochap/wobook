//! FFI smoke test: temp dir, in-memory key store and listener, no network.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use wobook_ffi::{
    AddRequest, AppConfig, AppListener, DataChange, ListQuery, PairingEvent, SearchQuery,
    SecureKeyStore, SecureStoreError, SyncStatus, WobookApp, WobookError,
};

#[derive(Default, Clone)]
struct MemStore {
    map: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    stores: Arc<Mutex<Vec<String>>>,
    corrupt: Arc<Mutex<bool>>,
}

impl SecureKeyStore for MemStore {
    fn load(&self, kind: String) -> Result<Option<Vec<u8>>, SecureStoreError> {
        if *self.corrupt.lock().unwrap() {
            return Err(SecureStoreError::Corrupt {
                reason: "bad tag".into(),
            });
        }
        Ok(self.map.lock().unwrap().get(&kind).cloned())
    }
    fn store(&self, kind: String, bytes: Vec<u8>) -> Result<(), SecureStoreError> {
        self.stores.lock().unwrap().push(kind.clone());
        self.map.lock().unwrap().insert(kind, bytes);
        Ok(())
    }
    fn remove(&self, kind: String) -> Result<(), SecureStoreError> {
        self.map.lock().unwrap().remove(&kind);
        Ok(())
    }
}

#[derive(Default, Clone)]
struct Listener {
    changes: Arc<Mutex<u32>>,
}

impl AppListener for Listener {
    fn on_data_changed(&self, _change: DataChange) {
        *self.changes.lock().unwrap() += 1;
    }
    fn on_sync_status(&self, _status: SyncStatus) {}
    fn on_pairing_event(&self, _event: PairingEvent) {}
}

fn config(dir: &std::path::Path) -> AppConfig {
    AppConfig {
        data_dir: dir.to_string_lossy().into_owned(),
        device_name: "Test phone".into(),
        enable_network: false,
        allow_tailnet: true,
    }
}

fn block<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(f)
}

#[test]
fn smoke() {
    // SAFETY-free env setup: tests in this file run in one process.
    unsafe_env();
    let dir = tempfile::tempdir().unwrap();
    let keys = MemStore::default();
    let listener = Listener::default();

    let app = WobookApp::open(
        config(dir.path()),
        Box::new(keys.clone()),
        Box::new(listener.clone()),
    )
    .unwrap();
    assert!(app.list(ListQuery::default()).unwrap().is_empty());
    assert_eq!(app.this_device().unwrap().name, "Test phone");
    assert_eq!(*keys.stores.lock().unwrap(), vec!["device_key".to_string()]);

    let added = block(app.add(AddRequest {
        url: "https://ui.shadcn.com/".into(),
        title: Some("shadcn/ui".into()),
        description: None,
        tags: vec!["ui library".into(), "React".into()],
        fetch: false,
        merge: false,
    }))
    .unwrap();
    assert!(added.created);
    let mut tags = added.bookmark.tags.clone();
    tags.sort();
    assert_eq!(tags, vec!["react", "ui library"]);

    let merged = block(app.add(AddRequest {
        url: "https://ui.shadcn.com".into(),
        title: None,
        description: None,
        tags: vec!["css".into()],
        fetch: false,
        merge: true,
    }))
    .unwrap();
    assert!(merged.merged);
    assert_eq!(merged.bookmark.tags.len(), 3);

    let hits = app
        .search(SearchQuery {
            query: "shcn".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(hits.len(), 1);
    let title_len = hits[0].bookmark.title.chars().count() as u32;
    assert_eq!(hits[0].title_indices.len(), 4);
    assert!(hits[0].title_indices.iter().all(|&i| i < title_len));
    assert_eq!(hits[0].display_url, "ui.shadcn.com");

    assert!(matches!(
        app.get("not a url".into()),
        Err(WobookError::InvalidUrl { .. })
    ));
    assert!(app.get("https://nope.example/".into()).unwrap().is_none());

    let offer = app.start_pairing_offer().unwrap();
    let payload: serde_json::Value = serde_json::from_str(&offer.qr_payload_json).unwrap();
    assert_eq!(payload["v"], 1);
    assert!(payload["ep"].as_array().is_some_and(|e| !e.is_empty()));
    let ahead = offer.expires_at_ms - wobook_core::now_ms();
    assert!(
        (100_000..=121_000).contains(&ahead),
        "expires in {ahead} ms"
    );
    assert!(matches!(
        app.join_pairing("hello".into()),
        Err(WobookError::InvalidRequest { .. })
    ));

    std::thread::sleep(std::time::Duration::from_millis(100));
    assert!(*listener.changes.lock().unwrap() > 0);
    app.shutdown().unwrap();
    drop(app);

    // Reopen keeps data and does not store a new key.
    let app = WobookApp::open(
        config(dir.path()),
        Box::new(keys.clone()),
        Box::new(listener.clone()),
    )
    .unwrap();
    assert_eq!(app.list(ListQuery::default()).unwrap().len(), 1);
    assert_eq!(keys.stores.lock().unwrap().len(), 1);
    app.shutdown().unwrap();
    drop(app);

    // Corrupt key material is identity loss, never a silent new identity.
    *keys.corrupt.lock().unwrap() = true;
    let err = WobookApp::open(
        config(dir.path()),
        Box::new(keys.clone()),
        Box::new(listener),
    )
    .err()
    .expect("open must fail");
    assert!(matches!(err, WobookError::IdentityLost { .. }), "{err:?}");
}

fn unsafe_env() {
    // Edition 2024: set_var is unsafe; the workspace forbids unsafe, but
    // this crate does not inherit workspace lints.
    unsafe {
        std::env::set_var("WOBOOK_DISCOVERY", "off");
        std::env::set_var("WOBOOK_SYNC_LOOPBACK", "1");
    }
}
