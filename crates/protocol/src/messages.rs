// SPDX-License-Identifier: GPL-3.0-or-later
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    Action, DeviceInfo, ErrorCode, LiveState, PairingInfo, PendingPairing, Role, ShowSnapshot,
};

/// Static information about a host, available before pairing (`GET /api/v1/info`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct HostInfo {
    pub name: String,
    pub app_version: String,
    pub protocol_version: u32,
}

/// Who the server thinks this connection is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SessionInfo {
    pub device_id: String,
    pub device_name: String,
    pub role: Role,
    /// Connection originates from the host machine itself.
    pub local: bool,
    /// Appended as `?k=` to media URLs (images cannot send headers).
    pub media_key: String,
}

/// Messages sent from a client to the host over `GET /api/v1/ws`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum ClientMessage {
    /// Must be the first message on every connection.
    Hello {
        protocol_version: u32,
        token: String,
    },
    /// Request an action. The host answers with `action_result` carrying the same `request_id`.
    Action {
        request_id: u32,
        action: Action,
    },
    /// Output windows report their size so slides are pre-rendered at native resolution.
    Viewport {
        width: u32,
        height: u32,
    },
    Ping {
        nonce: u32,
    },
}

/// Messages sent from the host to a client.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum ServerMessage {
    Welcome {
        host: HostInfo,
        session: SessionInfo,
    },
    Show {
        show: ShowSnapshot,
    },
    Live {
        live: LiveState,
    },
    /// Admins only.
    Devices {
        devices: Vec<DeviceInfo>,
        pending: Vec<PendingPairing>,
    },
    /// Admins only.
    Pairing {
        pairing: PairingInfo,
    },
    /// Background rendering status (operator view progress bar).
    RenderProgress {
        queued: u32,
    },
    /// Session role changed by an admin.
    Session {
        session: SessionInfo,
    },
    ActionResult {
        request_id: u32,
        error: Option<ErrorCode>,
    },
    Pong {
        nonce: u32,
    },
    /// A request failed. `code` is a stable, translatable identifier.
    Error {
        code: ErrorCode,
    },
}
