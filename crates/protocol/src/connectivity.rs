// SPDX-License-Identifier: GPL-3.0-or-later
//! How remotes reach the host: LAN interfaces, HTTPS, the relay, and the relay tunnel framing.
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Default TCP port of the optional HTTPS server.
pub const DEFAULT_HTTPS_PORT: u16 = 4749;

/// Noise protocol used between remotes and hosts over the relay.
pub const RELAY_NOISE_PATTERN: &str = "Noise_NK_25519_ChaChaPoly_BLAKE2s";
/// Prologue prefix; the host id (UTF-8) follows it.
pub const RELAY_PROLOGUE: &str = "midnightsnack relay 1\n";
/// Largest Noise message (including the 16-byte tag).
pub const NOISE_MAX_MESSAGE: usize = 65_535;
/// Largest plaintext frame inside the tunnel.
pub const TUNNEL_MAX_FRAME: usize = NOISE_MAX_MESSAGE - 16;
/// Header of a tunnel frame: kind (1), stream (4, big endian), flags (1).
pub const TUNNEL_HEADER: usize = 6;
/// Largest payload of one tunnel frame.
pub const TUNNEL_MAX_PAYLOAD: usize = TUNNEL_MAX_FRAME - TUNNEL_HEADER;

/// The settings of a relay connection (admins).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RelaySettings {
    pub enabled: bool,
    /// Base URL of the relay, e.g. `https://relay.example.org`.
    pub url: String,
    /// Access token required by some relays (set by whoever runs the relay).
    #[serde(default)]
    pub access_token: Option<String>,
}

/// State of the host's connection to the relay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum RelayState {
    Off,
    Connecting,
    Connected,
    /// The last attempt failed; the host retries.
    Failed,
}

/// Why the relay connection failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum RelayError {
    InvalidUrl,
    /// DNS, TCP, TLS or WebSocket failure.
    Unreachable,
    /// The relay requires an access token, or the token is wrong.
    Unauthorized,
    /// The relay does not accept more hosts.
    Full,
    /// The relay speaks another protocol version.
    Incompatible,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RelayStatus {
    pub enabled: bool,
    pub url: String,
    pub has_access_token: bool,
    pub state: RelayState,
    pub error: Option<RelayError>,
    /// This host's id on relays.
    pub host_id: String,
    /// Remotes currently connected through the relay.
    pub remotes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct HttpsStatus {
    pub enabled: bool,
    /// Port the HTTPS server listens on, if running.
    pub port: Option<u16>,
    /// SHA-256 fingerprint of the certificate (`AB:CD:…`).
    pub fingerprint: Option<String>,
}

/// A network interface remotes may use.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct NetworkInterface {
    pub name: String,
    pub address: String,
    /// Looks like this computer's own hotspot.
    pub hotspot: bool,
}

/// Everything about how remotes reach the host (admins).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ConnectivityInfo {
    /// Plain HTTP port.
    pub port: u16,
    pub interfaces: Vec<NetworkInterface>,
    pub https: HttpsStatus,
    pub relay: RelayStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum JoinKind {
    Lan,
    Https,
    Relay,
}

/// A link for pairing a new device (QR code).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct JoinLink {
    pub kind: JoinKind,
    pub url: String,
    /// Interface name or relay host, for the selector.
    pub label: String,
}

/// The relay route of a host, as remotes need it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RelayRoute {
    /// The remote's page on the relay, e.g. `https://relay.example.org/r/<host id>`.
    pub url: String,
    /// The host's static Noise public key (base64url, no padding).
    pub key: String,
}

/// Other ways to reach the host, sent to paired devices for fallback.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Routes {
    /// Base URLs on the local network (`http://…`, `https://…`), preferred first.
    pub lan: Vec<String>,
    pub relay: Option<RelayRoute>,
}

/// How a device is connected right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ConnectionPath {
    /// A window of the host app on this computer.
    Local,
    Lan,
    Https,
    Relay,
}

/// Frame kinds inside the relay tunnel. See `docs/dev/protocol.md`.
pub mod tunnel {
    /// A WebSocket text message (stream 0); FIN marks its last fragment.
    pub const WS_MESSAGE: u8 = 1;
    /// HTTP request head (JSON); FIN means there is no body.
    pub const REQUEST_HEAD: u8 = 2;
    pub const REQUEST_BODY: u8 = 3;
    /// HTTP response head (JSON); FIN means there is no body.
    pub const RESPONSE_HEAD: u8 = 4;
    pub const RESPONSE_BODY: u8 = 5;
    /// Abort a stream (either side).
    pub const RESET: u8 = 6;
    pub const PING: u8 = 7;
    pub const PONG: u8 = 8;
    /// The WebSocket session ended (stream 0).
    pub const WS_CLOSE: u8 = 9;

    /// Last frame of a message or body.
    pub const FIN: u8 = 1;

    /// Operations between relay and host on the host's link (`[op][channel u32 BE][payload]`).
    pub mod link {
        /// A remote connected (relay → host).
        pub const OPEN: u8 = 1;
        /// A Noise message for or from a channel.
        pub const DATA: u8 = 2;
        /// The channel ended (both directions).
        pub const CLOSE: u8 = 3;
    }
}

/// WebSocket close codes the relay uses towards remotes.
pub mod relay_close {
    /// The host is not connected to the relay.
    pub const HOST_OFFLINE: u16 = 4404;
    /// The host has too many remotes on the relay.
    pub const HOST_FULL: u16 = 4429;
    /// The remote sent too much, too fast.
    pub const RATE_LIMITED: u16 = 4408;
    /// The host closed the channel.
    pub const CLOSED_BY_HOST: u16 = 4000;
}
