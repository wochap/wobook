//! Endpoint hints, ranking, backoff and racing (D9).

use std::{
    future::Future,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};

use tokio::task::JoinSet;

/// Interface name prefixes of container and VM bridges never offered as hints.
const IGNORED_INTERFACE_PREFIXES: [&str; 4] = ["docker", "veth", "virbr", "br-"];
/// Head start of the most recently successful endpoint.
pub const RACE_HEAD_START: Duration = Duration::from_millis(300);
pub const BACKOFF_MIN: Duration = Duration::from_secs(1);
pub const BACKOFF_MAX: Duration = Duration::from_secs(30);

/// 100.64.0.0/10 (Tailscale CGNAT range).
#[must_use]
pub fn is_tailnet_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            o[0] == 100 && (o[1] & 0xc0) == 64
        }
        IpAddr::V6(v6) => v6.segments()[0..3] == [0xfd7a, 0x115c, 0xa1e0],
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceAddr {
    pub name: String,
    pub ip: IpAddr,
}

fn usable(interface: &InterfaceAddr) -> bool {
    if IGNORED_INTERFACE_PREFIXES
        .iter()
        .any(|prefix| interface.name.starts_with(prefix))
    {
        return false;
    }
    match interface.ip {
        IpAddr::V4(ip) => !ip.is_loopback() && !ip.is_link_local() && !ip.is_unspecified(),
        IpAddr::V6(ip) => !ip.is_loopback() && !ip.is_unicast_link_local() && !ip.is_unspecified(),
    }
}

/// Usable interface addresses on `port`; LAN first, tailnet appended.
/// `include_loopback` adds 127.0.0.1 last (tests, single host).
#[must_use]
pub fn endpoint_hints(
    interfaces: &[InterfaceAddr],
    port: u16,
    include_loopback: bool,
) -> Vec<SocketAddr> {
    let mut lan = Vec::new();
    let mut tailnet = Vec::new();
    for interface in interfaces.iter().filter(|i| usable(i)) {
        let addr = SocketAddr::new(interface.ip, port);
        let list = if is_tailnet_ip(interface.ip) {
            &mut tailnet
        } else {
            &mut lan
        };
        if !list.contains(&addr) {
            list.push(addr);
        }
    }
    lan.extend(tailnet);
    if include_loopback || lan.is_empty() {
        lan.push(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port));
    }
    lan.truncate(16);
    lan
}

#[must_use]
pub fn system_interfaces() -> Vec<InterfaceAddr> {
    match if_addrs::get_if_addrs() {
        Ok(list) => list
            .into_iter()
            .map(|i| InterfaceAddr {
                ip: i.ip(),
                name: i.name,
            })
            .collect(),
        Err(e) => {
            eprintln!("wobook-sync: cannot list network interfaces: {e}");
            Vec::new()
        }
    }
}

/// `WOBOOK_SYNC_LOOPBACK=1` advertises 127.0.0.1 too.
#[must_use]
pub fn current_hints(port: u16) -> Vec<SocketAddr> {
    let loopback = std::env::var("WOBOOK_SYNC_LOOPBACK").is_ok_and(|v| v == "1");
    endpoint_hints(&system_interfaces(), port, loopback)
}

/// Jittered exponential backoff: 1 s doubling to 30 s.
#[must_use]
pub fn backoff(failures: u32) -> Duration {
    let base = BACKOFF_MIN
        .saturating_mul(1u32 << failures.min(5))
        .min(BACKOFF_MAX);
    let jitter = rand::random::<f64>() * 0.4 + 0.8;
    base.mul_f64(jitter).clamp(BACKOFF_MIN, BACKOFF_MAX)
}

/// Dials `endpoints` (best first): the first starts now, the rest after
/// [`RACE_HEAD_START`] in parallel. First success wins; the others are
/// aborted. Returns every failure seen when all fail.
pub async fn race<T, E, F, Fut>(
    endpoints: Vec<SocketAddr>,
    dial: F,
) -> Result<(SocketAddr, T, Vec<(SocketAddr, E)>), Vec<(SocketAddr, E)>>
where
    T: Send + 'static,
    E: Send + 'static,
    F: Fn(SocketAddr) -> Fut,
    Fut: Future<Output = Result<T, E>> + Send + 'static,
{
    let mut set = JoinSet::new();
    for (i, addr) in endpoints.into_iter().enumerate() {
        let fut = dial(addr);
        set.spawn(async move {
            if i > 0 {
                tokio::time::sleep(RACE_HEAD_START).await;
            }
            (addr, fut.await)
        });
    }
    let mut failures = Vec::new();
    while let Some(joined) = set.join_next().await {
        match joined {
            Ok((addr, Ok(value))) => {
                set.abort_all();
                return Ok((addr, value, failures));
            }
            Ok((addr, Err(e))) => failures.push((addr, e)),
            Err(_) => {}
        }
    }
    Err(failures)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn iface(name: &str, ip: &str) -> InterfaceAddr {
        InterfaceAddr {
            name: name.into(),
            ip: ip.parse().unwrap(),
        }
    }

    #[test]
    fn hints_skip_bridges_and_append_tailnet() {
        let list = [
            iface("tailscale0", "100.84.12.7"),
            iface("lo", "127.0.0.1"),
            iface("docker0", "172.17.0.1"),
            iface("veth12", "10.1.1.1"),
            iface("enp3s0", "192.168.1.40"),
            iface("enp3s0", "fe80::1"),
        ];
        let hints = endpoint_hints(&list, 47390, false);
        assert_eq!(
            hints,
            vec![
                "192.168.1.40:47390".parse().unwrap(),
                "100.84.12.7:47390".parse().unwrap()
            ]
        );
        assert!(is_tailnet_ip("100.127.255.1".parse().unwrap()));
        assert!(!is_tailnet_ip("100.128.0.1".parse().unwrap()));
    }

    #[test]
    fn backoff_bounds() {
        for n in 0..10 {
            let d = backoff(n);
            assert!(d >= BACKOFF_MIN && d <= BACKOFF_MAX);
        }
    }

    #[tokio::test]
    async fn race_prefers_success() {
        let a: SocketAddr = "10.0.0.1:1".parse().unwrap();
        let b: SocketAddr = "10.0.0.2:1".parse().unwrap();
        let out = race(vec![a, b], move |addr| async move {
            if addr == a { Err("dead") } else { Ok(7) }
        })
        .await
        .unwrap();
        assert_eq!(out.0, b);
        assert_eq!(out.1, 7);
        assert_eq!(out.2.len(), 1);
    }
}
