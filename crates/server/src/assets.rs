// SPDX-License-Identifier: GPL-3.0-or-later
//! The web remote, embedded into the binary at build time (`apps/remote/dist`).

use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../../apps/remote/dist"]
#[allow_missing = true]
struct RemoteAssets;

const CSP: &str = "default-src 'self'; img-src 'self' data: blob:; connect-src 'self' ws: wss:; \
                   style-src 'self' 'unsafe-inline'; frame-ancestors 'none'";

pub async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let (file, path) = match RemoteAssets::get(path) {
        Some(f) if !path.is_empty() => (f, path.to_owned()),
        // Single-page app: unknown paths (e.g. /join) get index.html.
        _ => match RemoteAssets::get("index.html") {
            Some(f) => (f, "index.html".to_owned()),
            None => {
                return (
                    StatusCode::NOT_FOUND,
                    "web remote not built (pnpm build:remote)",
                )
                    .into_response()
            }
        },
    };
    let mime = mime_guess::from_path(&path).first_or_octet_stream();
    let cache = if path.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    (
        [
            (header::CONTENT_TYPE, mime.as_ref().to_owned()),
            (header::CACHE_CONTROL, cache.to_owned()),
            (header::CONTENT_SECURITY_POLICY, CSP.to_owned()),
        ],
        file.data,
    )
        .into_response()
}
