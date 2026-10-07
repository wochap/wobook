//! Trust store (D4): `control.sqlite`.

use std::{
    net::SocketAddr,
    os::unix::fs::PermissionsExt,
    path::Path,
    sync::{Mutex, MutexGuard},
};

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use crate::identity::{DeviceId, PublicDeviceKey};

#[derive(Debug, thiserror::Error)]
pub enum ControlError {
    #[error("control.sqlite: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("control.sqlite: {0}")]
    Io(#[from] std::io::Error),
    #[error("control.sqlite: corrupt row: {0}")]
    Corrupt(&'static str),
    #[error("control store lock poisoned")]
    Poisoned,
}

type Result<T> = std::result::Result<T, ControlError>;

pub const RETAIN_PREVIOUS_MS: i64 = 24 * 60 * 60 * 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TrustState {
    Trusted,
    Revoked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EndpointKind {
    Lan,
    Tailnet,
    Manual,
}

impl EndpointKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lan => "lan",
            Self::Tailnet => "tailnet",
            Self::Manual => "manual",
        }
    }
    fn parse(s: &str) -> Result<Self> {
        match s {
            "lan" => Ok(Self::Lan),
            "tailnet" => Ok(Self::Tailnet),
            "manual" => Ok(Self::Manual),
            _ => Err(ControlError::Corrupt("endpoint kind")),
        }
    }
    /// `tailnet` for 100.64.0.0/10, `lan` otherwise.
    #[must_use]
    pub fn for_address(address: &SocketAddr) -> Self {
        if crate::endpoints::is_tailnet_ip(address.ip()) {
            Self::Tailnet
        } else {
            Self::Lan
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeerTrustRecord {
    pub device_id: DeviceId,
    pub public_key: PublicDeviceKey,
    pub name: String,
    pub platform: String,
    pub paired_at_ms: i64,
    pub last_seen_ms: Option<i64>,
    pub last_sync_ms: Option<i64>,
    pub state: TrustState,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PeerEndpoint {
    pub address: SocketAddr,
    pub kind: EndpointKind,
    pub last_success_ms: Option<i64>,
    pub last_failure_ms: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveryGroup {
    pub epoch: u64,
    pub secret: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveryRotation {
    pub previous_epoch: u64,
    pub previous_secret: [u8; 32],
    pub retain_until_ms: i64,
}

/// Resolves the trust record of an authenticated peer key.
pub trait TrustResolver: Send + Sync + 'static {
    fn peer_trust(&self, device: DeviceId) -> std::result::Result<Option<PeerTrustRecord>, String>;
}

pub struct SqliteControlStore {
    conn: Mutex<Connection>,
}

const SCHEMA: &str = r"
CREATE TABLE IF NOT EXISTS local_identity (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    device_id TEXT NOT NULL,
    public_key BLOB NOT NULL CHECK (length(public_key) = 32),
    created_at_ms INTEGER NOT NULL
) STRICT;
CREATE TABLE IF NOT EXISTS device_name (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    name TEXT NOT NULL,
    updated_at_ms INTEGER NOT NULL
) STRICT;
CREATE TABLE IF NOT EXISTS trusted_devices (
    device_id TEXT PRIMARY KEY,
    public_key BLOB NOT NULL CHECK (length(public_key) = 32),
    name TEXT NOT NULL,
    platform TEXT NOT NULL CHECK (platform IN ('linux', 'android')),
    paired_at_ms INTEGER NOT NULL,
    last_seen_ms INTEGER,
    last_sync_ms INTEGER,
    trust_state TEXT NOT NULL CHECK (trust_state IN ('trusted', 'revoked'))
) STRICT;
CREATE TABLE IF NOT EXISTS peer_endpoints (
    device_id TEXT NOT NULL,
    address TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('lan', 'tailnet', 'manual')),
    last_success_ms INTEGER,
    last_failure_ms INTEGER,
    PRIMARY KEY (device_id, address)
) STRICT;
CREATE TABLE IF NOT EXISTS discovery_group (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    epoch INTEGER NOT NULL,
    secret BLOB NOT NULL CHECK (length(secret) = 32)
) STRICT;
CREATE TABLE IF NOT EXISTS discovery_rotation (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    previous_epoch INTEGER NOT NULL,
    previous_secret BLOB NOT NULL CHECK (length(previous_secret) = 32),
    target_epoch INTEGER NOT NULL,
    retain_until_ms INTEGER NOT NULL,
    stage TEXT NOT NULL CHECK (stage IN ('distributing', 'retaining'))
) STRICT;
CREATE TABLE IF NOT EXISTS recovery_attempts (
    root TEXT PRIMARY KEY,
    attempts INTEGER NOT NULL
) STRICT;
CREATE TABLE IF NOT EXISTS sync_port (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    port INTEGER NOT NULL CHECK (port BETWEEN 1 AND 65535)
) STRICT;
";

fn key32(bytes: Vec<u8>, what: &'static str) -> Result<[u8; 32]> {
    bytes.try_into().map_err(|_| ControlError::Corrupt(what))
}

fn hostname() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .ok()
        .or_else(|| std::env::var("HOSTNAME").ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "wobook".into())
}

impl SqliteControlStore {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn conn(&self) -> Result<MutexGuard<'_, Connection>> {
        self.conn.lock().map_err(|_| ControlError::Poisoned)
    }

    pub fn local_identity(&self) -> Result<Option<(DeviceId, PublicDeviceKey)>> {
        let row: Option<Vec<u8>> = self
            .conn()?
            .query_row(
                "SELECT public_key FROM local_identity WHERE singleton = 1",
                [],
                |r| r.get(0),
            )
            .optional()?;
        row.map(|pk| {
            let key = PublicDeviceKey::from_bytes(key32(pk, "local public key")?)
                .map_err(|_| ControlError::Corrupt("local public key"))?;
            Ok((key.device_id(), key))
        })
        .transpose()
    }

    pub fn set_local_identity(&self, key: &PublicDeviceKey, now: i64) -> Result<()> {
        self.conn()?.execute(
            "INSERT OR REPLACE INTO local_identity VALUES (1, ?1, ?2, ?3)",
            params![key.device_id().to_hex(), key.as_bytes().as_slice(), now],
        )?;
        Ok(())
    }

    /// Stored name, or the hostname when unset.
    pub fn device_name(&self) -> Result<String> {
        let name: Option<String> = self
            .conn()?
            .query_row(
                "SELECT name FROM device_name WHERE singleton = 1",
                [],
                |r| r.get(0),
            )
            .optional()?;
        Ok(name.unwrap_or_else(hostname))
    }

    pub fn set_device_name(&self, name: &str, now: i64) -> Result<()> {
        self.conn()?.execute(
            "INSERT OR REPLACE INTO device_name VALUES (1, ?1, ?2)",
            params![name, now],
        )?;
        Ok(())
    }

    /// Inserts or re-trusts a peer (pairing only).
    pub fn trust_peer(
        &self,
        key: &PublicDeviceKey,
        name: &str,
        platform: &str,
        now: i64,
    ) -> Result<()> {
        let platform = if platform == "android" {
            "android"
        } else {
            "linux"
        };
        self.conn()?.execute(
            "INSERT INTO trusted_devices
                 (device_id, public_key, name, platform, paired_at_ms, trust_state)
             VALUES (?1, ?2, ?3, ?4, ?5, 'trusted')
             ON CONFLICT(device_id) DO UPDATE SET
                 public_key = excluded.public_key, name = excluded.name,
                 platform = excluded.platform, paired_at_ms = excluded.paired_at_ms,
                 trust_state = 'trusted'",
            params![
                key.device_id().to_hex(),
                key.as_bytes().as_slice(),
                name,
                platform,
                now
            ],
        )?;
        Ok(())
    }

    /// Adds a peer announced by another trusted peer unless it is already known
    /// (a revoked row is never re-trusted this way).
    pub fn trust_announced(
        &self,
        key: &PublicDeviceKey,
        name: &str,
        platform: &str,
        now: i64,
    ) -> Result<bool> {
        if self.peer(key.device_id())?.is_some() {
            return Ok(false);
        }
        self.trust_peer(key, name, platform, now)?;
        Ok(true)
    }

    pub fn peer(&self, device: DeviceId) -> Result<Option<PeerTrustRecord>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare_cached(
            "SELECT public_key, name, platform, paired_at_ms, last_seen_ms, last_sync_ms, trust_state
             FROM trusted_devices WHERE device_id = ?1",
        )?;
        let row = stmt
            .query_row([device.to_hex()], |r| {
                Ok((
                    r.get::<_, Vec<u8>>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, Option<i64>>(4)?,
                    r.get::<_, Option<i64>>(5)?,
                    r.get::<_, String>(6)?,
                ))
            })
            .optional()?;
        row.map(|(pk, name, platform, paired, seen, sync, state)| {
            Ok(PeerTrustRecord {
                device_id: device,
                public_key: PublicDeviceKey::from_bytes(key32(pk, "peer key")?)
                    .map_err(|_| ControlError::Corrupt("peer key"))?,
                name,
                platform,
                paired_at_ms: paired,
                last_seen_ms: seen,
                last_sync_ms: sync,
                state: if state == "revoked" {
                    TrustState::Revoked
                } else {
                    TrustState::Trusted
                },
            })
        })
        .transpose()
    }

    /// All rows, trusted first, then by name.
    pub fn peers(&self) -> Result<Vec<PeerTrustRecord>> {
        let ids: Vec<String> = {
            let conn = self.conn()?;
            let mut stmt = conn.prepare(
                "SELECT device_id FROM trusted_devices ORDER BY trust_state DESC, name, device_id",
            )?;
            stmt.query_map([], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?
        };
        let mut out = Vec::new();
        for id in ids {
            let id: DeviceId = id.parse().map_err(|_| ControlError::Corrupt("device id"))?;
            if let Some(p) = self.peer(id)? {
                out.push(p);
            }
        }
        Ok(out)
    }

    pub fn trusted_peers(&self) -> Result<Vec<PeerTrustRecord>> {
        Ok(self
            .peers()?
            .into_iter()
            .filter(|p| p.state == TrustState::Trusted)
            .collect())
    }

    pub fn rename_peer(&self, device: DeviceId, name: &str) -> Result<bool> {
        Ok(self.conn()?.execute(
            "UPDATE trusted_devices SET name = ?2 WHERE device_id = ?1",
            params![device.to_hex(), name],
        )? > 0)
    }

    pub fn seen(
        &self,
        device: DeviceId,
        name: Option<&str>,
        platform: Option<&str>,
        now: i64,
    ) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE trusted_devices SET last_seen_ms = ?2,
                 name = coalesce(?3, name),
                 platform = coalesce(?4, platform)
             WHERE device_id = ?1 AND trust_state = 'trusted'",
            params![
                device.to_hex(),
                now,
                name,
                platform.filter(|p| matches!(*p, "linux" | "android"))
            ],
        )?;
        Ok(())
    }

