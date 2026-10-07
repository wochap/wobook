//! JSON daemon responses to FFI records.

use serde_json::Value;
use wobook_core::interchange::Format;

use crate::{
    AddResult, Bookmark, DeviceView, FetchOutcome, Hit, InterchangeFormat, PairingConfirmation,
    Reachability, SyncState, SyncStatus, WobookError,
};

impl From<wobook_core::model::Bookmark> for Bookmark {
    fn from(b: wobook_core::model::Bookmark) -> Self {
        Self {
            url: b.url,
            title: b.title,
            description: b.description,
            tags: b.tags,
            created_ms: b.created_ms,
            updated_ms: b.updated_ms,
            deleted: b.deleted,
        }
    }
}

pub fn bookmark(value: Value) -> Result<Bookmark, WobookError> {
    serde_json::from_value::<wobook_core::model::Bookmark>(value)
        .map(Bookmark::from)
        .map_err(WobookError::internal)
}

pub fn add_result(value: Value) -> Result<AddResult, WobookError> {
    Ok(AddResult {
        bookmark: bookmark(value["bookmark"].clone())?,
        created: value["created"].as_bool().unwrap_or(false),
        merged: value["merged"].as_bool().unwrap_or(false),
        restored: value["restored"].as_bool().unwrap_or(false),
        fetch: match value["fetch"].as_str() {
            Some("ok") => FetchOutcome::Ok,
            Some("failed") => FetchOutcome::Failed,
            _ => FetchOutcome::Skipped,
        },
    })
}

pub fn format(f: InterchangeFormat) -> Format {
    match f {
        InterchangeFormat::Jsonl => Format::Jsonl,
        InterchangeFormat::Netscape => Format::Netscape,
        InterchangeFormat::Buku => Format::Buku,
    }
}

/// Host + path: scheme and a leading `www.` dropped, a lone trailing `/`
/// dropped. Mirrors `domain/UrlDisplay.kt`.
pub fn display_url(url: &str) -> String {
    display_parts(url).0
}

/// Display form and the char offset in `url` where it starts.
fn display_parts(url: &str) -> (String, usize) {
    let mut skip = 0usize;
    let mut rest = url;
    if let Some(i) = rest.find("://") {
        skip += rest[..i + 3].chars().count();
        rest = &rest[i + 3..];
    }
    if let Some(r) = rest.strip_prefix("www.") {
        skip += 4;
        rest = r;
    }
    let mut out = rest.to_string();
    if out.ends_with('/') && out.matches('/').count() == 1 {
        out.pop();
    }
    (out, skip)
}

pub fn hit(h: wobook_core::search::Hit) -> Hit {
    let title = h.segments.title;
    let url = h.segments.url;
    let (display, skip) = display_parts(&h.bookmark.url);
    let display_len = display.chars().count() as u32;
    let title_indices = h
        .indices
        .iter()
        .filter(|&&i| i >= title[0] && i < title[1])
        .map(|i| i - title[0])
        .collect();
    let url_indices = h
        .indices
        .iter()
        .filter(|&&i| i >= url[0] && i < url[1])
        .map(|i| i - url[0])
        .filter(|&i| i as usize >= skip)
        .map(|i| i - skip as u32)
        .filter(|&i| i < display_len)
        .collect();
    Hit {
        bookmark: h.bookmark.into(),
        score: h.score,
        title_indices,
        url_indices,
        display_url: display,
    }
}

fn reachability(value: &Value) -> Reachability {
    match value.as_str() {
        Some("lan" | "manual") => Reachability::Lan,
        Some("tailnet") => Reachability::Tailnet,
        _ => Reachability::Unreachable,
    }
}

pub fn devices(devices: &Value, status: &Value) -> Vec<DeviceView> {
    let peers = status["peers"].as_array().cloned().unwrap_or_default();
    devices
        .as_array()
        .map(|list| {
            list.iter()
                .map(|d| {
                    let id = d["id"].as_str().unwrap_or_default().to_string();
                    let syncing = peers
                        .iter()
                        .find(|p| p["id"] == d["id"])
                        .is_some_and(|p| p["in_progress"].as_bool().unwrap_or(false));
                    DeviceView {
                        name: d["name"].as_str().unwrap_or_default().to_string(),
                        platform: d["platform"].as_str().unwrap_or_default().to_string(),
                        reachability: reachability(&d["reachability"]),
                        last_synced_ms: d["last_sync_ms"].as_i64(),
                        syncing,
                        revoked: d["revoked"].as_bool().unwrap_or(false),
                        id,
                    }
                })
                .filter(|d| !d.revoked)
                .collect()
        })
        .unwrap_or_default()
}

pub fn sync_status(status: &Value, paused: bool) -> SyncStatus {
    let peers: Vec<&Value> = status["peers"]
        .as_array()
        .map(|a| a.iter().filter(|p| p["revoked"] != true).collect())
        .unwrap_or_default();
    let last_sync_ms = peers.iter().filter_map(|p| p["last_sync_ms"].as_i64()).max();
    let state = if paused {
        SyncState::Disabled
    } else if let Some(p) = peers.iter().find(|p| p["in_progress"] == true) {
        SyncState::Syncing {
            device_name: p["name"].as_str().unwrap_or_default().to_string(),
        }
    } else if peers.iter().any(|p| p["connected"] == true) {
        SyncState::UpToDate
    } else {
        SyncState::NoPeerReachable
    };
    SyncStatus {
        state,
        last_sync_ms,
        peer_count: u32::try_from(peers.len()).unwrap_or(u32::MAX),
    }
}

pub fn confirmation(p: &Value) -> PairingConfirmation {
    PairingConfirmation {
        id: p["session"].as_str().unwrap_or_default().to_string(),
        peer_name: p["peer_name"].as_str().unwrap_or_default().to_string(),
        peer_platform: p["peer_platform"].as_str().unwrap_or_default().to_string(),
        fingerprint_groups: p["fingerprint"]
            .as_str()
            .unwrap_or_default()
            .split_whitespace()
            .map(str::to_string)
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_url_strips_scheme_and_www() {
        assert_eq!(display_url("https://www.example.com/"), "example.com");
        assert_eq!(display_url("https://ui.shadcn.com/docs/"), "ui.shadcn.com/docs/");
        assert_eq!(display_url("http://a.b/c?d=1"), "a.b/c?d=1");
    }
}
