// SPDX-License-Identifier: GPL-3.0-or-later
//! Files sent from devices (`POST /api/v1/upload`). They wait in the inbox until an admin accepts
//! (the file becomes a cue) or rejects them (the file is deleted). Uploads by admins, and all
//! uploads while auto-accept is on, become cues right away.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{ConnectInfo, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::Json;
use futures_util::StreamExt;
use midnightsnack_core::now_ms;
use midnightsnack_protocol::{ErrorCode, InboxItem, Role, UploadResponse};
use serde::Deserialize;
use tokio::io::AsyncWriteExt;

use crate::api::ApiFailure;
use crate::devices::Device;
use crate::state::{lock, AppState, Event};

/// Default largest accepted upload (videos can be big).
pub const DEFAULT_MAX_UPLOAD_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_NAME_CHARS: usize = 120;

/// File types that can become cues.
pub const UPLOAD_EXTENSIONS: &[&str] = &[
    "pdf", "pptx", "ppt", "pps", "ppsx", "odp", "key", "png", "jpg", "jpeg", "gif", "webp", "bmp",
    "tif", "tiff", "mp4", "m4v", "mov", "webm", "mkv", "ogv", "mp3", "m4a", "aac", "wav", "ogg",
    "oga", "opus", "flac",
];

struct Entry {
    item: InboxItem,
    path: PathBuf,
}

#[derive(Default)]
pub struct Inbox {
    entries: Vec<Entry>,
}

impl Inbox {
    pub fn items(&self) -> Vec<InboxItem> {
        self.entries.iter().map(|e| e.item.clone()).collect()
    }

    fn take(&mut self, id: &str) -> Option<Entry> {
        let i = self.entries.iter().position(|e| e.item.id == id)?;
        Some(self.entries.remove(i))
    }
}

/// Pending uploads; emptied on every start (pending items are not remembered).
fn inbox_dir(state: &AppState) -> PathBuf {
    data_root(state).join("inbox")
}

/// Accepted uploads, referenced by the show.
fn uploads_dir(state: &AppState) -> PathBuf {
    data_root(state).join("uploads")
}

fn data_root(state: &AppState) -> PathBuf {
    match &state.data_dir {
        Some(d) => d.clone(),
        None => std::env::temp_dir().join("midnightsnack-data"),
    }
}

/// Removes leftovers of uploads that were never decided on.
pub fn clear_pending(state: &AppState) {
    let _ = std::fs::remove_dir_all(inbox_dir(state));
}

/// The file name to keep: one path component, printable, limited length, allowed extension.
pub fn sanitize_name(name: &str) -> Option<String> {
    let base = name.rsplit(['/', '\\']).next()?.trim();
    let clean: String = base
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        .collect();
    let clean = clean.trim_start_matches('.').trim().to_owned();
    let ext = Path::new(&clean)
        .extension()?
        .to_str()?
        .to_ascii_lowercase();
    if !UPLOAD_EXTENSIONS.contains(&ext.as_str()) {
        return None;
    }
    if clean.chars().count() <= MAX_NAME_CHARS {
        return Some(clean);
    }
    let stem: String = clean.chars().take(MAX_NAME_CHARS - ext.len() - 1).collect();
    Some(format!("{stem}.{ext}"))
}

/// The device behind a `Authorization: Bearer <token>` header.
pub fn authenticate(
    state: &AppState,
    headers: &HeaderMap,
    addr: SocketAddr,
) -> Result<Device, ApiFailure> {
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(ErrorCode::Unauthorized)?;
    let device = lock(&state.devices)
        .authenticate(token.trim())
        .ok_or(ErrorCode::Unauthorized)?;
    if !state.device_allowed_from(&device, addr) {
        return Err(ErrorCode::Unauthorized.into());
    }
    Ok(device)
}

#[derive(Deserialize)]
pub struct UploadQuery {
    name: String,
}

