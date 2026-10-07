//! wobookd library: the single-writer `Daemon` (request dispatcher, sync
//! integration, hooks), shared by the `wobookd` binary and `wobook-ffi`.

pub mod hooks;
pub mod server;
pub mod sync;

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Instant,
};

use anyhow::{Context, Result};
use tokio::sync::{Mutex as AsyncMutex, Notify};
use wobook_core::{projection::ReadModel, search::Searcher, store};
use wobook_sync::identity::SecureKeyStore;

pub use server::{Daemon, Failure};

pub struct OpenOptions {
    pub data_dir: PathBuf,
    /// Shown in `status`; empty when there is no socket (FFI).
    pub socket: PathBuf,
    pub hooks_dir: PathBuf,
}

/// Startup order (D10): control store and identity, QUIC bind, repository,
/// read model, then discovery and peer supervisors. Returns with the engine
/// started. Must run inside a Tokio runtime.
pub async fn open(options: OpenOptions, key_store: &dyn SecureKeyStore) -> Result<Arc<Daemon>> {
    let OpenOptions {
        data_dir,
        socket,
        hooks_dir,
    } = options;
    let engine = wobook_sync::SyncEngine::open(&data_dir, key_store).context("open sync engine")?;
    let opened = store::open_with_transport(
        &data_dir,
        engine.transport.clone(),
        automerge_repo::RepoConfig::default(),
    )
    .await
    .map_err(|e| match e {
        automerge_repo::Error::Bootstrap(
            ref b @ automerge_repo::error::BootstrapError::RecoveryExhausted { .. },
        ) => anyhow::anyhow!(
            "recovery needs_attention: {b}; the quarantined copies are under {}",
            data_dir.join("quarantine").display()
        ),
        other => anyhow::anyhow!(other),
    })
    .context("open automerge store")?;
    let model = ReadModel::open_or_recreate(&data_dir.join("read-model.sqlite"))
        .context("open read model")?;

    let post = hooks::spawn_post_runner(hooks_dir.clone(), data_dir.clone());
    let joining = opened.joining;
    let (changes, _) = tokio::sync::watch::channel(0u64);
    let daemon = Arc::new(Daemon {
        store: std::sync::RwLock::new((opened.repo.clone(), opened.handle.clone())),
        joining: std::sync::atomic::AtomicBool::new(joining),
        sync: engine.clone(),
        model: Mutex::new(model),
        searcher: Mutex::new(Searcher::new()),
        writes: AsyncMutex::new(()),
        data_dir,
        socket,
        hooks_dir,
        started: Instant::now(),
        post,
        shutdown: Notify::new(),
        changes,
    });
    if let Err(e) = daemon.reconcile().await
        && !joining
    {
        anyhow::bail!("initial reconcile: {}", e.1);
    }
    daemon.install_store(opened);
    engine.start(Arc::new(sync::Host(Arc::downgrade(&daemon))));
    Ok(daemon)
}

/// Flushes the repository and stops the sync engine.
pub async fn close(daemon: &Daemon) -> Result<()> {
    daemon.repo().flush().await.context("flush")?;
    let _ = daemon.repo().shutdown().await;
    daemon.sync.shutdown().await;
    Ok(())
}
