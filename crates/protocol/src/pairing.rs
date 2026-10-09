// SPDX-License-Identifier: GPL-3.0-or-later
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::Role;

/// `POST /api/v1/pair` body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PairRequest {
    /// One-time token from the QR code / join URL.
    pub join_token: String,
    pub pin: String,
    pub device_name: String,
}

/// `POST /api/v1/pair` success response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PairResponse {
    pub request_id: String,
}

/// `GET /api/v1/pair/{request_id}` response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "status", rename_all = "snake_case")]
#[ts(export)]
pub enum PairStatus {
    Pending,
    /// Returned exactly once; the client must store `token`.
    Approved {
        token: String,
        device_id: String,
        role: Role,
    },
    Denied,
}

/// Body of every non-2xx API response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ApiError {
    pub code: crate::ErrorCode,
}

/// A pairing request waiting for operator approval.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PendingPairing {
    pub request_id: String,
    pub device_name: String,
    pub address: String,
    #[ts(type = "number")]
    pub requested_at_ms: i64,
}

/// A remembered (paired) device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub role: Role,
    pub connected: bool,
    /// Built-in device (host windows). Cannot be revoked.
    pub local: bool,
    /// An API key for a control surface (Companion, scripts) rather than a paired device.
    pub api_key: bool,
    /// Round-trip latency of the last ping, if measured.
    pub latency_ms: Option<u32>,
    #[ts(type = "number | null")]
    pub last_seen_ms: Option<i64>,
}

/// A file sent from a device, waiting for an admin to accept or reject it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct InboxItem {
    pub id: String,
    pub file_name: String,
    #[ts(type = "number")]
    pub size: u64,
    pub device_name: String,
    #[ts(type = "number")]
    pub received_ms: i64,
}

/// `POST /api/v1/upload` success response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct UploadResponse {
    pub upload_id: String,
    /// The file was added to the show right away (admin or auto-accept); otherwise it waits in
    /// the inbox.
    pub added: bool,
}

/// Pairing information shown by the host (QR code + PIN). Only sent to admins.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PairingInfo {
    pub pin: String,
    /// One join URL per usable network interface; the first is preferred.
    pub join_urls: Vec<String>,
    pub auto_approve: Option<Role>,
}
