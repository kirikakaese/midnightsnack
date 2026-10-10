// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared server state and the action entry point used by every transport.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use midnightsnack_capture::CaptureHub;
use midnightsnack_core::{dispatch, now_ms, Change, Dispatched, Engine, Origin, Show};
use midnightsnack_protocol::{
    Action, ConnectivityInfo, ErrorCode, HostInfo, HttpsStatus, JoinKind, JoinLink,
    NetworkInterface, PairingInfo, Position, RelayError, RelayRoute, RelaySettings, RelayState,
    RelayStatus, Role, Routes, ShowSnapshot,
};
use midnightsnack_render::{RenderService, SlideSource, TargetSize};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, Notify};

use crate::devices::DeviceStore;
use crate::pairing::Pairing;
use crate::util::{read_json, write_json_atomic};

/// Thumbnail size used by operator previews and remotes.
pub const THUMB_SIZE: TargetSize = TargetSize {
    width: 640,
    height: 360,
};
/// Output size assumed until an output window reports its viewport.
pub const DEFAULT_OUTPUT_SIZE: TargetSize = TargetSize {
    width: 1920,
    height: 1080,
};
/// Slides pre-rendered beyond the next one.
const PREFETCH_AHEAD: usize = 3;

/// A remote's pointer moved; relayed to every other connection.
#[derive(Debug, Clone, PartialEq)]
pub struct PointerUpdate {
    pub device_id: String,
    pub pos: Option<[f32; 2]>,
    pub mode: midnightsnack_protocol::PointerMode,
    pub color: String,
}

/// Something changed; connections decide what to send.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Show,
    Live,
    Devices,
    Pairing,
    RenderProgress(u32),
    /// Disconnect a device (`Some`) or every remote device (`None`).
    Kick(Option<String>),
    /// A device's role changed.
    Session(String),
    Pointer(Arc<PointerUpdate>),
    /// The upload inbox changed (admins).
    Inbox,
    /// API keys were restricted to this computer: drop remote API key connections.
    ApiLocalOnly,
    /// Interfaces, HTTPS or relay status changed (connectivity, pairing links, routes).
    Connectivity,
    /// The OpenSlides meeting data changed (every client).
    OpenSlides,
    /// The OpenSlides connection status changed (admins).
    OpenSlidesStatus,
}

/// Live state of the relay link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelayRuntime {
    pub state: RelayState,
    pub error: Option<RelayError>,
    pub remotes: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub auto_approve: Option<Role>,
    /// Add uploads from every device without asking.
    pub auto_accept_uploads: bool,
    /// API keys only work from this computer.
    pub api_local_only: bool,
    pub osc: midnightsnack_protocol::OscSettings,
    /// Serve HTTPS (self-signed certificate) next to plain HTTP.
    pub https: bool,
    pub relay: RelaySettings,
    pub openslides: crate::openslides_service::OpenSlidesSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            auto_approve: None,
            auto_accept_uploads: false,
            api_local_only: true,
            osc: Default::default(),
            https: false,
            relay: RelaySettings::default(),
            openslides: Default::default(),
        }
    }
}

pub struct AppState {
    pub host: HostInfo,
    pub engine: Mutex<Engine>,
    pub devices: Mutex<DeviceStore>,
    pub pairing: Mutex<Pairing>,
    pub settings: Mutex<Settings>,
    pub render: RenderService,
    pub capture: CaptureHub,
    pub inbox: Mutex<crate::inbox::Inbox>,
    pub max_upload: std::sync::atomic::AtomicU64,
    /// Port the OSC server listens on, if running.
    pub osc_listening: Mutex<Option<u16>>,
    /// Rebinds the OSC server after its settings changed.
    pub osc_restart: Notify,
    pub events: broadcast::Sender<Event>,
    /// media key -> device id
    pub media_keys: Mutex<HashMap<String, String>>,
    pub output_size: Mutex<TargetSize>,
    /// `http://<ip>:<port>` (and `https://…` while HTTPS runs) for each usable interface.
    pub base_urls: Mutex<Vec<String>>,
    /// Usable network interfaces, best first.
    pub interfaces: Mutex<Vec<NetworkInterface>>,
    /// Port of the plain HTTP server.
    pub http_port: std::sync::atomic::AtomicU16,
    pub https_cert: Mutex<Option<crate::https::CertBundle>>,
    /// HTTPS port and certificate fingerprint while it runs.
    pub https_runtime: Mutex<(Option<u16>, Option<String>)>,
    pub https_restart: Notify,
    pub relay_identity: Mutex<crate::relay_link::RelayIdentity>,
    pub relay_runtime: Mutex<RelayRuntime>,
    /// Reconnects the relay link after its settings changed.
    pub relay_restart: Notify,
    pub openslides_password: Mutex<String>,
    pub openslides_runtime: Mutex<crate::openslides_service::OpenSlidesRuntime>,
    /// The selected meeting, as shown by OpenSlides cues.
    pub openslides_data: Mutex<Option<Arc<midnightsnack_protocol::OsMeetingData>>>,
    /// Reconnects to OpenSlides after its settings changed.
    pub openslides_restart: Notify,
    pub data_dir: Option<PathBuf>,
    pub autosave: Notify,
    /// Wakes the auto-advance scheduler after any change.
    pub schedule: Notify,
}

