//! Control stream messages (D8, D9): hello, discovery secret rotation,
//! nudges and new-device announcements. JSON lines, 64 KiB each.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};

use crate::control::DiscoveryGroup;

pub const MAX_CONTROL_LINE: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupWire {
    pub epoch: u64,
    pub secret: String,
}

impl From<&DiscoveryGroup> for GroupWire {
    fn from(g: &DiscoveryGroup) -> Self {
        Self {
            epoch: g.epoch,
            secret: URL_SAFE_NO_PAD.encode(g.secret),
        }
    }
}

impl GroupWire {
    #[must_use]
    pub fn decode(&self) -> Option<DiscoveryGroup> {
        let secret = URL_SAFE_NO_PAD.decode(&self.secret).ok()?.try_into().ok()?;
        Some(DiscoveryGroup {
            epoch: self.epoch,
            secret,
        })
    }
}

/// A device as shared in provisioning and announcements.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceWire {
    pub id: String,
    /// base64url Ed25519 public key.
    pub key: String,
    pub name: String,
    pub platform: String,
    #[serde(default)]
    pub endpoints: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ControlMessage {
    Hello {
        name: String,
        platform: String,
        epoch: u64,
        #[serde(default)]
        group: Option<GroupWire>,
        #[serde(default)]
        endpoints: Vec<String>,
    },
    DiscoveryUpdate {
        group: GroupWire,
    },
    DiscoveryAck {
        epoch: u64,
    },
    Nudge,
    Announce {
        device: DeviceWire,
    },
}

impl ControlMessage {
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut line = serde_json::to_vec(self).unwrap_or_default();
        line.push(b'\n');
        line
    }

    pub fn decode(line: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let g = DiscoveryGroup {
            epoch: 3,
            secret: [5; 32],
        };
        let msg = ControlMessage::DiscoveryUpdate {
            group: GroupWire::from(&g),
        };
        let line = msg.encode();
        assert!(line.ends_with(b"\n"));
        let back = ControlMessage::decode(&line[..line.len() - 1]).unwrap();
        assert_eq!(back, msg);
        let ControlMessage::DiscoveryUpdate { group } = back else {
            unreachable!()
        };
        assert_eq!(group.decode().unwrap(), g);
        assert_eq!(
            ControlMessage::decode(br#"{"type":"nudge"}"#).unwrap(),
            ControlMessage::Nudge
        );
    }
}