    pub fn synced(&self, device: DeviceId, now: i64) -> Result<()> {
        self.conn()?.execute(
            "UPDATE trusted_devices SET last_sync_ms = ?2 WHERE device_id = ?1",
            params![device.to_hex(), now],
        )?;
        Ok(())
    }

    /// Marks revoked and forgets endpoints. The row is kept forever.
    pub fn revoke(&self, device: DeviceId) -> Result<bool> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let n = tx.execute(
            "UPDATE trusted_devices SET trust_state = 'revoked' WHERE device_id = ?1",
            [device.to_hex()],
        )?;
        tx.execute(
            "DELETE FROM peer_endpoints WHERE device_id = ?1",
            [device.to_hex()],
        )?;
        tx.commit()?;
        Ok(n > 0)
    }

    pub fn upsert_endpoint(
        &self,
        device: DeviceId,
        address: SocketAddr,
        kind: EndpointKind,
    ) -> Result<()> {
        // A manual entry stays manual; otherwise the newer classification wins.
        self.conn()?.execute(
            "INSERT INTO peer_endpoints (device_id, address, kind) VALUES (?1, ?2, ?3)
             ON CONFLICT(device_id, address) DO UPDATE SET
                 kind = CASE WHEN peer_endpoints.kind = 'manual' THEN 'manual' ELSE excluded.kind END",
            params![device.to_hex(), address.to_string(), kind.as_str()],
        )?;
        Ok(())
    }

    pub fn record_success(&self, device: DeviceId, address: SocketAddr, now: i64) -> Result<()> {
        self.conn()?.execute(
            "UPDATE peer_endpoints SET last_success_ms = ?3 WHERE device_id = ?1 AND address = ?2",
            params![device.to_hex(), address.to_string(), now],
        )?;
        Ok(())
    }

    pub fn record_failure(&self, device: DeviceId, address: SocketAddr, now: i64) -> Result<()> {
        self.conn()?.execute(
            "UPDATE peer_endpoints SET last_failure_ms = ?3 WHERE device_id = ?1 AND address = ?2",
            params![device.to_hex(), address.to_string(), now],
        )?;
        Ok(())
    }

    /// Endpoints ordered by most recent success.
    pub fn endpoints(&self, device: DeviceId) -> Result<Vec<PeerEndpoint>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare_cached(
            "SELECT address, kind, last_success_ms, last_failure_ms FROM peer_endpoints
             WHERE device_id = ?1
             ORDER BY last_success_ms IS NULL, last_success_ms DESC, kind = 'manual' DESC, address",
        )?;
        let rows = stmt
            .query_map([device.to_hex()], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<i64>>(2)?,
                    r.get::<_, Option<i64>>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows.into_iter()
            .map(|(a, k, s, f)| {
                Ok(PeerEndpoint {
                    address: a.parse().map_err(|_| ControlError::Corrupt("address"))?,
                    kind: EndpointKind::parse(&k)?,
                    last_success_ms: s,
                    last_failure_ms: f,
                })
            })
            .collect()
    }

    pub fn discovery_group(&self) -> Result<Option<DiscoveryGroup>> {
        let row: Option<(i64, Vec<u8>)> = self
            .conn()?
            .query_row(
                "SELECT epoch, secret FROM discovery_group WHERE singleton = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        row.map(|(epoch, secret)| {
            Ok(DiscoveryGroup {
                epoch: u64::try_from(epoch).map_err(|_| ControlError::Corrupt("epoch"))?,
                secret: key32(secret, "group secret")?,
            })
        })
        .transpose()
    }

    pub fn set_discovery_group(&self, group: &DiscoveryGroup) -> Result<()> {
        self.conn()?.execute(
            "INSERT OR REPLACE INTO discovery_group VALUES (1, ?1, ?2)",
            params![
                i64::try_from(group.epoch).unwrap_or(i64::MAX),
                group.secret.as_slice()
            ],
        )?;
        Ok(())
    }

    /// Current group, generating one on first use.
    pub fn ensure_discovery_group(&self) -> Result<DiscoveryGroup> {
        if let Some(g) = self.discovery_group()? {
            return Ok(g);
        }
        let group = DiscoveryGroup {
            epoch: 1,
            secret: rand::random(),
        };
        self.set_discovery_group(&group)?;
        Ok(group)
    }

    /// New secret and epoch, keeping the previous one for 24 hours.
    pub fn rotate_discovery_group(&self, now: i64) -> Result<DiscoveryGroup> {
        let current = self.ensure_discovery_group()?;
        let next = DiscoveryGroup {
            epoch: current.epoch + 1,
            secret: rand::random(),
        };
        self.adopt_discovery_group(&next, now)?;
        Ok(next)
    }

    /// Switches to `next` when it is newer, retaining the current secret.
    pub fn adopt_discovery_group(&self, next: &DiscoveryGroup, now: i64) -> Result<bool> {
        let current = self.ensure_discovery_group()?;
        if next.epoch <= current.epoch {
            return Ok(false);
        }
        self.conn()?.execute(
            "INSERT OR REPLACE INTO discovery_rotation VALUES (1, ?1, ?2, ?3, ?4, 'retaining')",
            params![
                i64::try_from(current.epoch).unwrap_or(i64::MAX),
                current.secret.as_slice(),
                i64::try_from(next.epoch).unwrap_or(i64::MAX),
                now + RETAIN_PREVIOUS_MS
            ],
        )?;
        self.set_discovery_group(next)?;
        Ok(true)
    }

    pub fn discovery_rotation(&self, now: i64) -> Result<Option<DiscoveryRotation>> {
        let row: Option<(i64, Vec<u8>, i64)> = self
            .conn()?
            .query_row(
                "SELECT previous_epoch, previous_secret, retain_until_ms FROM discovery_rotation
                 WHERE singleton = 1 AND retain_until_ms > ?1",
                [now],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        row.map(|(e, s, until)| {
            Ok(DiscoveryRotation {
                previous_epoch: u64::try_from(e).map_err(|_| ControlError::Corrupt("epoch"))?,
                previous_secret: key32(s, "previous secret")?,
                retain_until_ms: until,
            })
        })
        .transpose()
    }

    pub fn recovery_attempts(&self, root: &str) -> Result<u32> {
        let n: Option<i64> = self
            .conn()?
            .query_row(
                "SELECT attempts FROM recovery_attempts WHERE root = ?1",
                [root],
                |r| r.get(0),
            )
            .optional()?;
        Ok(n.and_then(|n| u32::try_from(n).ok()).unwrap_or(0))
    }

    pub fn set_recovery_attempts(&self, root: &str, attempts: u32) -> Result<()> {
        self.conn()?.execute(
            "INSERT OR REPLACE INTO recovery_attempts VALUES (?1, ?2)",
            params![root, attempts],
        )?;
        Ok(())
    }

    pub fn sync_port(&self) -> Result<Option<u16>> {
        let port: Option<i64> = self
            .conn()?
            .query_row("SELECT port FROM sync_port WHERE singleton = 1", [], |r| {
                r.get(0)
            })
            .optional()?;
        Ok(port.and_then(|p| u16::try_from(p).ok()))
    }

    pub fn set_sync_port(&self, port: u16) -> Result<()> {
        self.conn()?
            .execute("INSERT OR REPLACE INTO sync_port VALUES (1, ?1)", [port])?;
        Ok(())
    }
}

