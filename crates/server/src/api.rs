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
        (Some(w), Some(h)) => TargetSize::new(w, h),
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
