//! wobookd: single-writer daemon owning the Automerge document and read model.

mod hooks;
mod server;
mod sync;

use std::{
    fs::{self, File, OpenOptions},
    io,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Instant,
};

use anyhow::{Context, Result};
use clap::Parser;
use fs2::FileExt;
use tokio::{
    io::{AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
    sync::{Mutex as AsyncMutex, Notify},
};
use wobook_core::{
    paths,
    projection::ReadModel,
    protocol::{self, ErrorCode, Request, Response},
    search::Searcher,
    store,
};

use crate::server::Daemon;

#[derive(Parser, Debug)]
#[command(name = "wobookd", version, about = "wobook bookmark daemon")]
struct Args {
    /// Data directory (default: $WOBOOK_DATA_DIR or $XDG_DATA_HOME/wobook).
    #[arg(long)]
    data_dir: Option<PathBuf>,
    /// Socket path (default: $WOBOOK_SOCKET or $XDG_RUNTIME_DIR/wobook/wobookd.sock).
    #[arg(long)]
    socket: Option<PathBuf>,
    /// Hooks directory (default: $WOBOOK_HOOKS_DIR or $XDG_CONFIG_HOME/wobook/hooks).
    #[arg(long)]
    hooks_dir: Option<PathBuf>,
}

fn private_dir(path: &Path) -> io::Result<()> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

fn acquire_lock(path: &Path) -> io::Result<File> {
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)?;
    lock.try_lock_exclusive().map_err(|e| {
        io::Error::new(
            e.kind(),
            format!("another wobookd holds {}: {e}", path.display()),
        )
    })?;
    Ok(lock)
}

/// Unlinks a leftover socket unless something still accepts on it.
fn clear_stale_socket(path: &Path) -> io::Result<()> {
    if fs::symlink_metadata(path).is_err() {
        return Ok(());
    }
    if std::os::unix::net::UnixStream::connect(path).is_ok() {
        return Err(io::Error::new(
            io::ErrorKind::AddrInUse,
            format!("{} is already accepting connections", path.display()),
        ));
    }
    fs::remove_file(path)
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("wobookd: {e:#}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let args = Args::parse();
    let data_dir = args.data_dir.unwrap_or_else(paths::data_dir);
    let socket = args.socket.unwrap_or_else(paths::socket_path);
    let hooks_dir = args.hooks_dir.unwrap_or_else(paths::hooks_dir);

    private_dir(&data_dir).with_context(|| format!("create {}", data_dir.display()))?;
    let lock_path = data_dir.join("daemon.lock");
    let _lock = acquire_lock(&lock_path).context("wobookd already running")?;
    if let Some(parent) = socket.parent() {
        private_dir(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    clear_stale_socket(&socket).context("cannot replace socket")?;

    // Startup order (D10): control store and identity, QUIC bind, repository,
    // then discovery and peer supervisors.
    let engine = wobook_sync::SyncEngine::open(
        &data_dir,
        &wobook_sync::identity::FileKeyStore::new(&data_dir),
    )
    .context("open sync engine")?;
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

    let listener =
        UnixListener::bind(&socket).with_context(|| format!("bind {}", socket.display()))?;
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600))?;

    let post = hooks::spawn_post_runner(hooks_dir.clone(), data_dir.clone());
    let joining = opened.joining;
    let daemon = Arc::new(Daemon {
        store: std::sync::RwLock::new((opened.repo.clone(), opened.handle.clone())),
        joining: std::sync::atomic::AtomicBool::new(joining),
        sync: engine.clone(),
        model: Mutex::new(model),
        searcher: Mutex::new(Searcher::new()),
        writes: AsyncMutex::new(()),
        data_dir: data_dir.clone(),
        socket: socket.clone(),
        hooks_dir,
        started: Instant::now(),
        post,
        shutdown: Notify::new(),
    });
    if let Err(e) = daemon.reconcile().await
        && !joining
    {
        anyhow::bail!("initial reconcile: {}", e.1);
    }
    daemon.install_store(opened);
    engine.start(Arc::new(sync::Host(Arc::downgrade(&daemon))));

    eprintln!(
        "wobookd: device {} \"{}\" sync port {}{}",
        engine.identity.id(),
        engine.device_name(),
        engine.transport.port(),
        if joining {
            " (fetching document from peers)"
        } else {
            ""
        }
    );
    eprintln!(
        "wobookd {} listening on {} (data {})",
        wobook_core::VERSION,
        socket.display(),
        data_dir.display()
    );

    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    loop {
        tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok((stream, _)) => {
                    let daemon = daemon.clone();
                    tokio::spawn(async move { serve(daemon, stream).await });
                }
                Err(e) => eprintln!("wobookd: accept failed: {e}"),
            },
            () = daemon.shutdown.notified() => break,
            _ = tokio::signal::ctrl_c() => break,
            _ = sigterm.recv() => break,
        }
    }

    eprintln!("wobookd: shutting down");
    let _ = fs::remove_file(&socket);
    daemon.repo().flush().await.context("flush")?;
    let _ = daemon.repo().shutdown().await;
    engine.shutdown().await;
    // Let in-flight replies (the shutdown response) finish writing.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    Ok(())
}

async fn serve(daemon: Arc<Daemon>, stream: UnixStream) {
    let (read, mut write) = stream.into_split();
    let mut reader = BufReader::new(read);
    let line = match protocol::read_line_capped(&mut reader, protocol::MAX_REQUEST).await {
        Ok(Some(line)) => line,
        _ => return,
    };
    let response = match serde_json::from_str::<Request>(&line) {
        Ok(request) => daemon.handle(request).await,
        Err(e) => Response::err(ErrorCode::InvalidRequest, e.to_string()),
    };
    let _ = write.write_all(response.to_line().as_bytes()).await;
    let _ = write.shutdown().await;
}
