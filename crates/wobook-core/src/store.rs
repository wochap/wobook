//! Automerge storage wrapper (D5): one root document in `FilesystemStorage`.

use std::{path::Path, sync::Arc};

use async_trait::async_trait;
use automerge_repo::{
    BootstrapStatus, DocHandle, Error, FilesystemStorage, PeerId, Repo, RepoConfig,
    error::NetworkError,
    network::{NetworkEvent, NetworkTransport},
};
use bytes::Bytes;
use tokio::sync::mpsc;

/// Transport that never connects; replaced by a real one in p2p-sync.
pub struct NullTransport {
    events: std::sync::Mutex<Option<mpsc::Receiver<NetworkEvent>>>,
    _sender: mpsc::Sender<NetworkEvent>,
}

impl NullTransport {
    pub fn new() -> Arc<Self> {
        let (sender, rx) = mpsc::channel(1);
        Arc::new(Self {
            events: std::sync::Mutex::new(Some(rx)),
            _sender: sender,
        })
    }
}

#[async_trait]
impl NetworkTransport for NullTransport {
    fn take_events(&self) -> Result<mpsc::Receiver<NetworkEvent>, NetworkError> {
        self.events
            .lock()
            .map_err(|_| NetworkError::Closed)?
            .take()
            .ok_or(NetworkError::EventsAlreadyTaken)
    }
    async fn send(&self, _peer: &PeerId, _frame: Bytes) -> Result<(), NetworkError> {
        Err(NetworkError::Closed)
    }
    async fn close_peer(&self, _peer: &PeerId) -> Result<(), NetworkError> {
        Ok(())
    }
    async fn close(&self) -> Result<(), NetworkError> {
        Ok(())
    }
}

pub struct Store {
    pub repo: Repo,
    pub handle: DocHandle,
}

/// Opens the repository under `data_dir`, creating the root document with the
/// bookmark schema on first run.
pub async fn open_or_init(data_dir: &Path) -> Result<Store, Error> {
    open_with_transport(data_dir, NullTransport::new()).await
}

pub async fn open_with_transport(
    data_dir: &Path,
    transport: Arc<dyn NetworkTransport>,
) -> Result<Store, Error> {
    let storage = Arc::new(FilesystemStorage::open(data_dir).await?);
    let repo = Repo::open(storage.clone(), storage, transport, RepoConfig::default()).await?;
    let handle = match repo.bootstrap_status() {
        BootstrapStatus::Ready { root } => match repo.get(root).await? {
            Some(handle) => handle,
            None => repo.open_document(root).await?,
        },
        BootstrapStatus::NeedsDecision => repo.initialize_new().await?,
        other => {
            return Err(Error::Config(format!(
                "repository bootstrap is {other:?}; cannot open locally"
            )));
        }
    };
    handle.ready().await?;
    handle
        .change(|tx| crate::doc::ensure_schema(tx).map_err(|e| Error::Change(e.to_string())))
        .await?;
    repo.flush().await?;
    Ok(Store { repo, handle })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reopen_keeps_data() {
        let dir = tempfile::tempdir().unwrap();
        {
            let store = open_or_init(dir.path()).await.unwrap();
            store
                .handle
                .change(|tx| {
                    crate::doc::upsert(
                        tx,
                        &crate::model::Bookmark {
                            url: "https://a/".into(),
                            ..Default::default()
                        },
                    )
                    .map_err(|e| Error::Change(e.to_string()))
                })
                .await
                .unwrap();
            store.repo.flush().await.unwrap();
            store.repo.shutdown().await.unwrap();
        }
        let store = open_or_init(dir.path()).await.unwrap();
        let all = store
            .handle
            .read(|doc| crate::doc::read_all(doc).unwrap())
            .await
            .unwrap();
        assert_eq!(all.len(), 1);
    }
}
