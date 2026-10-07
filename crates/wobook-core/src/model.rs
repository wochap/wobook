//! Bookmark and the JSONL record shape (D10).

use serde::{Deserialize, Serialize};

/// A bookmark. Serialized form is the JSONL record used by export, the daemon
/// protocol and hooks. `tags` are normalized and sorted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Bookmark {
    pub url: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub created_ms: i64,
    #[serde(default)]
    pub updated_ms: i64,
    #[serde(default)]
    pub deleted: bool,
}

/// Lenient import record: every field optional except `url`; unknown fields
/// ignored. `fetch` is only meaningful in `pre-add` hook replacements.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Record {
    pub url: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub created_ms: Option<i64>,
    #[serde(default)]
    pub updated_ms: Option<i64>,
    #[serde(default)]
    pub deleted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fetch: Option<bool>,
}

impl Bookmark {
    /// Export form: `deleted` only present when true.
    pub fn to_export_json(&self) -> serde_json::Value {
        let mut value = serde_json::to_value(self).unwrap_or_default();
        if !self.deleted
            && let Some(obj) = value.as_object_mut()
        {
            obj.remove("deleted");
        }
        value
    }
}
