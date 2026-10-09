// SPDX-License-Identifier: GPL-3.0-or-later
//! Network addresses for join URLs and mDNS/DNS-SD advertisement.

use std::net::{IpAddr, Ipv4Addr};

use mdns_sd::{ServiceDaemon, ServiceInfo};
use midnightsnack_protocol::{MDNS_SERVICE_TYPE, PROTOCOL_VERSION};

/// IPv4 addresses remotes can reach, best first (private LAN ranges before others).
pub fn lan_addresses() -> Vec<Ipv4Addr> {
    let mut addrs: Vec<Ipv4Addr> = if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter(|i| !i.is_loopback())
        .filter_map(|i| match i.ip() {
            IpAddr::V4(v4) if !v4.is_link_local() && !v4.is_unspecified() => Some(v4),
            _ => None,
        })
        .collect();
    addrs.sort_by_key(|a| {
        let o = a.octets();
        match o {
            [192, 168, ..] => 0,
            [10, ..] => 1,
            [172, b, ..] if (16..32).contains(&b) => 2,
            _ => 3,
        }
    });
    addrs.dedup();
    addrs
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
