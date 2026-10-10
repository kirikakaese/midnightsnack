// SPDX-License-Identifier: GPL-3.0-or-later
//! Controller mode: this app runs the operator UI of another midnightsnack host on the network.
//! The other host is found via mDNS (or typed in), paired like a phone (PIN, approved by its
//! operator), and remembered with its device token.

use std::time::Duration;

use midnightsnack_protocol::{ApiError, PairRequest, PairResponse, PairStatus, Role};
use midnightsnack_server::discovery::{self, FoundHost};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder};

use crate::HostState;

const LABEL_PREFIX: &str = "controller-";
/// How long to wait for the other operator to approve.
const APPROVAL_TIMEOUT: Duration = Duration::from_secs(180);

/// A host this computer controls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteHost {
    pub id: String,
    pub name: String,
    /// `http://<ip>:<port>`
    pub base_url: String,
    pub role: Role,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub token: String,
}

impl RemoteHost {
    /// Without the token, for the UI.
    fn public(&self) -> RemoteHost {
        RemoteHost {
            token: String::new(),
            ..self.clone()
        }
    }

    pub fn ws_url(&self) -> String {
        let rest = self
            .base_url
            .strip_prefix("https://")
            .map(|r| format!("wss://{r}"))
            .or_else(|| {
                self.base_url
                    .strip_prefix("http://")
                    .map(|r| format!("ws://{r}"))
            })
            .unwrap_or_else(|| self.base_url.clone());
        format!("{rest}/api/v1/ws")
    }
}

pub fn label(id: &str) -> String {
    format!("{LABEL_PREFIX}{id}")
}

pub fn remote_id(label: &str) -> Option<&str> {
    label.strip_prefix(LABEL_PREFIX)
}

/// Accepts `192.168.1.5`, `192.168.1.5:4747`, `http://…` or a full join link.
pub fn normalize_url(input: &str) -> Option<String> {
    let input = input.trim();
    let rest = input
        .strip_prefix("http://")
        .or_else(|| input.strip_prefix("https://"))
        .unwrap_or(input);
    let host_port = rest.split(['/', '#', '?']).next()?.trim();
    if host_port.is_empty() || host_port.contains(char::is_whitespace) {
        return None;
    }
    let with_port = if host_port.starts_with('[') {
        if host_port.contains("]:") {
            host_port.to_owned()
        } else {
            format!("{host_port}:{}", midnightsnack_protocol::DEFAULT_PORT)
        }
    } else if host_port.contains(':') {
        host_port.to_owned()
    } else {
        format!("{host_port}:{}", midnightsnack_protocol::DEFAULT_PORT)
    };
    Some(format!("http://{with_port}"))
}

/// The join token of a pasted join link (`…/join#t=<token>`), if any.
pub fn join_token(input: &str) -> String {
    input
        .split_once("#t=")
        .map(|(_, t)| t.trim().to_owned())
        .unwrap_or_default()
}

#[tauri::command]
pub async fn discover_hosts() -> Vec<FoundHost> {
    tauri::async_runtime::spawn_blocking(|| discovery::browse(Duration::from_secs(2)))
        .await
        .unwrap_or_default()
}

#[tauri::command]
pub fn remote_hosts(state: State<'_, HostState>) -> Vec<RemoteHost> {
    let s = state.settings.lock().unwrap_or_else(|e| e.into_inner());
    s.remotes.iter().map(RemoteHost::public).collect()
}

#[tauri::command]
pub fn forget_remote(app: AppHandle, state: State<'_, HostState>, id: String) {
    if let Some(w) = app.get_webview_window(&label(&id)) {
        let _ = w.destroy();
    }
    state
        .settings
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remotes
        .retain(|r| r.id != id);
    state.save_settings();
}

