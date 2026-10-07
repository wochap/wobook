//! QR payload v1 (D5).

use std::net::SocketAddr;

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use qrcode::{QrCode, render::unicode::Dense1x2};
use serde::{Deserialize, Serialize};

use crate::identity::DeviceId;

pub const PAYLOAD_VERSION: u32 = 1;
pub const PAIR_TTL_S: i64 = 120;
pub const CLOCK_SKEW_S: i64 = 30;
pub const MAX_ENDPOINTS: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QrPayload {
    pub v: u32,
    pub name: String,
    pub id: String,
    pub ep: Vec<String>,
    pub s: String,
    pub exp: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PayloadError {
    #[error("unsupported payload version {0}")]
    Version(u32),
    #[error("id must be 64 lowercase hex characters")]
    Id,
    #[error("payload needs 1 to 16 endpoints")]
    EndpointCount,
    #[error("endpoint {0} is not ip:port")]
    Endpoint(String),
    #[error("secret must be 32 base64url bytes")]
    Secret,
    #[error("payload expired")]
    Expired,
    #[error("payload expiry is too far in the future")]
    FarFuture,
}

/// A payload that passed validation.
#[derive(Debug, Clone)]
pub struct ValidPayload {
    pub name: String,
    pub id: DeviceId,
    pub endpoints: Vec<SocketAddr>,
    pub secret: [u8; 32],
    pub exp: i64,
}

impl QrPayload {
    #[must_use]
    pub fn build(
        name: &str,
        id: DeviceId,
        endpoints: &[SocketAddr],
        secret: &[u8; 32],
        now_s: i64,
    ) -> Self {
        Self {
            v: PAYLOAD_VERSION,
            name: name.to_string(),
            id: id.to_hex(),
            ep: endpoints
                .iter()
                .take(MAX_ENDPOINTS)
                .map(ToString::to_string)
                .collect(),
            s: URL_SAFE_NO_PAD.encode(secret),
            exp: now_s + PAIR_TTL_S,
        }
    }

    pub fn validate(&self, now_s: i64) -> Result<ValidPayload, PayloadError> {
        if self.v != PAYLOAD_VERSION {
            return Err(PayloadError::Version(self.v));
        }
        let id: DeviceId = self.id.parse().map_err(|_| PayloadError::Id)?;
        if self.ep.is_empty() || self.ep.len() > MAX_ENDPOINTS {
            return Err(PayloadError::EndpointCount);
        }
        let endpoints = self
            .ep
            .iter()
            .map(|e| e.parse().map_err(|_| PayloadError::Endpoint(e.clone())))
            .collect::<Result<Vec<SocketAddr>, _>>()?;
        let secret: [u8; 32] = URL_SAFE_NO_PAD
            .decode(self.s.trim_end_matches('='))
            .ok()
            .and_then(|b| b.try_into().ok())
            .ok_or(PayloadError::Secret)?;
        if self.exp + CLOCK_SKEW_S < now_s {
            return Err(PayloadError::Expired);
        }
        if self.exp > now_s + PAIR_TTL_S + CLOCK_SKEW_S {
            return Err(PayloadError::FarFuture);
        }
        Ok(ValidPayload {
            name: self.name.clone(),
            id,
            endpoints,
            secret,
            exp: self.exp,
        })
    }

    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// Terminal QR code (unicode half blocks).
#[must_use]
pub fn render_qr(text: &str) -> String {
    QrCode::new(text.as_bytes())
        .map(|code| code.render::<Dense1x2>().quiet_zone(true).build())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn good() -> QrPayload {
        let id = DeviceId::from_public_key(&[7; 32]);
        QrPayload::build(
            "gdesktop",
            id,
            &["192.168.1.40:47390".parse().unwrap()],
            &[9; 32],
            1000,
        )
    }

    #[test]
    fn valid_round_trip() {
        let p = good();
        let v = p.validate(1000).unwrap();
        assert_eq!(v.secret, [9; 32]);
        assert_eq!(p.exp, 1120);
        let back: QrPayload = serde_json::from_str(&p.to_json()).unwrap();
        assert_eq!(back, p);
        assert!(!render_qr(&p.to_json()).is_empty());
    }

    #[test]
    fn every_failure() {
        let mut p = good();
        p.v = 2;
        assert_eq!(p.validate(1000).unwrap_err(), PayloadError::Version(2));
        let mut p = good();
        p.id = "ABC".into();
        assert_eq!(p.validate(1000).unwrap_err(), PayloadError::Id);
        let mut p = good();
        p.id = p.id.to_uppercase();
        assert_eq!(p.validate(1000).unwrap_err(), PayloadError::Id);
        let mut p = good();
        p.ep.clear();
        assert_eq!(p.validate(1000).unwrap_err(), PayloadError::EndpointCount);
        let mut p = good();
        p.ep = vec!["1.2.3.4:1".into(); 17];
        assert_eq!(p.validate(1000).unwrap_err(), PayloadError::EndpointCount);
        let mut p = good();
        p.ep = vec!["host:1".into()];
        assert!(matches!(p.validate(1000), Err(PayloadError::Endpoint(_))));
        let mut p = good();
        p.s = URL_SAFE_NO_PAD.encode([1u8; 16]);
        assert_eq!(p.validate(1000).unwrap_err(), PayloadError::Secret);
        let p = good();
        assert!(p.validate(1120 + 30).is_ok());
        assert_eq!(p.validate(1120 + 31).unwrap_err(), PayloadError::Expired);
        assert_eq!(p.validate(800).unwrap_err(), PayloadError::FarFuture);
    }
}
