// SPDX-License-Identifier: GPL-3.0-or-later
//! Actions that need the file system, the renderer or the device store.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use midnightsnack_core::bundle::{self, SHOW_FILE_EXTENSION};
use midnightsnack_core::{Cue, CueContent, MediaRef, Show};
use midnightsnack_protocol::{Action, ErrorCode};
use midnightsnack_render::{is_supported_image, list_image_folder};

use crate::state::{lock, AppState, Event};

pub async fn run(state: &Arc<AppState>, action: Action) -> Result<(), ErrorCode> {
    match action {
        Action::AddFiles { paths, at_index } => add_files(state, paths, at_index).await,
        Action::NewShow => {
            state.render.cancel_prefetch();
            let c = lock(&state.engine).replace_show(Show::default(), None);
            state.after_change(c);
            Ok(())
        }
        Action::OpenShow { path } => open_show(state, PathBuf::from(path)).await,
        Action::SaveShow { path, embed_media } => save_show(state, path, embed_media).await,
        Action::ApprovePairing { request_id, role } => {
            let name = lock(&state.pairing)
                .device_name(&request_id)
                .ok_or(ErrorCode::NotFound)?;
            let (device_id, token) = lock(&state.devices).add(&name, role);
            lock(&state.pairing).approve(&request_id, device_id, token, role);
            state.emit(Event::Devices);
            Ok(())
        }
        Action::DenyPairing { request_id } => {
            if !lock(&state.pairing).deny(&request_id) {
                return Err(ErrorCode::NotFound);
            }
            state.emit(Event::Devices);
            Ok(())
        }
        Action::SetDeviceRole { device_id, role } => {
            if !lock(&state.devices).set_role(&device_id, role) {
                return Err(ErrorCode::NotFound);
            }
            state.emit(Event::Session(device_id));
            state.emit(Event::Devices);
            Ok(())
        }
        Action::RevokeDevice { device_id } => {
            if !lock(&state.devices).revoke(&device_id) {
                return Err(ErrorCode::NotFound);
            }
            state.emit(Event::Kick(Some(device_id)));
            state.emit(Event::Devices);
            Ok(())
        }
        Action::DisconnectAll => {
            lock(&state.devices).revoke_all();
            lock(&state.pairing).rotate();
            state.emit(Event::Kick(None));
            state.emit(Event::Devices);
            state.emit(Event::Pairing);
            Ok(())
        }
        Action::SetAutoApprove { role } => {
            lock(&state.pairing).auto_approve = role;
            lock(&state.settings).auto_approve = role;
            state.save_settings();
            state.emit(Event::Pairing);
            Ok(())
        }
        Action::SetLogoImage { path } => {
            let asset = image_asset(state, path)?;
            let c = lock(&state.engine).set_logo_asset(asset);
            state.after_change(c);
            Ok(())
        }
        Action::SetBackgroundImage { cue_id, path } => {
            let asset = image_asset(state, path)?;
            let c = lock(&state.engine).set_background_asset(cue_id.as_deref(), asset)?;
            state.after_change(c);
            Ok(())
        }
        Action::SetOverlayImage { overlay_id, path } => {
            let asset = image_asset(state, path)?;
            let c = lock(&state.engine).set_overlay_asset(&overlay_id, asset)?;
            state.after_change(c);
            Ok(())
        }
        _ => Err(ErrorCode::InvalidState),
    }
}

/// Registers an image file as a show asset; `None` passes through (clears the image).
fn image_asset(state: &AppState, path: Option<String>) -> Result<Option<String>, ErrorCode> {
    let Some(path) = path else { return Ok(None) };
    let path = std::fs::canonicalize(path).map_err(|_| ErrorCode::NotFound)?;
    if !path.is_file() || !is_supported_image(&path) {
        return Err(ErrorCode::UnsupportedFile);
    }
    Ok(Some(lock(&state.engine).add_asset(MediaRef::linked(path))))
}

const VIDEO_EXTENSIONS: &[&str] = &["mp4", "m4v", "mov", "webm", "mkv", "ogv"];
const AUDIO_EXTENSIONS: &[&str] = &["mp3", "m4a", "aac", "wav", "ogg", "oga", "opus", "flac"];

