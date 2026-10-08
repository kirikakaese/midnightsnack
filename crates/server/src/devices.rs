// SPDX-License-Identifier: GPL-3.0-or-later
//! Paired devices and their session tokens.

use std::collections::HashMap;
use std::path::PathBuf;

use midnightsnack_core::now_ms;
use midnightsnack_protocol::{DeviceInfo, Role};
use serde::{Deserialize, Serialize};

use crate::util::{random_id, random_token, read_json, sha256_hex, write_json_atomic};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredDevice {
    id: String,
    name: String,
    role: Role,
    /// SHA-256 of the session token; the token itself is never stored.
    token_hash: String,
    created_ms: i64,
    last_seen_ms: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub role: Role,
    /// Built-in host window; only accepted from loopback connections.
    pub local: bool,
}

/// Remembered devices (persisted) plus the host's own local sessions (in memory).
pub struct DeviceStore {
    path: Option<PathBuf>,
    devices: Vec<StoredDevice>,
    local: Vec<(String, Device)>, // (token hash, device)
    connected: HashMap<String, usize>,
    latency: HashMap<String, u32>,
}

impl DeviceStore {
    pub fn load(path: Option<PathBuf>) -> Self {
        let devices = path.as_deref().and_then(read_json).unwrap_or_default();
        DeviceStore {
            path,
            devices,
            local: Vec::new(),
            connected: HashMap::new(),
            latency: HashMap::new(),
        }
    }

    fn persist(&self) {
        if let Some(p) = &self.path {
            if let Err(e) = write_json_atomic(p, &self.devices) {
                tracing::error!(error = %e, "failed to save devices");
            }
        }
    }

    /// Creates an in-memory session for a host window. Returns its token.
    pub fn add_local(&mut self, name: &str, role: Role) -> String {
        let token = random_token();
        let device = Device {
            id: format!("local-{}", random_id()),
            name: name.to_owned(),
            role,
            local: true,
        };
        self.local.push((sha256_hex(&token), device));
        token
    }

    /// Registers a newly paired device and returns `(device id, token)`.
    pub fn add(&mut self, name: &str, role: Role) -> (String, String) {
        let token = random_token();
        let id = random_id();
        self.devices.push(StoredDevice {
            id: id.clone(),
            name: name.to_owned(),
            role,
            token_hash: sha256_hex(&token),
            created_ms: now_ms(),
            last_seen_ms: None,
        });
        self.persist();
        (id, token)
    }

    pub fn authenticate(&self, token: &str) -> Option<Device> {
        let hash = sha256_hex(token);
        if let Some((_, d)) = self.local.iter().find(|(h, _)| *h == hash) {
            return Some(d.clone());
        }
        self.devices
            .iter()
            .find(|d| d.token_hash == hash)
            .map(|d| Device {
                id: d.id.clone(),
                name: d.name.clone(),
                role: d.role,
                local: false,
            })
    }

    /// Current state of a device, or `None` if it was revoked.
    pub fn get(&self, id: &str) -> Option<Device> {
        if let Some((_, d)) = self.local.iter().find(|(_, d)| d.id == id) {
            return Some(d.clone());
        }
        self.devices.iter().find(|d| d.id == id).map(|d| Device {
            id: d.id.clone(),
            name: d.name.clone(),
            role: d.role,
            local: false,
        })
    }

    pub fn set_role(&mut self, id: &str, role: Role) -> bool {
        let Some(d) = self.devices.iter_mut().find(|d| d.id == id) else {
            return false;
        };
        d.role = role;
        self.persist();
        true
    }

    pub fn revoke(&mut self, id: &str) -> bool {
        let before = self.devices.len();
        self.devices.retain(|d| d.id != id);
        let removed = self.devices.len() != before;
        if removed {
            self.persist();
        }
        removed
    }

    /// Forgets every remote device. Local sessions are kept.
    pub fn revoke_all(&mut self) {
        self.devices.clear();
        self.persist();
    }

    pub fn mark_connected(&mut self, id: &str, delta: isize) {
        let n = self.connected.entry(id.to_owned()).or_default();
        *n = n.saturating_add_signed(delta);
        if *n == 0 {
            self.connected.remove(id);
            self.latency.remove(id);
        }
        if let Some(d) = self.devices.iter_mut().find(|d| d.id == id) {
            d.last_seen_ms = Some(now_ms());
        }
    }

    pub fn set_latency(&mut self, id: &str, ms: u32) {
        self.latency.insert(id.to_owned(), ms);
    }

    pub fn list(&self) -> Vec<DeviceInfo> {
        let local = self.local.iter().map(|(_, d)| DeviceInfo {
            id: d.id.clone(),
            name: d.name.clone(),
            role: d.role,
            connected: self.connected.contains_key(&d.id),
            local: true,
            latency_ms: self.latency.get(&d.id).copied(),
            last_seen_ms: None,
        });
        let remote = self.devices.iter().map(|d| DeviceInfo {
            id: d.id.clone(),
            name: d.name.clone(),
            role: d.role,
            connected: self.connected.contains_key(&d.id),
            local: false,
            latency_ms: self.latency.get(&d.id).copied(),
            last_seen_ms: d.last_seen_ms,
        });
        local.chain(remote).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_authenticate_and_persist_hashed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("devices.json");
        let mut store = DeviceStore::load(Some(path.clone()));
        let (id, token) = store.add("Phone", Role::Presenter);
        assert_eq!(store.authenticate(&token).unwrap().id, id);
        assert!(store.authenticate("wrong").is_none());
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(
            !raw.contains(&token),
            "token must not be stored in clear text"
        );

        let store2 = DeviceStore::load(Some(path));
        assert_eq!(store2.authenticate(&token).unwrap().role, Role::Presenter);
    }

    #[test]
    fn revoke_and_roles() {
        let mut store = DeviceStore::load(None);
        let (id, token) = store.add("Phone", Role::Presenter);
        assert!(store.set_role(&id, Role::Operator));
        assert_eq!(store.get(&id).unwrap().role, Role::Operator);
        assert!(store.revoke(&id));
        assert!(store.authenticate(&token).is_none());
        let local = store.add_local("Operator", Role::Admin);
        store.revoke_all();
        assert!(store.authenticate(&local).unwrap().local);
    }
}
