//! wobookd: single-writer daemon owning the Automerge document and read model.

use std::{
    fs::{self, File, OpenOptions},
    io,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result};
use clap::Parser;
use fs2::FileExt;
use tokio::{
    io::{AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
};
use wobook_core::{
    paths,
    protocol::{self, ErrorCode, Request, Response},
};
use wobookd::Daemon;

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

    let daemon = wobookd::open(
        wobookd::OpenOptions {
            data_dir: data_dir.clone(),
            socket: socket.clone(),
            hooks_dir,
        },
        &wobook_sync::identity::FileKeyStore::new(&data_dir),
    )
    .await?;
    let engine = daemon.sync.clone();
    let joining = daemon.joining.load(std::sync::atomic::Ordering::Acquire);
    let listener =
        UnixListener::bind(&socket).with_context(|| format!("bind {}", socket.display()))?;
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600))?;

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
    wobookd::close(&daemon).await?;
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