async fn add_files(
    state: &Arc<AppState>,
    paths: Vec<String>,
    at_index: Option<u32>,
) -> Result<(), ErrorCode> {
    let mut cues = Vec::new();
    let mut first_error = None;
    for p in paths {
        match cue_for_path(state, Path::new(&p)).await {
            Ok(cue) => cues.push(cue),
            Err(e) => {
                tracing::warn!(path = %p, ?e, "cannot add file");
                first_error.get_or_insert(e);
            }
        }
    }
    if !cues.is_empty() {
        let c = lock(&state.engine).insert_cues(cues, at_index.map(|i| i as usize));
        state.after_change(c);
    }
    first_error.map_or(Ok(()), Err)
}

fn display_name(path: &Path) -> String {
    path.file_stem()
        .or_else(|| path.file_name())
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Untitled".into())
}

async fn cue_for_path(state: &Arc<AppState>, path: &Path) -> Result<Cue, ErrorCode> {
    let path = std::fs::canonicalize(path).map_err(|_| ErrorCode::NotFound)?;
    if path.is_dir() {
        let dir = path.clone();
        let files = tokio::task::spawn_blocking(move || list_image_folder(&dir))
            .await
            .map_err(|_| ErrorCode::Internal)?
            .map_err(|_| ErrorCode::Io)?;
        if files.is_empty() {
            return Err(ErrorCode::UnsupportedFile);
        }
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        return Ok(Cue::new(
            name,
            CueContent::ImageFolder {
                files: files.into_iter().map(MediaRef::linked).collect(),
            },
        ));
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    match ext.as_deref() {
        Some("pdf") => {
            let info = state
                .render
                .inspect_pdf(path.clone())
                .await
                .map_err(|e| e.code())?;
            let mut cue = Cue::new(
                display_name(&path),
                CueContent::Pdf {
                    file: MediaRef::linked(&path),
                    page_count: info.page_count,
                },
            );
            cue.slide_notes = info.notes;
            Ok(cue)
        }
        Some(e) if VIDEO_EXTENSIONS.contains(&e) || AUDIO_EXTENSIONS.contains(&e) => Ok(Cue::new(
            display_name(&path),
            CueContent::Media {
                file: MediaRef::linked(&path),
                video: VIDEO_EXTENSIONS.contains(&e),
                options: Default::default(),
                duration_ms: None,
            },
        )),
        _ if is_supported_image(&path) => Ok(Cue::new(
            display_name(&path),
            CueContent::Image {
                file: MediaRef::linked(&path),
            },
        )),
        _ => Err(ErrorCode::UnsupportedFile),
    }
}

async fn open_show(state: &Arc<AppState>, path: PathBuf) -> Result<(), ErrorCode> {
    let extract = shows_dir(state).join(uuid::Uuid::new_v4().to_string());
    let p = path.clone();
    let show = tokio::task::spawn_blocking(move || bundle::open(&p, &extract))
        .await
        .map_err(|_| ErrorCode::Internal)?
        .map_err(|e| {
            tracing::warn!(error = %e, "failed to open show");
            e.code()
        })?;
    state.render.cancel_prefetch();
    let c = lock(&state.engine).replace_show(show, Some(path));
    state.after_change(c);
    Ok(())
}

async fn save_show(
    state: &Arc<AppState>,
    path: Option<String>,
    embed_media: bool,
) -> Result<(), ErrorCode> {
    let (show, current) = {
        let e = lock(&state.engine);
        (e.show().clone(), e.path().cloned())
    };
    let mut path = path
        .map(PathBuf::from)
        .or(current)
        .ok_or(ErrorCode::InvalidState)?;
    if path.extension().is_none_or(|e| e != SHOW_FILE_EXTENSION) {
        path.set_extension(SHOW_FILE_EXTENSION);
    }
    let p = path.clone();
    tokio::task::spawn_blocking(move || bundle::save(&show, &p, embed_media))
        .await
        .map_err(|_| ErrorCode::Internal)?
        .map_err(|e| {
            tracing::warn!(error = %e, "failed to save show");
            e.code()
        })?;
    let c = lock(&state.engine).mark_saved(path);
    state.after_change(c);
    Ok(())
}

/// Where embedded media of opened bundles is extracted.
pub fn shows_dir(state: &AppState) -> PathBuf {
    match &state.data_dir {
        Some(d) => d.join("shows"),
        None => std::env::temp_dir().join("midnightsnack-shows"),
    }
}
