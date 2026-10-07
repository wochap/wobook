//! Proof of possession, fingerprints and pairing rate limits (D6).

use std::{
    collections::HashMap,
    net::IpAddr,
    sync::Mutex,
    time::{Duration, Instant},
};

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

pub const PAIR_LABEL: &[u8] = b"wobook-pair-v1";
/// Invalid MACs one joiner key may send per window.
pub const TRIES_PER_KEY: u32 = 3;
/// Invalid MACs one source address (/64 for IPv6) may send per window.
pub const TRIES_PER_ADDRESS: u32 = 5;
/// Invalid MACs from anyone that burn the window.
pub const TRIES_TOTAL: u32 = 20;
/// Pairing connections per source address per minute.
pub const CONNECTIONS_PER_MINUTE: u32 = 10;
const RATE_MAX_ENTRIES: usize = 1024;

fn pair_hmac(secret: &[u8], offerer_pk: &[u8], joiner_pk: &[u8], nonce: &[u8]) -> Hmac<Sha256> {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).expect("hmac accepts any key length");
    for part in [PAIR_LABEL, offerer_pk, joiner_pk, nonce] {
        mac.update(part);
    }
    mac
}

/// `HMAC-SHA256(secret, "wobook-pair-v1" || offerer_pk || joiner_pk || nonce)`.
#[must_use]
pub fn pair_mac(secret: &[u8], offerer_pk: &[u8], joiner_pk: &[u8], nonce: &[u8]) -> Vec<u8> {
    pair_hmac(secret, offerer_pk, joiner_pk, nonce)
        .finalize()
        .into_bytes()
        .to_vec()
}

/// Constant-time MAC check.
#[must_use]
pub fn verify_pair_mac(
    secret: &[u8],
    offerer_pk: &[u8],
    joiner_pk: &[u8],
    nonce: &[u8],
    provided: &[u8],
) -> bool {
    pair_hmac(secret, offerer_pk, joiner_pk, nonce)
        .verify_slice(provided)
        .is_ok()
}

/// First 16 bytes of `sha256(pubkey)` as four groups of eight lowercase hex.
#[must_use]
pub fn fingerprint(public_key: &[u8]) -> String {
    let digest = Sha256::digest(public_key);
    digest[..16]
        .chunks(4)
        .map(hex::encode)
        .collect::<Vec<_>>()
        .join(" ")
}

/// IPv4 address, or the /64 prefix of an IPv6 address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AddressKey(u128);

impl From<IpAddr> for AddressKey {
    fn from(ip: IpAddr) -> Self {
        match ip {
            IpAddr::V4(v4) => Self(u128::from(u32::from(v4))),
            IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
                Some(v4) => Self(u128::from(u32::from(v4))),
                None => Self((u128::from(v6) & !((1u128 << 64) - 1)) | (1 << 127)),
            },
        }
    }
}

/// Failed-proof counters for one pairing window.
#[derive(Debug, Default)]
pub struct FailureCounter {
    per_key: HashMap<[u8; 32], u32>,
    per_address: HashMap<AddressKey, u32>,
    total: u32,
}

impl FailureCounter {
    /// Whether this key or address already used up its tries.
    #[must_use]
    pub fn locked(&self, key: &[u8; 32], ip: IpAddr) -> bool {
        self.per_key.get(key).copied().unwrap_or(0) >= TRIES_PER_KEY
            || self
                .per_address
                .get(&AddressKey::from(ip))
                .copied()
                .unwrap_or(0)
                >= TRIES_PER_ADDRESS
    }
    /// Counts a failure; returns true when the window is burnt.
    pub fn fail(&mut self, key: [u8; 32], ip: IpAddr) -> bool {
        *self.per_key.entry(key).or_default() += 1;
        *self.per_address.entry(AddressKey::from(ip)).or_default() += 1;
        self.total += 1;
        self.total >= TRIES_TOTAL
    }
}

/// Token bucket per source address.
pub struct PairRateLimiter {
    burst: f64,
    per_second: f64,
    buckets: Mutex<HashMap<AddressKey, Bucket>>,
}

