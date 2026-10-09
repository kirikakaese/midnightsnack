// SPDX-License-Identifier: GPL-3.0-or-later
//! Small HTTP API for scripts and control surfaces: send an action, read the state. Requests
//! authenticate with a device token or API key (`Authorization: Bearer <token>`).

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::{ConnectInfo, State};
use axum::http::HeaderMap;
use axum::Json;
use midnightsnack_core::{now_ms, Origin};
use midnightsnack_protocol::{Action, ErrorCode, StateSummary};

use crate::api::ApiFailure;
use crate::inbox::authenticate;
use crate::state::{lock, AppState};

/// `POST /api/v1/action` with an `Action` as JSON body. Answers `{}` or an `ApiError`.
pub async fn action(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: Result<Json<Action>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<serde_json::Value>, ApiFailure> {
    let device = authenticate(&state, &headers, addr)?;
    let Json(action) = body.map_err(|_| ErrorCode::MalformedMessage)?;
    let origin = Origin {
        role: device.role,
        local: device.local,
    };
    state.perform(origin, action).await?;
    Ok(Json(serde_json::json!({})))
}

/// `GET /api/v1/state`: what is live, masters and timers.
pub async fn summary(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Result<Json<StateSummary>, ApiFailure> {
    authenticate(&state, &headers, addr)?;
    Ok(Json(lock(&state.engine).summary(now_ms())))
}
