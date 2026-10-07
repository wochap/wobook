//! Private mDNS advertise and browse (D8). Discovery only refreshes endpoints
//! of devices that are already trusted; it never grants trust.

use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use hkdf::Hkdf;
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use sha2::Sha256;

use crate::{
    control::{DiscoveryGroup, EndpointKind, SqliteControlStore, TrustState},
    identity::DeviceId,
};

const SERVICE_PREFIX: &str = "_wobook-";
const TAG_LEN: usize = 10;

/// `_wobook-<10 lowercase base32 chars>._udp.local.` from the group secret.
#[must_use]
pub fn service_type(secret: &[u8; 32]) -> String {
    let hk = Hkdf::<Sha256>::new(Some(b"wobook-discovery"), secret);
    let mut tag = [0u8; 8];
    hk.expand(b"wobook-discovery-v1 service-type", &mut tag)
        .expect("8 bytes is a valid HKDF length");
    let encoded = data_encoding::BASE32_NOPAD
        .encode(&tag)
        .to_ascii_lowercase();
    format!("{SERVICE_PREFIX}{}._udp.local.", &encoded[..TAG_LEN])
}

/// `WOBOOK_DISCOVERY=off` disables multicast entirely.
#[must_use]
pub fn enabled() -> bool {
    !std::env::var("WOBOOK_DISCOVERY").is_ok_and(|v| v.eq_ignore_ascii_case("off"))
}

struct Active {
    service_type: String,
    fullname: String,
}

pub struct Discovery {
    daemon: ServiceDaemon,
    device: DeviceId,
    port: u16,
    store: Arc<SqliteControlStore>,
    active: Mutex<Vec<Active>>,
}

impl Discovery {
    pub fn start(
        device: DeviceId,
        port: u16,
        store: Arc<SqliteControlStore>,
    ) -> Result<Arc<Self>, String> {
        let daemon = ServiceDaemon::new().map_err(|e| e.to_string())?;
        daemon
            .set_service_name_len_max(30)
            .map_err(|e| e.to_string())?;
        let discovery = Arc::new(Self {
            daemon,
            device,
            port,
            store,
            active: Mutex::new(Vec::new()),
        });
        discovery.refresh()?;
        Ok(discovery)
    }

    /// Re-advertises under the current group and browses current and retained
    /// previous service types.
    pub fn refresh(self: &Arc<Self>) -> Result<(), String> {
        let now = wobook_core::now_ms();
        let group = self
            .store
            .ensure_discovery_group()
            .map_err(|e| e.to_string())?;
        let mut groups = vec![group.clone()];
        if let Ok(Some(rotation)) = self.store.discovery_rotation(now) {
            groups.push(DiscoveryGroup {
                epoch: rotation.previous_epoch,
                secret: rotation.previous_secret,
            });
        }
        let mut active = self.active.lock().map_err(|_| "poisoned")?;
        for old in active.drain(..) {
            let _ = self.daemon.unregister(&old.fullname);
            let _ = self.daemon.stop_browse(&old.service_type);
        }
        let ty = service_type(&group.secret);
        let id = self.device.to_hex();
        let host = format!("wobook-{}.local.", &id[..12]);
        let port = self.port.to_string();
        let epoch = group.epoch.to_string();
        let txt = [
            ("v", "1"),
            ("id", id.as_str()),
            ("port", port.as_str()),
            ("epoch", epoch.as_str()),
        ];
        let info = ServiceInfo::new(&ty, &id[..16], &host, "", self.port, &txt[..])
            .map_err(|e| e.to_string())?
            .enable_addr_auto();
        let fullname = info.get_fullname().to_string();
        self.daemon.register(info).map_err(|e| e.to_string())?;
        active.push(Active {
            service_type: ty.clone(),
            fullname,
        });
        for g in groups {
            let ty = service_type(&g.secret);
            let receiver = self.daemon.browse(&ty).map_err(|e| e.to_string())?;
            let this = self.clone();
            tokio::spawn(async move {
                while let Ok(event) = receiver.recv_async().await {
                    if let ServiceEvent::ServiceResolved(info) = event {
                        this.learn(&info);
                    }
                }
            });
            if ty != active[0].service_type {
                active.push(Active {
                    service_type: ty,
                    fullname: String::new(),
                });
            }
        }
        Ok(())
    }

    fn learn(&self, info: &mdns_sd::ResolvedService) {
        let Some(id) = info.get_property_val_str("id") else {
            return;
        };
        let Ok(device) = id.parse::<DeviceId>() else {
            return;
        };
        if device == self.device {
            return;
        }
        // Unknown or revoked advertisers are ignored.
        match self.store.peer(device) {
            Ok(Some(record)) if record.state == TrustState::Trusted => {}
            _ => return,
        }
        let port = info
            .get_property_val_str("port")
            .and_then(|p| p.parse().ok())
            .unwrap_or(info.get_port());
        for ip in info.get_addresses() {
            let ip = ip.to_ip_addr();
            if ip.is_loopback() {
                continue;
            }
            let address = SocketAddr::new(ip, port);
            let _ =
                self.store
                    .upsert_endpoint(device, address, EndpointKind::for_address(&address));
        }
    }

    pub fn shutdown(&self) {
        if let Ok(active) = self.active.lock() {
            for a in active.iter() {
                if !a.fullname.is_empty() {
                    let _ = self.daemon.unregister(&a.fullname);
                }
            }
        }
        let _ = self.daemon.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_type_shape() {
        let ty = service_type(&[1; 32]);
        assert!(ty.starts_with("_wobook-") && ty.ends_with("._udp.local."));
        let tag = &ty[8..18];
        assert_eq!(tag.len(), 10);
        assert!(
            tag.bytes()
                .all(|b| b.is_ascii_lowercase() || (b'2'..=b'7').contains(&b))
        );
        assert_ne!(ty, service_type(&[2; 32]));
    }
}