struct Bucket {
    tokens: f64,
    updated: Instant,
}

impl Default for PairRateLimiter {
    fn default() -> Self {
        Self::new(
            CONNECTIONS_PER_MINUTE,
            Duration::from_secs(60) / CONNECTIONS_PER_MINUTE,
        )
    }
}

impl PairRateLimiter {
    #[must_use]
    pub fn new(burst: u32, refill: Duration) -> Self {
        Self {
            burst: f64::from(burst),
            per_second: 1.0 / refill.as_secs_f64(),
            buckets: Mutex::new(HashMap::new()),
        }
    }

    fn refilled(&self, bucket: &Bucket, now: Instant) -> f64 {
        let elapsed = now.saturating_duration_since(bucket.updated).as_secs_f64();
        (bucket.tokens + elapsed * self.per_second).min(self.burst)
    }

    /// Takes one connection from the address's bucket. Fails closed when the
    /// table is full of addresses that are still limited.
    pub fn allow(&self, ip: IpAddr, now: Instant) -> bool {
        let key = AddressKey::from(ip);
        let Ok(mut buckets) = self.buckets.lock() else {
            return false;
        };
        if !buckets.contains_key(&key) && buckets.len() >= RATE_MAX_ENTRIES {
            buckets.retain(|_, bucket| self.refilled(bucket, now) < self.burst);
            if buckets.len() >= RATE_MAX_ENTRIES {
                return false;
            }
        }
        let bucket = buckets.entry(key).or_insert(Bucket {
            tokens: self.burst,
            updated: now,
        });
        let tokens = self.refilled(bucket, now);
        bucket.updated = now;
        if tokens >= 1.0 {
            bucket.tokens = tokens - 1.0;
            true
        } else {
            bucket.tokens = tokens;
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmac_vector() {
        let mac = pair_mac(&[1; 32], &[2; 32], &[3; 32], &[4; 32]);
        assert_eq!(
            hex::encode(&mac),
            "0d8d47c59e04fff5c2390aa6d4a799280b6195a81fe581066522ca045bfe2235"
        );
        assert!(verify_pair_mac(
            &[1; 32], &[2; 32], &[3; 32], &[4; 32], &mac
        ));
        assert!(!verify_pair_mac(
            &[5; 32], &[2; 32], &[3; 32], &[4; 32], &mac
        ));
    }

    #[test]
    fn fingerprint_format() {
        let fp = fingerprint(&[2; 32]);
        let groups: Vec<&str> = fp.split(' ').collect();
        assert_eq!(groups.len(), 4);
        assert!(
            groups
                .iter()
                .all(|g| g.len() == 8 && g.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
        );
    }

    #[test]
    fn failure_limits() {
        let mut c = FailureCounter::default();
        let ip: IpAddr = "10.0.0.1".parse().unwrap();
        for _ in 0..3 {
            assert!(!c.locked(&[1; 32], ip));
            c.fail([1; 32], ip);
        }
        assert!(c.locked(&[1; 32], ip));
        assert!(!c.locked(&[2; 32], "10.0.0.2".parse().unwrap()));
        c.fail([2; 32], ip);
        c.fail([2; 32], ip);
        assert!(c.locked(&[3; 32], ip));
        let a: IpAddr = "2001:db8::1".parse().unwrap();
        let b: IpAddr = "2001:db8::2".parse().unwrap();
        assert_eq!(AddressKey::from(a), AddressKey::from(b));
        let mut c = FailureCounter::default();
        let burnt = (0..20u8)
            .map(|i| c.fail([i; 32], IpAddr::from([10, 0, 0, i])))
            .last();
        assert_eq!(burnt, Some(true));
    }

    #[test]
    fn connection_rate() {
        let r = PairRateLimiter::default();
        let ip: IpAddr = "10.0.0.1".parse().unwrap();
        let now = Instant::now();
        for _ in 0..10 {
            assert!(r.allow(ip, now));
        }
        assert!(!r.allow(ip, now));
        assert!(r.allow(ip, now + Duration::from_secs(6)));
    }
}
