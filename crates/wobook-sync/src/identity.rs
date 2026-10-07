//! Device identity (D3): Ed25519 keypair, `DeviceId = sha256(public_key)`,
//! seed storage behind `SecureKeyStore`.

use std::{
    fmt,
    fs::{self, OpenOptions},
    io::{self, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    str::FromStr,
    sync::Mutex,
};

use ed25519_dalek::{SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

#[derive(Debug, thiserror::Error)]
pub enum IdentityError {
    #[error("key store: {0}")]
    Store(String),
    #[error("stored key is malformed")]
    Malformed,
    #[error("invalid device id")]
    InvalidId,
}

/// `sha256(public_key)`, shown as 64 lowercase hex characters.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DeviceId([u8; 32]);

impl DeviceId {
    #[must_use]
    pub fn from_public_key(public_key: &[u8; 32]) -> Self {
        Self(Sha256::digest(public_key).into())
    }
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
    #[must_use]
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }
    /// Constant-time comparison for authentication paths.
    #[must_use]
    pub fn constant_time_eq(&self, other: &Self) -> bool {
        self.0
            .iter()
            .zip(other.0.iter())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
    }
}

impl fmt::Display for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl fmt::Debug for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DeviceId({})", &self.to_hex()[..12])
    }
}

impl FromStr for DeviceId {
    type Err = IdentityError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != 64 || !s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
            return Err(IdentityError::InvalidId);
        }
        let mut out = [0u8; 32];
        hex::decode_to_slice(s, &mut out).map_err(|_| IdentityError::InvalidId)?;
        Ok(Self(out))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PublicDeviceKey([u8; 32]);

impl PublicDeviceKey {
    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self, IdentityError> {
        VerifyingKey::from_bytes(&bytes).map_err(|_| IdentityError::Malformed)?;
        Ok(Self(bytes))
    }
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
    #[must_use]
    pub fn device_id(&self) -> DeviceId {
        DeviceId::from_public_key(&self.0)
    }
    #[must_use]
    pub fn constant_time_eq(&self, other: &Self) -> bool {
        self.0
            .iter()
            .zip(other.0.iter())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
    }
}

impl fmt::Debug for PublicDeviceKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PublicDeviceKey({})", hex::encode(&self.0[..6]))
    }
}

pub struct PrivateDeviceKey(SigningKey);

impl PrivateDeviceKey {
    #[must_use]
    pub fn generate() -> Self {
        Self(SigningKey::generate(&mut OsRng))
    }
    #[must_use]
    pub fn from_seed(seed: &[u8; 32]) -> Self {
        Self(SigningKey::from_bytes(seed))
    }
    #[must_use]
    pub fn seed(&self) -> Zeroizing<[u8; 32]> {
        Zeroizing::new(self.0.to_bytes())
    }
    #[must_use]
    pub const fn signing_key(&self) -> &SigningKey {
        &self.0
    }
    #[must_use]
    pub fn public_key(&self) -> PublicDeviceKey {
        PublicDeviceKey(self.0.verifying_key().to_bytes())
    }
}

impl fmt::Debug for PrivateDeviceKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PrivateDeviceKey([REDACTED])")
    }
}

#[derive(Debug)]
pub struct DeviceIdentity {
    id: DeviceId,
    public_key: PublicDeviceKey,
    private_key: PrivateDeviceKey,
}

impl DeviceIdentity {
    #[must_use]
    pub fn from_private(private_key: PrivateDeviceKey) -> Self {
        let public_key = private_key.public_key();
        Self {
            id: public_key.device_id(),
            public_key,
            private_key,
        }
    }
    #[must_use]
    pub fn generate() -> Self {
        Self::from_private(PrivateDeviceKey::generate())
    }
    #[must_use]
    pub const fn id(&self) -> DeviceId {
        self.id
    }
    #[must_use]
    pub const fn public_key(&self) -> PublicDeviceKey {
        self.public_key
    }
    #[must_use]
    pub const fn private_key(&self) -> &PrivateDeviceKey {
        &self.private_key
    }
}

/// Secret kinds kept behind a `SecureKeyStore`.
pub const KIND_DEVICE_KEY: &str = "device_key";
pub const KIND_DISCOVERY_SECRET: &str = "discovery_secret";
pub const KIND_DISCOVERY_SECRET_PREV: &str = "discovery_secret_prev";

