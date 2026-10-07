//! Daemon protocol (D9): one newline-terminated JSON request per connection,
//! one newline-terminated JSON response.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::interchange::Format;

pub const MAX_REQUEST: usize = 4 * 1024 * 1024;

fn yes() -> bool {
    true
}

fn default_origin() -> String {
    "cli".into()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    Ping,
    Add {
        url: String,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        description: Option<String>,
        #[serde(default)]
        tags: Vec<String>,
        #[serde(default = "yes")]
        fetch: bool,
        #[serde(default)]
        merge: bool,
        #[serde(default = "default_origin")]
        origin: String,
    },
    Update {
        url: String,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        description: Option<String>,
        #[serde(default)]
        tags: Option<Vec<String>>,
        #[serde(default)]
        add_tags: Option<Vec<String>>,
        #[serde(default)]
        remove_tags: Option<Vec<String>>,
        #[serde(default = "default_origin")]
        origin: String,
    },
    Rename {
        from: String,
        to: String,
        #[serde(default = "default_origin")]
        origin: String,
    },
    Delete {
        url: String,
        #[serde(default = "default_origin")]
        origin: String,
    },
    Restore {
        url: String,
        #[serde(default = "default_origin")]
        origin: String,
    },
    Get {
        url: String,
    },
    List {
        #[serde(default)]
        tags: Vec<String>,
        #[serde(default)]
        include_deleted: bool,
        #[serde(default)]
        limit: Option<usize>,
    },
    Search {
        query: String,
        #[serde(default)]
        tags: Vec<String>,
        #[serde(default)]
        include_deleted: bool,
        #[serde(default)]
        limit: Option<usize>,
    },
    Tags,
    Import {
        format: Option<Format>,
        path: String,
    },
    Export {
        format: Format,
        #[serde(default)]
        path: Option<String>,
        #[serde(default)]
        include_deleted: bool,
    },
    Status,
    Hooks,
    RunHooks {
        event: String,
        url: String,
    },
    Shutdown,
    #[serde(rename = "pair.start")]
    PairStart,
    #[serde(rename = "pair.join")]
    PairJoin {
        payload: Value,
    },
    #[serde(rename = "pair.pending")]
    PairPending,
    #[serde(rename = "pair.confirm")]
    PairConfirm {
        session: String,
    },
    #[serde(rename = "pair.reject")]
    PairReject {
        session: String,
    },
    #[serde(rename = "devices.list")]
    DevicesList,
    #[serde(rename = "devices.rename")]
    DevicesRename {
        id: String,
        name: String,
    },
    #[serde(rename = "devices.revoke")]
    DevicesRevoke {
        id: String,
    },
    #[serde(rename = "devices.add_endpoint")]
    DevicesAddEndpoint {
        id: String,
        address: String,
    },
    #[serde(rename = "sync.status")]
    SyncStatus,
    #[serde(rename = "sync.now")]
    SyncNow,
    #[serde(rename = "device.name")]
    DeviceName {
        #[serde(default)]
        name: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidRequest,
    InvalidUrl,
    NotFound,
    Exists,
    HookRejected,
    Io,
    Internal,
    PairWindowClosed,
    PairRateLimited,
    PairPendingMissing,
    UnknownDevice,
    DeviceRevoked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorBody {
    pub code: ErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorBody>,
}

impl Response {
    pub fn ok(result: Value) -> Self {
        Self {
            ok: true,
            result: Some(result),
            error: None,
        }
    }
    pub fn err(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            ok: false,
            result: None,
            error: Some(ErrorBody {
                code,
                message: message.into(),
            }),
        }
    }
    pub fn to_line(&self) -> String {
        let mut s = serde_json::to_string(self).unwrap_or_else(|_| {
            r#"{"ok":false,"error":{"code":"internal","message":"serialize"}}"#.into()
        });
        s.push('\n');
        s
    }
}

/// Reads one newline-terminated line of at most `cap` bytes. `Ok(None)` when
/// the peer sent nothing or exceeded the cap.
pub async fn read_line_capped<R>(reader: &mut R, cap: usize) -> std::io::Result<Option<String>>
where
    R: tokio::io::AsyncBufRead + Unpin,
{
    use tokio::io::AsyncBufReadExt;
    let mut buf = Vec::new();
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            break;
        }
        if let Some(pos) = available.iter().position(|b| *b == b'\n') {
            buf.extend_from_slice(&available[..pos]);
            reader.consume(pos + 1);
            break;
        }
        let n = available.len();
        buf.extend_from_slice(available);
        reader.consume(n);
        if buf.len() > cap {
            return Ok(None);
        }
    }
    if buf.len() > cap || buf.is_empty() {
        return Ok(None);
    }
    Ok(Some(String::from_utf8_lossy(&buf).into_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_shapes() {
        let r: Request = serde_json::from_str(r#"{"type":"ping"}"#).unwrap();
        assert_eq!(r, Request::Ping);
        let r: Request =
            serde_json::from_str(r#"{"type":"add","url":"a.example","tags":["x"]}"#).unwrap();
        assert!(matches!(
            r,
            Request::Add {
                fetch: true,
                merge: false,
                ..
            }
        ));
        assert!(serde_json::from_str::<Request>(r#"{"type":"nope"}"#).is_err());
        let r: Request = serde_json::from_str(r#"{"type":"device.name"}"#).unwrap();
        assert_eq!(r, Request::DeviceName { name: None });
        let r: Request = serde_json::from_str(r#"{"type":"pair.confirm","session":"ab"}"#).unwrap();
        assert_eq!(
            r,
            Request::PairConfirm {
                session: "ab".into()
            }
        );
        let line = Response::err(ErrorCode::NotFound, "x").to_line();
        assert!(line.contains(r#""code":"not_found""#));
    }
}