/// Pairs with another host and waits for its operator to approve. `address` may be an address
/// or a join link; with a join link the request is approved automatically if the other host
/// allows it. Errors are protocol error codes (`invalid_pin`, `pairing_denied`, …).
#[tauri::command]
pub async fn pair_remote(
    state: State<'_, HostState>,
    address: String,
    pin: String,
    device_name: String,
) -> Result<RemoteHost, String> {
    let base_url = normalize_url(&address).ok_or("malformed_message")?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|_| "internal")?;
    let info: midnightsnack_protocol::HostInfo = client
        .get(format!("{base_url}/api/v1/info"))
        .send()
        .await
        .map_err(|_| "not_found")?
        .json()
        .await
        .map_err(|_| "protocol_mismatch")?;
    if info.protocol_version != midnightsnack_protocol::PROTOCOL_VERSION {
        return Err("protocol_mismatch".into());
    }
    let res = client
        .post(format!("{base_url}/api/v1/pair"))
        .json(&PairRequest {
            join_token: join_token(&address),
            pin,
            device_name,
        })
        .send()
        .await
        .map_err(|_| "not_found")?;
    if !res.status().is_success() {
        let code = res
            .json::<ApiError>()
            .await
            .map(|e| e.code)
            .map_err(|_| "internal")?;
        return Err(error_name(code));
    }
    let PairResponse { request_id } = res.json().await.map_err(|_| "internal")?;
    let deadline = tokio::time::Instant::now() + APPROVAL_TIMEOUT;
    loop {
        if tokio::time::Instant::now() > deadline {
            return Err("pairing_denied".into());
        }
        let status: PairStatus = client
            .get(format!("{base_url}/api/v1/pair/{request_id}"))
            .send()
            .await
            .map_err(|_| "not_found")?
            .json()
            .await
            .map_err(|_| "pairing_denied")?;
        match status {
            PairStatus::Pending => tokio::time::sleep(Duration::from_secs(1)).await,
            PairStatus::Denied => return Err("pairing_denied".into()),
            PairStatus::Approved {
                token,
                device_id,
                role,
            } => {
                let remote = RemoteHost {
                    id: device_id,
                    name: info.name,
                    base_url,
                    role,
                    token,
                };
                let mut s = state.settings.lock().unwrap_or_else(|e| e.into_inner());
                s.remotes.retain(|r| r.base_url != remote.base_url);
                s.remotes.push(remote.clone());
                drop(s);
                state.save_settings();
                return Ok(remote.public());
            }
        }
    }
}

fn error_name(code: midnightsnack_protocol::ErrorCode) -> String {
    serde_json::to_value(code)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_else(|| "internal".into())
}

/// Opens (or focuses) the operator window for a remembered host.
#[tauri::command]
pub fn open_controller(
    app: AppHandle,
    state: State<'_, HostState>,
    id: String,
) -> Result<(), String> {
    let name = state
        .settings
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remotes
        .iter()
        .find(|r| r.id == id)
        .map(|r| r.name.clone())
        .ok_or("not_found")?;
    if let Some(w) = app.get_webview_window(&label(&id)) {
        let _ = w.set_focus();
        return Ok(());
    }
    let url = format!("index.html#/controller/{id}");
    WebviewWindowBuilder::new(&app, label(&id), WebviewUrl::App(url.into()))
        .title(format!("DECK — {name}"))
        .inner_size(1280.0, 800.0)
        .min_inner_size(900.0, 600.0)
        .build()
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn remote(state: &HostState, id: &str) -> Option<RemoteHost> {
    state
        .settings
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remotes
        .iter()
        .find(|r| r.id == id)
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_and_join_links() {
        assert_eq!(
            normalize_url("192.168.1.5").as_deref(),
            Some("http://192.168.1.5:4747")
        );
        assert_eq!(
            normalize_url(" http://stage.local:5000/ ").as_deref(),
            Some("http://stage.local:5000")
        );
        let link = "http://10.0.0.2:4747/join#t=abc123";
        assert_eq!(normalize_url(link).as_deref(), Some("http://10.0.0.2:4747"));
        assert_eq!(join_token(link), "abc123");
        assert_eq!(join_token("10.0.0.2"), "");
        assert_eq!(
            normalize_url("[fe80::1]").as_deref(),
            Some("http://[fe80::1]:4747")
        );
        assert_eq!(normalize_url(""), None);
        assert_eq!(normalize_url("a b"), None);
    }

    #[test]
    fn websocket_url_follows_the_base() {
        let r = RemoteHost {
            id: "d".into(),
            name: "Stage".into(),
            base_url: "http://10.0.0.2:4747".into(),
            role: Role::Admin,
            token: "t".into(),
        };
        assert_eq!(r.ws_url(), "ws://10.0.0.2:4747/api/v1/ws");
        assert!(serde_json::to_string(&r.public())
            .unwrap()
            .contains("\"name\""));
        assert!(!serde_json::to_string(&r.public())
            .unwrap()
            .contains("token"));
    }
}
