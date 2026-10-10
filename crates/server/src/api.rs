// SPDX-License-Identifier: GPL-3.0-or-later
//! REST endpoints: host info, pairing and slide images.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{ConnectInfo, Path, Query, Request, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use midnightsnack_protocol::{ApiError, ErrorCode, HostInfo, PairRequest, PairResponse};
use midnightsnack_render::TargetSize;
use serde::Deserialize;
use tower::ServiceExt;
use tower_http::services::ServeFile;

use crate::pairing::Submitted;
use crate::state::{lock, AppState, Event, THUMB_SIZE};

pub struct ApiFailure(pub StatusCode, pub ErrorCode);

impl IntoResponse for ApiFailure {
    fn into_response(self) -> Response {
        (self.0, Json(ApiError { code: self.1 })).into_response()
    }
}

impl From<ErrorCode> for ApiFailure {
    fn from(code: ErrorCode) -> Self {
        let status = match code {
            ErrorCode::Unauthorized | ErrorCode::InvalidPin | ErrorCode::InvalidJoinToken => {
                StatusCode::UNAUTHORIZED
            }
            ErrorCode::Forbidden | ErrorCode::LocalOnly => StatusCode::FORBIDDEN,
            ErrorCode::PairingLocked => StatusCode::TOO_MANY_REQUESTS,
            ErrorCode::NotFound => StatusCode::NOT_FOUND,
            ErrorCode::MalformedMessage | ErrorCode::InvalidState => StatusCode::BAD_REQUEST,
            ErrorCode::PdfEngineMissing => StatusCode::SERVICE_UNAVAILABLE,
            ErrorCode::UnsupportedFile => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            ErrorCode::FileTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            ErrorCode::InboxFull => StatusCode::TOO_MANY_REQUESTS,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        ApiFailure(status, code)
    }
}

pub async fn info(State(state): State<Arc<AppState>>) -> Json<HostInfo> {
    Json(state.host.clone())
}

pub async fn pair(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(req): Json<PairRequest>,
) -> Result<Json<PairResponse>, ApiFailure> {
    let submitted =
        lock(&state.pairing).submit(&req.join_token, &req.pin, &req.device_name, addr.ip())?;
    let request_id = match submitted {
        Submitted::Pending(id) => id,
        Submitted::AutoApprove(id, role) => {
            let name = lock(&state.pairing).device_name(&id).unwrap_or_default();
            let (device_id, token) = lock(&state.devices).add(&name, role);
            lock(&state.pairing).approve(&id, device_id, token, role);
            id
        }
    };
    tracing::info!(%addr, device = %req.device_name, "pairing request");
    state.emit(Event::Devices);
    state.emit(Event::Pairing);
    Ok(Json(PairResponse { request_id }))
}

pub async fn pair_status(
    State(state): State<Arc<AppState>>,
    Path(request_id): Path<String>,
) -> Response {
    match lock(&state.pairing).status(&request_id) {
        Some(status) => Json(status).into_response(),
        None => ApiFailure::from(ErrorCode::NotFound).into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct SlideQuery {
    k: String,
    w: Option<u32>,
    h: Option<u32>,
}

pub async fn slide(
    State(state): State<Arc<AppState>>,
    Path((cue_id, slide)): Path<(String, u32)>,
    Query(q): Query<SlideQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiFailure> {
    if !lock(&state.media_keys).contains_key(&q.k) {
        return Err(ErrorCode::Unauthorized.into());
    }
    let source = state
        .slide_source(&cue_id, slide)
        .ok_or(ErrorCode::NotFound)?;
    let size = match (q.w, q.h) {
        (Some(w), Some(h)) => snap_size(w, h, *lock(&state.output_size)),
        _ => THUMB_SIZE,
    };
    let path = state
        .render
        .render(source, size)
        .await
        .map_err(|e| ApiFailure::from(e.code()))?;
    let etag = format!(
        "\"{}\"",
        path.parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
            + "-"
            + &path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
    );
    if headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        == Some(etag.as_str())
    {
        return Ok(StatusCode::NOT_MODIFIED.into_response());
    }
    let bytes = tokio::fs::read(&path).await.map_err(|_| ErrorCode::Io)?;
    let mime = if path.extension().is_some_and(|e| e == "png") {
        "image/png"
    } else {
        "image/jpeg"
    };
    Ok((
        [
            (header::CONTENT_TYPE, mime.to_owned()),
            (header::CACHE_CONTROL, "private, no-cache".to_owned()),
            (header::ETAG, etag),
        ],
        Body::from(bytes),
    )
        .into_response())
}

/// Render sizes a client may ask for besides the outputs' own size: each distinct size is a
/// new render and a new cache file, so arbitrary sizes would let any device fill the disk.
const SIZE_STEPS: [(u32, u32); 6] = [
    (640, 360),
    (1280, 720),
    (1920, 1080),
    (2560, 1440),
    (3840, 2160),
    (7680, 4320),
];

/// The output size if that is what was asked for, else the smallest step that covers it.
pub(crate) fn snap_size(w: u32, h: u32, output: TargetSize) -> TargetSize {
    let wanted = TargetSize::new(w, h);
    if wanted == output {
        return output;
    }
    let (w, h) = SIZE_STEPS
        .into_iter()
        .find(|&(sw, sh)| sw >= wanted.width && sh >= wanted.height)
        .unwrap_or(SIZE_STEPS[SIZE_STEPS.len() - 1]);
    TargetSize::new(w, h)
}

#[derive(Debug, Deserialize)]
pub struct KeyQuery {
    k: String,
}

/// Serves a file with HTTP range support (needed for seeking in videos).
async fn serve_file(path: std::path::PathBuf, req: Request) -> Result<Response, ApiFailure> {
    let res = ServeFile::new(path)
        .oneshot(req)
        .await
        .map_err(|_| ErrorCode::Io)?;
    if res.status() == StatusCode::NOT_FOUND {
        return Err(ErrorCode::NotFound.into());
    }
    let mut res = res.map(Body::new);
    res.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("private, no-cache"),
    );
    Ok(res)
}

/// The original file of a video or audio cue.
pub async fn media_file(
    State(state): State<Arc<AppState>>,
    Path(cue_id): Path<String>,
    Query(q): Query<KeyQuery>,
    req: Request,
) -> Result<Response, ApiFailure> {
    if !lock(&state.media_keys).contains_key(&q.k) {
        return Err(ErrorCode::Unauthorized.into());
    }
    let path = {
        let engine = lock(&state.engine);
        let show = engine.show();
        match &show.cue(&cue_id).ok_or(ErrorCode::NotFound)?.content {
            midnightsnack_core::CueContent::Media { file, .. } => show.resolve(file),
            _ => None,
        }
        .ok_or(ErrorCode::NotFound)?
    };
    // Defense in depth: shows are checked when opened.
    if !crate::host_actions::is_media_file(&path) {
        return Err(ErrorCode::NotFound.into());
    }
    serve_file(path, req).await
}

/// An image asset (logo, background, logo bug).
pub async fn asset(
    State(state): State<Arc<AppState>>,
    Path(asset_id): Path<String>,
    Query(q): Query<KeyQuery>,
    req: Request,
) -> Result<Response, ApiFailure> {
    if !lock(&state.media_keys).contains_key(&q.k) {
        return Err(ErrorCode::Unauthorized.into());
    }
    let path = {
        let engine = lock(&state.engine);
        let show = engine.show();
        show.asset(&asset_id)
            .and_then(|a| show.resolve(&a.file))
            .ok_or(ErrorCode::NotFound)?
    };
    if !midnightsnack_render::is_supported_image(&path) {
        return Err(ErrorCode::NotFound.into());
    }
    serve_file(path, req).await
}

#[derive(Debug, Deserialize)]
pub struct CaptureQuery {
    k: String,
    /// Frames per second for this viewer (phones ask for fewer).
    fps: Option<u32>,
}

/// Live capture as an MJPEG stream (`multipart/x-mixed-replace`), which `<img>` displays
/// natively. When the source is lost the stream simply pauses, so viewers keep the last frame.
pub async fn capture(
    State(state): State<Arc<AppState>>,
    Path(cue_id): Path<String>,
    Query(q): Query<CaptureQuery>,
) -> Result<Response, ApiFailure> {
    if !lock(&state.media_keys).contains_key(&q.k) {
        return Err(ErrorCode::Unauthorized.into());
    }
    let info = {
        let engine = lock(&state.engine);
        match &engine
            .show()
            .cue(&cue_id)
            .ok_or(ErrorCode::NotFound)?
            .content
        {
            midnightsnack_core::CueContent::Capture { capture } => capture.clone(),
            _ => return Err(ErrorCode::NotFound.into()),
        }
    };
    let fps = q.fps.unwrap_or(info.fps).clamp(1, info.fps.max(1));
    let mut rx = state.capture.subscribe(&info.source, fps);
    let min_gap = std::time::Duration::from_secs_f64(1.0 / fps as f64);
    let stream = async_stream(move |tx| async move {
        let mut last = tokio::time::Instant::now() - min_gap;
        loop {
            if rx.changed().await.is_err() {
                return;
            }
            let wait = (last + min_gap).saturating_duration_since(tokio::time::Instant::now());
            if !wait.is_zero() {
                tokio::time::sleep(wait).await;
            }
            last = tokio::time::Instant::now();
            let Some(frame) = rx.borrow_and_update().clone() else {
                continue;
            };
            let head = format!(
                "--frame\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
                frame.jpeg.len()
            );
            let mut part = bytes::BytesMut::with_capacity(head.len() + frame.jpeg.len() + 2);
            part.extend_from_slice(head.as_bytes());
            part.extend_from_slice(&frame.jpeg);
            part.extend_from_slice(b"\r\n");
            if tx
                .send(Ok::<_, std::io::Error>(part.freeze()))
                .await
                .is_err()
            {
                return;
            }
        }
    });
    Ok((
        [
            (
                header::CONTENT_TYPE,
                "multipart/x-mixed-replace; boundary=frame".to_owned(),
            ),
            (header::CACHE_CONTROL, "no-store".to_owned()),
        ],
        Body::from_stream(stream),
    )
        .into_response())
}

/// A stream fed by a task through a bounded channel (backpressure: slow viewers skip frames).
fn async_stream<F, Fut, T>(f: F) -> impl futures_util::Stream<Item = T>
where
    F: FnOnce(tokio::sync::mpsc::Sender<T>) -> Fut,
    Fut: std::future::Future<Output = ()> + Send + 'static,
    T: Send + 'static,
{
    let (tx, rx) = tokio::sync::mpsc::channel(1);
    tokio::spawn(f(tx));
    futures_util::stream::unfold(rx, |mut rx| async move { rx.recv().await.map(|v| (v, rx)) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_sizes_snap_to_a_few_steps() {
        let out = TargetSize::new(1366, 768);
        assert_eq!(snap_size(1366, 768, out), out);
        assert_eq!(snap_size(100, 100, out), TargetSize::new(640, 360));
        assert_eq!(snap_size(1300, 700, out), TargetSize::new(1920, 1080));
        assert_eq!(snap_size(7680, 7679, out), TargetSize::new(7680, 4320));
        let distinct: std::collections::HashSet<_> = (16..4000)
            .map(|w| snap_size(w, w / 2, out))
            .map(|s| (s.width, s.height))
            .collect();
        assert!(distinct.len() <= SIZE_STEPS.len() + 1);
    }
}