/// Locks a mutex, recovering from poisoning (a panicking connection task must not take the
/// whole show down).
pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl AppState {
    pub fn new(
        host: HostInfo,
        render: RenderService,
        data_dir: Option<PathBuf>,
        engine: Engine,
    ) -> Arc<Self> {
        let settings: Settings = data_dir
            .as_ref()
            .and_then(|d| read_json(&d.join("settings.json")))
            .unwrap_or_default();
        let devices = DeviceStore::load(data_dir.as_ref().map(|d| d.join("devices.json")));
        let (events, _) = broadcast::channel(256);
        Arc::new(AppState {
            host,
            engine: Mutex::new(engine),
            devices: Mutex::new(devices),
            pairing: Mutex::new(Pairing::new(settings.auto_approve)),
            settings: Mutex::new(settings),
            render,
            capture: CaptureHub::new(),
            inbox: Mutex::new(Default::default()),
            max_upload: std::sync::atomic::AtomicU64::new(crate::inbox::DEFAULT_MAX_UPLOAD_BYTES),
            osc_listening: Mutex::new(None),
            osc_restart: Notify::new(),
            events,
            media_keys: Mutex::new(HashMap::new()),
            output_size: Mutex::new(DEFAULT_OUTPUT_SIZE),
            base_urls: Mutex::new(Vec::new()),
            interfaces: Mutex::new(Vec::new()),
            http_port: std::sync::atomic::AtomicU16::new(0),
            https_cert: Mutex::new(None),
            https_runtime: Mutex::new((None, None)),
            https_restart: Notify::new(),
            relay_identity: Mutex::new(crate::relay_link::RelayIdentity::load_or_create(
                data_dir.as_deref(),
            )),
            relay_runtime: Mutex::new(RelayRuntime {
                state: RelayState::Off,
                error: None,
                remotes: 0,
            }),
            relay_restart: Notify::new(),
            openslides_password: Mutex::new(crate::openslides_service::load_password(
                data_dir.as_deref(),
            )),
            openslides_runtime: Mutex::new(Default::default()),
            openslides_data: Mutex::new(None),
            openslides_restart: Notify::new(),
            data_dir,
            autosave: Notify::new(),
            schedule: Notify::new(),
        })
    }

    /// Whether a device may connect from `addr` (host windows and, if restricted, API keys only
    /// from this computer).
    pub fn device_allowed_from(&self, device: &crate::devices::Device, addr: SocketAddr) -> bool {
        allowed_from(
            device,
            addr.ip().is_loopback(),
            lock(&self.settings).api_local_only,
        )
    }

    pub fn control_settings(&self) -> midnightsnack_protocol::ControlSettings {
        let s = lock(&self.settings);
        midnightsnack_protocol::ControlSettings {
            api_local_only: s.api_local_only,
            osc: s.osc,
            osc_listening: *lock(&self.osc_listening),
        }
    }

    pub fn max_upload_bytes(&self) -> u64 {
        self.max_upload.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn emit(&self, event: Event) {
        let _ = self.events.send(event);
    }

    pub fn save_settings(&self) {
        if let Some(d) = &self.data_dir {
            let s = lock(&self.settings).clone();
            if let Err(e) = write_json_atomic(&d.join("settings.json"), &s) {
                tracing::error!(error = %e, "failed to save settings");
            }
        }
    }

    pub fn pairing_info(&self) -> PairingInfo {
        let p = lock(&self.pairing);
        let token = p.join_token().to_owned();
        let interfaces = lock(&self.interfaces).clone();
        let http_port = self.http_port.load(std::sync::atomic::Ordering::Relaxed);
        let https_port = lock(&self.https_runtime).0;
        let mut links: Vec<JoinLink> = interfaces
            .iter()
            .map(|i| JoinLink {
                kind: JoinKind::Lan,
                url: format!("http://{}:{http_port}/join#t={token}", i.address),
                label: i.name.clone(),
            })
            .collect();
        if let Some(port) = https_port {
            links.extend(interfaces.iter().map(|i| JoinLink {
                kind: JoinKind::Https,
                url: format!("https://{}:{port}/join#t={token}", i.address),
                label: i.name.clone(),
            }));
        }
        if lock(&self.relay_runtime).state == RelayState::Connected {
            if let Some(route) = self.relay_route() {
                let label = route
                    .url
                    .split("://")
                    .nth(1)
                    .and_then(|r| r.split('/').next())
                    .unwrap_or_default()
                    .to_owned();
                links.push(JoinLink {
                    kind: JoinKind::Relay,
                    url: format!("{}#t={token}&k={}", route.url, route.key),
                    label,
                });
            }
        }
        PairingInfo {
            pin: p.pin().to_owned(),
            links,
            auto_approve: p.auto_approve,
        }
    }

    /// The relay page and key remotes use, while a relay is configured.
    pub fn relay_route(&self) -> Option<RelayRoute> {
        let settings = lock(&self.settings).relay.clone();
        if !settings.enabled {
            return None;
        }
        let base = crate::relay_link::normalize_url(&settings.url).ok()?;
        let id = lock(&self.relay_identity).clone();
        Some(RelayRoute {
            url: format!("{base}/r/{}", id.host_id()),
            key: id.public_key_b64(),
        })
    }

    /// Other ways to reach this host, for paired remotes.
    pub fn routes(&self) -> Routes {
        // Encrypted first: a phone leaving the relay should not fall back to plain HTTP when
        // HTTPS is on.
        let mut lan = lock(&self.base_urls).clone();
        lan.sort_by_key(|u| !u.starts_with("https://"));
        Routes {
            lan,
            relay: self.relay_route(),
        }
    }

    pub fn connectivity(&self) -> ConnectivityInfo {
        let settings = lock(&self.settings).clone();
        let (https_port, fingerprint) = lock(&self.https_runtime).clone();
        let relay = *lock(&self.relay_runtime);
        ConnectivityInfo {
            port: self.http_port.load(std::sync::atomic::Ordering::Relaxed),
            interfaces: lock(&self.interfaces).clone(),
            https: HttpsStatus {
                enabled: settings.https,
                port: https_port,
                fingerprint,
            },
            relay: RelayStatus {
                enabled: settings.relay.enabled,
                url: settings.relay.url.clone(),
                has_access_token: settings
                    .relay
                    .access_token
                    .as_deref()
                    .is_some_and(|t| !t.is_empty()),
                state: relay.state,
                error: relay.error,
                host_id: lock(&self.relay_identity).host_id(),
                remotes: relay.remotes,
            },
        }
    }

    pub fn show_snapshot(&self, role: Role) -> ShowSnapshot {
        lock(&self.engine).show_snapshot(role == Role::Admin)
    }

    /// The single entry point for actions from any transport.
    pub async fn perform(
        self: &Arc<Self>,
        origin: Origin,
        action: Action,
    ) -> Result<(), ErrorCode> {
        let result = {
            let mut engine = lock(&self.engine);
            dispatch(&mut engine, origin, action, now_ms())
        };
        match result? {
            Dispatched::Applied(change) => {
                self.after_change(change);
                Ok(())
            }
            Dispatched::Host(action) => crate::host_actions::run(self, *action).await,
        }
    }

    /// Broadcasts a change, schedules prefetching and autosave.
    pub fn after_change(&self, change: Change) {
        if change.show {
            self.emit(Event::Show);
        }
        if change.live {
            self.emit(Event::Live);
        }
        if change.any() {
            self.prefetch();
            self.autosave.notify_one();
            self.schedule.notify_one();
        }
    }

    /// Resolves a slide to something the renderer understands. `None` for blank cues or
    /// invalid positions.
    pub fn slide_source(&self, cue_id: &str, slide: u32) -> Option<SlideSource> {
        let engine = lock(&self.engine);
        slide_source(engine.show(), cue_id, slide)
    }

    /// Pre-renders what is likely to be shown next at output and thumbnail size.
    pub fn prefetch(&self) {
        let output = *lock(&self.output_size);
        let sources: Vec<SlideSource> = {
            let engine = lock(&self.engine);
            let show = engine.show();
            let live = engine.live_state(0);
            let mut positions: Vec<Position> =
                [live.output, live.program, live.next.clone(), live.prev]
                    .into_iter()
                    .flatten()
                    .collect();
            if let Some(next) = live.next {
                positions.extend(following(show, &next, PREFETCH_AHEAD - 1));
            }
            positions
                .iter()
                .filter_map(|p| slide_source(show, &p.cue_id, p.slide))
                .collect()
        };
        for s in sources {
            self.render.prefetch(s.clone(), output);
            self.render.prefetch(s, THUMB_SIZE);
        }
    }
}

/// Recomputes the base URLs from interfaces and ports.
pub fn refresh_base_urls(state: &AppState) {
    let http_port = state.http_port.load(std::sync::atomic::Ordering::Relaxed);
    let https_port = lock(&state.https_runtime).0;
    let interfaces = lock(&state.interfaces).clone();
    let mut urls: Vec<String> = interfaces
        .iter()
        .map(|i| format!("http://{}:{http_port}", i.address))
        .collect();
    if let Some(port) = https_port {
        urls.extend(
            interfaces
                .iter()
                .map(|i| format!("https://{}:{port}", i.address)),
        );
    }
    *lock(&state.base_urls) = urls;
}

/// Where converted office documents are stored.
pub fn converted_dir(state: &AppState) -> std::path::PathBuf {
    state.render.cache().root().join("converted")
}

pub fn slide_source(show: &Show, cue_id: &str, slide: u32) -> Option<SlideSource> {
    use midnightsnack_core::CueContent::*;
    let cue = show.cue(cue_id)?;
    if slide >= cue.slide_count() {
        return None;
    }
    match &cue.content {
        Pdf { file, .. } => Some(SlideSource::PdfPage {
            path: show.resolve(file)?,
            page: slide,
        }),
        Image { file } => Some(SlideSource::Image {
            path: show.resolve(file)?,
        }),
        ImageFolder { files } => Some(SlideSource::Image {
            path: show.resolve(files.get(slide as usize)?)?,
        }),
        // Rendered by the clients themselves (video element, text layout).
        Blank { .. }
        | Media { .. }
        | Text { .. }
        | Timer { .. }
        | Web { .. }
        | Capture { .. }
        | OpenSlides { .. } => None,
    }
}

/// Up to `n` positions after `from`, crossing cue boundaries.
fn following(show: &Show, from: &Position, n: usize) -> Vec<Position> {
    let mut out = Vec::new();
    let Some(start) = show.cue_index(&from.cue_id) else {
        return out;
    };
    let mut slide = from.slide + 1;
    for cue in &show.cues[start..] {
        while slide < cue.slide_count() {
            if out.len() == n {
                return out;
            }
            out.push(Position {
                cue_id: cue.id.clone(),
                slide,
            });
            slide += 1;
        }
        slide = 0;
    }
    out
}

fn allowed_from(device: &crate::devices::Device, loopback: bool, api_local_only: bool) -> bool {
    loopback || !(device.local || (device.api_key && api_local_only))
}

#[cfg(test)]
mod address_tests {
    use super::*;
    use crate::devices::Device;

    fn device(local: bool, api_key: bool) -> Device {
        Device {
            id: "d".into(),
            name: "d".into(),
            role: Role::Operator,
            local,
            api_key,
        }
    }

    #[test]
    fn host_windows_and_restricted_api_keys_stay_on_this_computer() {
        let phone = device(false, false);
        let window = device(true, false);
        let key = device(false, true);
        for d in [&phone, &window, &key] {
            assert!(allowed_from(d, true, true), "loopback is always fine");
        }
        assert!(allowed_from(&phone, false, true));
        assert!(!allowed_from(&window, false, false));
        assert!(!allowed_from(&key, false, true));
        assert!(allowed_from(&key, false, false));
    }
}
