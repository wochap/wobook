//! wobook domain: bookmark model, Automerge layout, read model, search,
//! metadata fetch, interchange formats and the daemon protocol.

pub mod doc;
pub mod fetch;
pub mod interchange;
pub mod model;
pub mod paths;
pub mod projection;
pub mod protocol;
pub mod search;
pub mod store;
pub mod tags;
pub mod url;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}