/// Where secrets live, by kind (`device_key`, `discovery_secret`,
/// `discovery_secret_prev`). Linux uses files; Android supplies a
/// Keystore-backed one through the FFI.
pub trait SecureKeyStore: Send + Sync + 'static {
    fn load(&self, kind: &str) -> Result<Option<Zeroizing<Vec<u8>>>, IdentityError>;
    fn store(&self, kind: &str, bytes: &[u8]) -> Result<(), IdentityError>;
    fn remove(&self, kind: &str) -> Result<(), IdentityError>;

    /// The 32-byte identity seed, if stored.
    fn load_seed(&self) -> Result<Option<Zeroizing<[u8; 32]>>, IdentityError> {
        match self.load(KIND_DEVICE_KEY)? {
            None => Ok(None),
            Some(bytes) => {
                let seed: [u8; 32] = bytes
                    .as_slice()
                    .try_into()
                    .map_err(|_| IdentityError::Malformed)?;
                Ok(Some(Zeroizing::new(seed)))
            }
        }
    }

    /// Loads the identity, creating and storing one when none exists.
    fn load_or_create(&self) -> Result<(DeviceIdentity, bool), IdentityError> {
        if let Some(seed) = self.load_seed()? {
            return Ok((
                DeviceIdentity::from_private(PrivateDeviceKey::from_seed(&seed)),
                false,
            ));
        }
        let key = PrivateDeviceKey::generate();
        self.store(KIND_DEVICE_KEY, key.seed().as_slice())?;
        Ok((DeviceIdentity::from_private(key), true))
    }
}

#[derive(Default)]
pub struct InMemorySecureKeyStore(Mutex<std::collections::HashMap<String, Zeroizing<Vec<u8>>>>);

impl SecureKeyStore for InMemorySecureKeyStore {
    fn load(&self, kind: &str) -> Result<Option<Zeroizing<Vec<u8>>>, IdentityError> {
        Ok(self
            .0
            .lock()
            .map_err(|_| IdentityError::Store("poisoned".into()))?
            .get(kind)
            .cloned())
    }
    fn store(&self, kind: &str, bytes: &[u8]) -> Result<(), IdentityError> {
        self.0
            .lock()
            .map_err(|_| IdentityError::Store("poisoned".into()))?
            .insert(kind.to_string(), Zeroizing::new(bytes.to_vec()));
        Ok(())
    }
    fn remove(&self, kind: &str) -> Result<(), IdentityError> {
        self.0
            .lock()
            .map_err(|_| IdentityError::Store("poisoned".into()))?
            .remove(kind);
        Ok(())
    }
}

/// Files under `<data_dir>`, mode 0600: `identity.key` for the device key
/// (raw 32-byte seed), `<kind>.key` for anything else.
pub struct FileKeyStore {
    dir: PathBuf,
}

impl FileKeyStore {
    #[must_use]
    pub fn new(data_dir: &Path) -> Self {
        Self {
            dir: data_dir.to_path_buf(),
        }
    }
    /// Path of the identity seed.
    #[must_use]
    pub fn path(&self) -> PathBuf {
        self.path_for(KIND_DEVICE_KEY)
    }
    fn path_for(&self, kind: &str) -> PathBuf {
        if kind == KIND_DEVICE_KEY {
            self.dir.join("identity.key")
        } else {
            self.dir.join(format!("{kind}.key"))
        }
    }
}

impl SecureKeyStore for FileKeyStore {
    fn load(&self, kind: &str) -> Result<Option<Zeroizing<Vec<u8>>>, IdentityError> {
        let path = self.path_for(kind);
        match fs::read(&path) {
            Ok(bytes) => Ok(Some(Zeroizing::new(bytes))),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(IdentityError::Store(format!("{}: {e}", path.display()))),
        }
    }
    fn store(&self, kind: &str, bytes: &[u8]) -> Result<(), IdentityError> {
        let path = self.path_for(kind);
        let err = |e: io::Error| IdentityError::Store(format!("{}: {e}", path.display()));
        let tmp = path.with_extension("key.tmp");
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(&tmp)
            .map_err(err)?;
        file.write_all(bytes).map_err(err)?;
        file.sync_all().map_err(err)?;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600)).map_err(err)?;
        fs::rename(&tmp, &path).map_err(err)
    }
    fn remove(&self, kind: &str) -> Result<(), IdentityError> {
        let path = self.path_for(kind);
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(IdentityError::Store(format!("{}: {e}", path.display()))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_store_round_trip_and_mode() {
        let dir = tempfile::tempdir().unwrap();
        let store = FileKeyStore::new(dir.path());
        let (a, created) = store.load_or_create().unwrap();
        assert!(created);
        let (b, created) = store.load_or_create().unwrap();
        assert!(!created);
        assert_eq!(a.id(), b.id());
        let mode = fs::metadata(store.path()).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        assert_eq!(a.id().to_hex().len(), 64);
        assert_eq!(a.id().to_hex().parse::<DeviceId>().unwrap(), a.id());
    }
}
