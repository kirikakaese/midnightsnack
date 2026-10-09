// SPDX-License-Identifier: GPL-3.0-or-later
//! Network addresses for join URLs and mDNS/DNS-SD advertisement.

use std::net::{IpAddr, Ipv4Addr};

use mdns_sd::{ServiceDaemon, ServiceInfo};
use midnightsnack_protocol::{NetworkInterface, MDNS_SERVICE_TYPE, PROTOCOL_VERSION};

/// Network interfaces remotes can reach, best first: this computer's hotspot, then private
/// LAN ranges, then anything else.
pub fn interfaces() -> Vec<NetworkInterface> {
    let raw: Vec<(String, Ipv4Addr)> = if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter(|i| !i.is_loopback())
        .filter_map(|i| match i.ip() {
            IpAddr::V4(v4) if !v4.is_link_local() && !v4.is_unspecified() => Some((i.name, v4)),
            _ => None,
        })
        .collect();
    rank(raw)
}

fn rank(raw: Vec<(String, Ipv4Addr)>) -> Vec<NetworkInterface> {
    let mut v: Vec<(u8, NetworkInterface)> = raw
        .into_iter()
        .map(|(name, ip)| {
            let hotspot = is_hotspot(&name, ip);
            let o = ip.octets();
            let rank = if hotspot {
                0
            } else {
                match o {
                    [192, 168, ..] => 1,
                    [10, ..] => 2,
                    [172, b, ..] if (16..32).contains(&b) => 3,
                    _ => 4,
                }
            };
            let iface = NetworkInterface {
                name,
                address: ip.to_string(),
                hotspot,
            };
            (rank, iface)
        })
        .collect();
    v.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.name.cmp(&b.1.name)));
    v.dedup_by(|a, b| a.1.address == b.1.address);
    v.into_iter().map(|(_, i)| i).collect()
}

/// Recognizes the access point of the common hotspot implementations:
/// Windows Mobile Hotspot (192.168.137.1), macOS Internet Sharing (`bridge100`…), NetworkManager
/// shared connections (10.42.0.1), hostapd-style `ap0`/`uap0` interfaces.
pub fn is_hotspot(name: &str, ip: Ipv4Addr) -> bool {
    let name = name.to_ascii_lowercase();
    ip.octets() == [192, 168, 137, 1]
        || (ip.octets()[..3] == [10, 42, 0] && ip.octets()[3] == 1)
        || (name.starts_with("bridge1") && ip.octets()[..2] == [192, 168])
        || name == "ap0"
        || name == "uap0"
}

/// IPv4 addresses remotes can reach, best first.
pub fn lan_addresses() -> Vec<Ipv4Addr> {
    interfaces()
        .into_iter()
        .filter_map(|i| i.address.parse().ok())
        .collect()
}

/// Keeps the advertisement alive while held.
pub struct Advertisement {
    daemon: ServiceDaemon,
    fullname: String,
}

impl Drop for Advertisement {
    fn drop(&mut self) {
        let _ = self.daemon.unregister(&self.fullname);
        let _ = self.daemon.shutdown();
    }
}

pub fn advertise(instance: &str, port: u16, version: &str) -> Option<Advertisement> {
    let daemon = match ServiceDaemon::new() {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!(error = %e, "mDNS unavailable");
            return None;
        }
    };
    let host = format!(
        "{}.local.",
        instance
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect::<String>()
    );
    let proto = PROTOCOL_VERSION.to_string();
    let props = [
        ("version", version),
        ("protocol", proto.as_str()),
        ("path", "/"),
    ];
    let info = match ServiceInfo::new(MDNS_SERVICE_TYPE, instance, &host, (), port, &props[..]) {
        Ok(i) => i.enable_addr_auto(),
        Err(e) => {
            tracing::warn!(error = %e, "invalid mDNS service info");
            return None;
        }
    };
    let fullname = info.get_fullname().to_owned();
    if let Err(e) = daemon.register(info) {
        tracing::warn!(error = %e, "mDNS registration failed");
        return None;
    }
    tracing::info!(%fullname, "advertising via mDNS");
    Some(Advertisement { daemon, fullname })
}

/// A midnightsnack host found on the network.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct FoundHost {
    pub name: String,
    /// `http://<ip>:<port>`
    pub url: String,
    pub version: String,
}

/// Looks for hosts advertising via mDNS for `timeout`.
pub fn browse(timeout: std::time::Duration) -> Vec<FoundHost> {
    let Ok(daemon) = ServiceDaemon::new() else {
        return Vec::new();
    };
    let Ok(rx) = daemon.browse(MDNS_SERVICE_TYPE) else {
        let _ = daemon.shutdown();
        return Vec::new();
    };
    let deadline = std::time::Instant::now() + timeout;
    let mut found: Vec<FoundHost> = Vec::new();
    while let Some(left) = deadline.checked_duration_since(std::time::Instant::now()) {
        let Ok(event) = rx.recv_timeout(left) else {
            break;
        };
        if let mdns_sd::ServiceEvent::ServiceResolved(info) = event {
            let name = info
                .get_fullname()
                .strip_suffix(&format!(".{MDNS_SERVICE_TYPE}"))
                .unwrap_or(info.get_fullname())
                .to_owned();
            let version = info
                .get_property_val_str("version")
                .unwrap_or_default()
                .to_owned();
            // Prefer IPv4 addresses, like the join links.
            let mut addrs: Vec<IpAddr> = info.get_addresses().iter().copied().collect();
            addrs.sort_by_key(|a| !a.is_ipv4());
            if let Some(ip) = addrs.first() {
                let url = match ip {
                    IpAddr::V4(v4) => format!("http://{v4}:{}", info.get_port()),
                    IpAddr::V6(v6) => format!("http://[{v6}]:{}", info.get_port()),
                };
                if !found.iter().any(|f| f.url == url) {
                    found.push(FoundHost { name, url, version });
                }
            }
        }
    }
    let _ = daemon.shutdown();
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotspots_are_recognized_and_preferred() {
        let ifaces = rank(vec![
            ("eth0".into(), Ipv4Addr::new(172, 20, 0, 3)),
            ("wlan0".into(), Ipv4Addr::new(192, 168, 1, 20)),
            (
                "Local Area Connection* 10".into(),
                Ipv4Addr::new(192, 168, 137, 1),
            ),
            ("tun0".into(), Ipv4Addr::new(100, 64, 1, 2)),
        ]);
        let names: Vec<&str> = ifaces.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(
            names,
            ["Local Area Connection* 10", "wlan0", "eth0", "tun0"]
        );
        assert!(ifaces[0].hotspot);
        assert!(!ifaces[1].hotspot);
        assert!(is_hotspot("bridge100", Ipv4Addr::new(192, 168, 2, 1)));
        assert!(is_hotspot("wlp2s0", Ipv4Addr::new(10, 42, 0, 1)));
        assert!(!is_hotspot("wlp2s0", Ipv4Addr::new(10, 42, 0, 77)));
        assert!(!is_hotspot("en0", Ipv4Addr::new(192, 168, 2, 1)));
    }
}