pub async fn upload(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Query(q): Query<UploadQuery>,
    body: Body,
) -> Result<Json<UploadResponse>, ApiFailure> {
    let device = authenticate(&state, &headers, addr)?;
    if device.role < Role::Presenter {
        return Err(ErrorCode::Forbidden.into());
    }
    let name = sanitize_name(&q.name).ok_or(ApiFailure(
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        ErrorCode::UnsupportedFile,
    ))?;
    let max = state.max_upload_bytes();
    let too_large = ApiFailure(StatusCode::PAYLOAD_TOO_LARGE, ErrorCode::FileTooLarge);
    let declared = headers
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());
    if declared.is_some_and(|n| n > max) {
        return Err(too_large);
    }

    let id = uuid::Uuid::new_v4().simple().to_string();
    let dir = inbox_dir(&state).join(&id);
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|_| ErrorCode::Io)?;
    let path = dir.join(&name);
    let size = match receive(body, &path, max).await {
        Ok(n) => n,
        Err(e) => {
            let _ = tokio::fs::remove_dir_all(&dir).await;
            return Err(e);
        }
    };
    tracing::info!(device = %device.name, file = %name, size, "file uploaded");
    lock(&state.inbox).entries.push(Entry {
        item: InboxItem {
            id: id.clone(),
            file_name: name,
            size,
            device_name: device.name.clone(),
            received_ms: now_ms(),
        },
        path,
    });
    let auto = device.role == Role::Admin || lock(&state.settings).auto_accept_uploads;
    if auto {
        accept(&state, &id, None).await?;
    } else {
        state.emit(Event::Inbox);
    }
    Ok(Json(UploadResponse {
        upload_id: id,
        added: auto,
    }))
}

/// Streams the body to `path`, failing once it exceeds `max` bytes.
async fn receive(body: Body, path: &Path, max: u64) -> Result<u64, ApiFailure> {
    let mut file = tokio::fs::File::create(path)
        .await
        .map_err(|_| ErrorCode::Io)?;
    let mut stream = body.into_data_stream();
    let mut size = 0u64;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| ErrorCode::Io)?;
        size += chunk.len() as u64;
        if size > max {
            return Err(ApiFailure(
                StatusCode::PAYLOAD_TOO_LARGE,
                ErrorCode::FileTooLarge,
            ));
        }
        file.write_all(&chunk).await.map_err(|_| ErrorCode::Io)?;
    }
    file.flush().await.map_err(|_| ErrorCode::Io)?;
    if size == 0 {
        return Err(ErrorCode::UnsupportedFile.into());
    }
    Ok(size)
}

/// Moves the upload out of the inbox and adds it to the show. On failure (e.g. no converter
/// for a presentation) it stays in the inbox.
pub async fn accept(
    state: &Arc<AppState>,
    id: &str,
    at_index: Option<u32>,
) -> Result<(), ErrorCode> {
    let entry = lock(&state.inbox).take(id).ok_or(ErrorCode::NotFound)?;
    let dest_dir = uploads_dir(state).join(id);
    let dest = dest_dir.join(&entry.item.file_name);
    let moved = tokio::fs::create_dir_all(&dest_dir).await.is_ok()
        && tokio::fs::rename(&entry.path, &dest).await.is_ok();
    if !moved {
        lock(&state.inbox).entries.push(entry);
        return Err(ErrorCode::Io);
    }
    let result =
        crate::host_actions::add_paths(state, vec![dest.to_string_lossy().into_owned()], at_index)
            .await;
    match result {
        Ok(()) => {
            if let Some(parent) = entry.path.parent() {
                let _ = tokio::fs::remove_dir_all(parent).await;
            }
            state.emit(Event::Inbox);
            Ok(())
        }
        Err(e) => {
            // Back into the inbox so the admin can retry (after installing a converter) or
            // reject it.
            let _ = tokio::fs::rename(&dest, &entry.path).await;
            let _ = tokio::fs::remove_dir_all(&dest_dir).await;
            lock(&state.inbox).entries.push(entry);
            state.emit(Event::Inbox);
            Err(e)
        }
    }
}

pub async fn reject(state: &Arc<AppState>, id: &str) -> Result<(), ErrorCode> {
    let entry = lock(&state.inbox).take(id).ok_or(ErrorCode::NotFound)?;
    if let Some(parent) = entry.path.parent() {
        let _ = tokio::fs::remove_dir_all(parent).await;
    }
    state.emit(Event::Inbox);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_reduced_to_a_safe_file_name() {
        assert_eq!(sanitize_name("talk.PDF").as_deref(), Some("talk.PDF"));
        assert_eq!(
            sanitize_name("../../etc/evil.pdf").as_deref(),
            Some("evil.pdf")
        );
        assert_eq!(
            sanitize_name("C:\\Users\\me\\deck.pptx").as_deref(),
            Some("deck.pptx")
        );
        assert_eq!(sanitize_name("..hidden.png").as_deref(), Some("hidden.png"));
        assert_eq!(sanitize_name("run.sh"), None);
        assert_eq!(sanitize_name("noext"), None);
        assert_eq!(sanitize_name("a\u{0}b?.jpg").as_deref(), Some("ab.jpg"));
        let long = format!("{}.mp4", "x".repeat(500));
        let short = sanitize_name(&long).unwrap();
        assert_eq!(short.chars().count(), MAX_NAME_CHARS);
        assert!(short.ends_with(".mp4"));
    }
}
