//! QR pairing (D5, D6, D7).

pub mod manager;
pub mod payload;
pub mod proof;
pub mod reducer;

pub use manager::{MeshHost, PairingEvent, PairingManager, PendingPair};
pub use payload::QrPayload;
