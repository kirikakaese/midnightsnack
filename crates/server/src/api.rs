// SPDX-License-Identifier: GPL-3.0-or-later
//! REST endpoints: host info, pairing and slide images.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{ConnectInfo, Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use midnightsnack_protocol::{ApiError, ErrorCode, HostInfo, PairRequest, PairResponse};
use midnightsnack_render::TargetSize;
use serde::Deserialize;

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
