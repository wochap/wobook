//! Adapts the foreign `SecureKeyStore` callback to `wobook_sync`'s trait.

use std::sync::atomic::{AtomicBool, Ordering};

use wobook_sync::identity::{IdentityError, SecureKeyStore as SyncKeyStore};
use zeroize::Zeroizing;

use crate::{SecureKeyStore, SecureStoreError};

pub struct KeyStoreAdapter {
    inner: Box<dyn SecureKeyStore>,
    corrupt: AtomicBool,
}

impl KeyStoreAdapter {
    pub fn new(inner: Box<dyn SecureKeyStore>) -> Self {
        Self {
            inner,
            corrupt: AtomicBool::new(false),
        }
    }

    /// A `load` reported corrupt material during this open.
    pub fn corrupt(&self) -> bool {
        self.corrupt.load(Ordering::Acquire)
    }

    fn map(&self, e: SecureStoreError) -> IdentityError {
        match e {
            SecureStoreError::Corrupt { reason } => {
                self.corrupt.store(true, Ordering::Release);
                IdentityError::Store(format!("corrupt: {reason}"))
            }
            SecureStoreError::Failed { reason } => IdentityError::Store(reason),
        }
    }
}

impl SyncKeyStore for KeyStoreAdapter {
    fn load(&self, kind: &str) -> Result<Option<Zeroizing<Vec<u8>>>, IdentityError> {
        self.inner
            .load(kind.to_string())
            .map(|b| b.map(Zeroizing::new))
            .map_err(|e| self.map(e))
    }
    fn store(&self, kind: &str, bytes: &[u8]) -> Result<(), IdentityError> {
        self.inner
            .store(kind.to_string(), bytes.to_vec())
            .map_err(|e| self.map(e))
    }
    fn remove(&self, kind: &str) -> Result<(), IdentityError> {
        self.inner.remove(kind.to_string()).map_err(|e| self.map(e))
    }
}