impl TrustResolver for SqliteControlStore {
    fn peer_trust(&self, device: DeviceId) -> std::result::Result<Option<PeerTrustRecord>, String> {
        self.peer(device).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::DeviceIdentity;

    #[test]
    fn trust_revoke_endpoints_rotation() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteControlStore::open(&dir.path().join("control.sqlite")).unwrap();
        let peer = DeviceIdentity::generate();
        store
            .trust_peer(&peer.public_key(), "b", "linux", 1)
            .unwrap();
        let addr: SocketAddr = "192.168.1.4:47390".parse().unwrap();
        store
            .upsert_endpoint(peer.id(), addr, EndpointKind::Lan)
            .unwrap();
        store.record_success(peer.id(), addr, 5).unwrap();
        assert_eq!(
            store.endpoints(peer.id()).unwrap()[0].last_success_ms,
            Some(5)
        );
        assert!(
            !store
                .trust_announced(&peer.public_key(), "x", "linux", 2)
                .unwrap()
        );
        assert!(store.revoke(peer.id()).unwrap());
        assert_eq!(
            store.peer(peer.id()).unwrap().unwrap().state,
            TrustState::Revoked
        );
        assert!(store.endpoints(peer.id()).unwrap().is_empty());
        let g1 = store.ensure_discovery_group().unwrap();
        let g2 = store.rotate_discovery_group(10).unwrap();
        assert_eq!(g2.epoch, g1.epoch + 1);
        let rot = store.discovery_rotation(10).unwrap().unwrap();
        assert_eq!(rot.previous_secret, g1.secret);
        assert!(
            store
                .discovery_rotation(10 + RETAIN_PREVIOUS_MS)
                .unwrap()
                .is_none()
        );
        assert!(!store.device_name().unwrap().is_empty());
    }
}
