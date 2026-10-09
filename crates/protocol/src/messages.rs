// SPDX-License-Identifier: GPL-3.0-or-later
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    Action, CaptureTarget, DeviceInfo, ErrorCode, InboxItem, LiveState, PairingInfo,
    PendingPairing, PointerMode, Role, ShowSnapshot,
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
    /// Admins: list screens and windows available for capture.
    ListCaptureTargets,
    /// Admins: create an API key with the given role (answered by `api_key`).
    CreateApiKey {
        name: String,
        role: Role,
    },
    /// Laser pointer or drawing in progress (presenters and up). `pos` is a fraction (0–1) of
    /// the content area; `null` hides the pointer. Not acknowledged; excess messages are dropped.
    Pointer {
        pos: Option<[f32; 2]>,
        mode: PointerMode,
        /// `#rrggbb`.
        color: String,
    },
}

/// Messages sent from the host to a client.
// Messages are built, serialized and dropped right away; boxing the large state variants would
// only complicate every construction site.
#[allow(clippy::large_enum_variant)]
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
        /// API keys (HTTP API, WebSocket, OSC) only work from this computer.
        api_local_only: bool,
    },
    /// Reply to `create_api_key`: the token is shown once and never again.
    ApiKey {
        device_id: String,
        name: String,
        token: String,
    },
    /// Admins only.
    Pairing {
        pairing: PairingInfo,
    },
    /// Admins only: uploaded files waiting for a decision.
    Inbox {
        items: Vec<InboxItem>,
        auto_accept: bool,
    },
    /// Background rendering status (operator view progress bar).
    RenderProgress {
        queued: u32,
    },
    /// Screens and windows that can be captured (admins, on request).
    CaptureTargets {
        targets: Vec<CaptureTarget>,
    },
    /// Another device's pointer moved (not sent back to the device that points).
    Pointer {
        device_id: String,
        pos: Option<[f32; 2]>,
        mode: PointerMode,
        color: String,
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
