// SPDX-License-Identifier: GPL-3.0-or-later
//! A small fake OpenSlides 4 server for tests and the development server. It implements the
//! endpoints the adapter uses — `/system/auth/login`, `/system/auth/who-am-i/` and
//! `/system/autoupdate` (relations, `single=1`, streaming changes) — over an in-memory
//! key-value store that tests can change while clients are subscribed.

use std::collections::{HashMap, HashSet};
use std::convert::Infallible;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::body::{Body, Bytes};
use axum::extract::{Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::Router;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use base64::Engine as _;
use serde_json::{json, Map, Value};
use tokio::sync::broadcast;

struct Inner {
    data: Mutex<HashMap<String, Value>>,
    users: Vec<(String, String, u32)>,
    /// Access token → (user id, expiry in Unix seconds).
    tokens: Mutex<HashMap<String, (u32, u64)>>,
    /// Refresh cookie → user id.
    sessions: Mutex<HashMap<String, u32>>,
    token_ttl: Duration,
    anonymous: bool,
    changes: broadcast::Sender<()>,
    counter: AtomicU64,
    logins: AtomicU64,
}

/// A running mock instance.
#[derive(Clone)]
pub struct MockOpenSlides {
    pub url: String,
    inner: Arc<Inner>,
}

pub struct MockOptions {
    pub data: HashMap<String, Value>,
    /// (username, password, user id).
    pub users: Vec<(String, String, u32)>,
    pub token_ttl: Duration,
    /// Allow autoupdate requests without a token.
    pub anonymous: bool,
}

impl Default for MockOptions {
    fn default() -> Self {
        MockOptions {
            data: demo_meeting(),
            users: vec![("admin".into(), "admin".into(), 1)],
            token_ttl: Duration::from_secs(600),
            anonymous: false,
        }
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl MockOpenSlides {
    /// Starts on `127.0.0.1` with a free port.
    pub async fn start(options: MockOptions) -> std::io::Result<MockOpenSlides> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        Self::serve(listener, options)
    }

    pub fn serve(
        listener: tokio::net::TcpListener,
        options: MockOptions,
    ) -> std::io::Result<MockOpenSlides> {
        let url = format!("http://{}", listener.local_addr()?);
        let (changes, _) = broadcast::channel(64);
        let inner = Arc::new(Inner {
            data: Mutex::new(options.data),
            users: options.users,
            tokens: Mutex::new(HashMap::new()),
            sessions: Mutex::new(HashMap::new()),
            token_ttl: options.token_ttl,
            anonymous: options.anonymous,
            changes,
            counter: AtomicU64::new(1),
            logins: AtomicU64::new(0),
        });
        let app = Router::new()
            .route("/system/auth/login", post(login))
            .route("/system/auth/who-am-i", post(who_am_i))
            .route("/system/auth/who-am-i/", post(who_am_i))
            .route("/system/autoupdate", post(autoupdate))
            // Not part of OpenSlides: lets end-to-end tests change data like an operator would.
            .route("/mock/set", post(mock_set))
            .with_state(inner.clone());
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        Ok(MockOpenSlides { url, inner })
    }

    /// Changes values (`null` removes) and notifies subscribed clients.
    pub fn set(&self, changes: Value) {
        if let Value::Object(map) = changes {
            let mut data = lock(&self.inner.data);
            for (k, v) in map {
                if v.is_null() {
                    data.remove(&k);
                } else {
                    data.insert(k, v);
                }
            }
        }
        let _ = self.inner.changes.send(());
    }

    /// Invalidates all access tokens (as if they expired).
    pub fn expire_tokens(&self) {
        lock(&self.inner.tokens).clear();
    }

    pub fn login_count(&self) -> u64 {
        self.inner.logins.load(Ordering::Relaxed)
    }
}

async fn mock_set(State(inner): State<Arc<Inner>>, body: Bytes) -> StatusCode {
    let Ok(Value::Object(map)) = serde_json::from_slice::<Value>(&body) else {
        return StatusCode::BAD_REQUEST;
    };
    {
        let mut data = lock(&inner.data);
        for (k, v) in map {
            if v.is_null() {
                data.remove(&k);
            } else {
                data.insert(k, v);
            }
        }
    }
    let _ = inner.changes.send(());
    StatusCode::NO_CONTENT
}

fn issue(inner: &Inner, user: u32) -> (String, String) {
    let n = inner.counter.fetch_add(1, Ordering::Relaxed);
    let exp = now_secs() + inner.token_ttl.as_secs();
    let header = B64.encode(br#"{"alg":"HS256","typ":"JWT"}"#);
    let payload = B64.encode(json!({"exp": exp, "userId": user, "sessionId": n}).to_string());
    let token = format!("bearer {header}.{payload}.mock{n}");
    lock(&inner.tokens).insert(token.clone(), (user, exp));
    let cookie = format!("bearer%20refresh{n}");
    lock(&inner.sessions).insert(cookie.clone(), user);
    (token, cookie)
}

fn ticket(token: String, cookie: String) -> Response {
    (
        [
            ("authentication", token),
            (
                header::SET_COOKIE.as_str(),
                format!("refreshId={cookie}; HttpOnly; Path=/system/auth"),
            ),
        ],
        axum::Json(json!({"success": true, "message": "Authentication successful!"})),
    )
        .into_response()
}

async fn login(State(inner): State<Arc<Inner>>, body: Bytes) -> Response {
    let creds: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    let user = creds["username"].as_str().unwrap_or_default();
    let pass = creds["password"].as_str().unwrap_or_default();
    let Some(&(_, _, id)) = inner.users.iter().find(|(u, p, _)| u == user && p == pass) else {
        return (
            StatusCode::FORBIDDEN,
            axum::Json(json!({"success": false, "message": "Username or password is incorrect."})),
        )
            .into_response();
    };
    inner.logins.fetch_add(1, Ordering::Relaxed);
    let (token, cookie) = issue(&inner, id);
    ticket(token, cookie)
}

async fn who_am_i(State(inner): State<Arc<Inner>>, headers: HeaderMap) -> Response {
    let cookie = headers
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|c| {
            c.split(';')
                .find_map(|p| p.trim().strip_prefix("refreshId=").map(str::to_owned))
        });
    let user = cookie.and_then(|c| lock(&inner.sessions).get(&c).copied());
    let Some(user) = user else {
        return (
            StatusCode::FORBIDDEN,
            axum::Json(json!({"success": false, "message": "Not signed in"})),
        )
            .into_response();
    };
    let (token, cookie) = issue(&inner, user);
    ticket(token, cookie)
}

/// Collects the keys a request selects, following relations through the current data.
fn select(data: &HashMap<String, Value>, request: &Value, out: &mut HashSet<String>) {
    let Some(list) = request.as_array() else {
        return;
    };
    for r in list {
        let collection = r["collection"].as_str().unwrap_or_default();
        let ids: Vec<u64> = r["ids"]
            .as_array()
            .map(|a| a.iter().filter_map(Value::as_u64).collect())
            .unwrap_or_default();
        for id in ids {
            select_fields(data, collection, id, &r["fields"], out);
        }
    }
}

fn select_fields(
    data: &HashMap<String, Value>,
    collection: &str,
    id: u64,
    fields: &Value,
    out: &mut HashSet<String>,
) {
    let Some(fields) = fields.as_object() else {
        return;
    };
    for (field, spec) in fields {
        let key = format!("{collection}/{id}/{field}");
        let Some(value) = data.get(&key) else {
            continue;
        };
        out.insert(key);
        let Some(kind) = spec.get("type").and_then(Value::as_str) else {
            continue;
        };
        let sub = &spec["fields"];
        let target = spec["collection"].as_str().unwrap_or_default();
        match kind {
            "relation" => {
                if let Some(rid) = value.as_u64() {
                    select_fields(data, target, rid, sub, out);
                }
            }
            "relation-list" => {
                for rid in value
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_u64)
                {
                    select_fields(data, target, rid, sub, out);
                }
            }
            "generic-relation" => {
                if let Some((c, rid)) = value.as_str().and_then(|s| s.split_once('/')) {
                    if let Ok(rid) = rid.parse() {
                        select_fields(data, c, rid, sub, out);
                    }
                }
            }
            _ => {}
        }
    }
}

fn snapshot(inner: &Inner, request: &Value) -> Map<String, Value> {
    let data = lock(&inner.data);
    let mut keys = HashSet::new();
    select(&data, request, &mut keys);
    keys.into_iter()
        .filter_map(|k| data.get(&k).cloned().map(|v| (k, v)))
        .collect()
}

#[derive(serde::Deserialize)]
struct AutoupdateQuery {
    single: Option<String>,
}

async fn autoupdate(
    State(inner): State<Arc<Inner>>,
    Query(q): Query<AutoupdateQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let token = headers.get("authentication").and_then(|v| v.to_str().ok());
    match token {
        Some(t) => {
            let valid = lock(&inner.tokens)
                .get(t)
                .is_some_and(|&(_, exp)| exp > now_secs());
            if !valid {
                return (
                    StatusCode::UNAUTHORIZED,
                    axum::Json(json!({"error": {"type": "auth", "msg": "invalid token"}})),
                )
                    .into_response();
            }
        }
        None if !inner.anonymous => {
            return (
                StatusCode::UNAUTHORIZED,
                axum::Json(json!({"error": {"type": "auth", "msg": "anonymous is disabled"}})),
            )
                .into_response();
        }
        None => {}
    }
    let Ok(request) = serde_json::from_slice::<Value>(&body) else {
        return (
            StatusCode::BAD_REQUEST,
            axum::Json(json!({"error": {"type": "invalid", "msg": "body is not JSON"}})),
        )
            .into_response();
    };
    let first = snapshot(&inner, &request);
    if q.single.as_deref() == Some("1") {
        return format!("{}\n", Value::Object(first)).into_response();
    }
    let mut changes = inner.changes.subscribe();
    let stream = async_stream(move |tx| async move {
        let mut last = first.clone();
        if tx.send(Value::Object(first).to_string()).await.is_err() {
            return;
        }
        while changes.recv().await.is_ok() {
            let now = snapshot(&inner, &request);
            let mut diff = Map::new();
            for (k, v) in &now {
                if last.get(k) != Some(v) {
                    diff.insert(k.clone(), v.clone());
                }
            }
            for k in last.keys() {
                if !now.contains_key(k) {
                    diff.insert(k.clone(), Value::Null);
                }
            }
            last = now;
            if !diff.is_empty() && tx.send(Value::Object(diff).to_string()).await.is_err() {
                return;
            }
        }
    });
    Response::builder()
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .body(Body::from_stream(stream))
        .expect("valid response")
}

/// Runs `f` producing lines; the body ends when `f` returns.
fn async_stream<F, Fut>(
    f: F,
) -> impl futures_util::Stream<Item = Result<String, Infallible>> + Send + 'static
where
    F: FnOnce(tokio::sync::mpsc::Sender<String>) -> Fut,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let (tx, rx) = tokio::sync::mpsc::channel::<String>(16);
    tokio::spawn(f(tx));
    futures_util::stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|line| (Ok(format!("{line}\n")), rx))
    })
}

/// A demo meeting: agenda with a topic and two motions, lists of speakers, two projectors.
pub fn demo_meeting() -> HashMap<String, Value> {
    let v: Value = serde_json::from_str(
        r#"{
        "user/1/id": 1, "user/1/username": "admin", "user/1/meeting_ids": [1],
        "user/2/id": 2, "user/2/username": "ada", "user/2/first_name": "Ada",
        "user/2/last_name": "Lovelace",
        "user/3/id": 3, "user/3/username": "grace", "user/3/title": "Dr.",
        "user/3/first_name": "Grace", "user/3/last_name": "Hopper",
        "user/4/id": 4, "user/4/username": "alan", "user/4/first_name": "Alan",
        "user/4/last_name": "Turing",
        "meeting_user/12/id": 12, "meeting_user/12/user_id": 2,
        "meeting_user/13/id": 13, "meeting_user/13/user_id": 3,
        "meeting_user/14/id": 14, "meeting_user/14/user_id": 4,
        "meeting/1/id": 1, "meeting/1/name": "Delegates assembly 2026",
        "meeting/1/is_active_in_organization_id": 1,
        "meeting/1/reference_projector_id": 1,
        "meeting/1/agenda_show_internal_items_on_projector": false,
        "meeting/1/agenda_item_ids": [1, 2, 3, 4],
        "meeting/1/motion_ids": [1, 2],
        "meeting/1/topic_ids": [1],
        "meeting/1/assignment_ids": [],
        "meeting/1/motion_block_ids": [],
        "meeting/1/list_of_speakers_ids": [1, 2, 3],
        "meeting/1/projector_ids": [1, 2],
        "agenda_item/1/id": 1, "agenda_item/1/item_number": "TOP 1", "agenda_item/1/type": "common",
        "agenda_item/1/weight": 1, "agenda_item/1/level": 0, "agenda_item/1/closed": true,
        "agenda_item/1/content_object_id": "topic/1",
        "agenda_item/2/id": 2, "agenda_item/2/item_number": "TOP 2", "agenda_item/2/type": "common",
        "agenda_item/2/weight": 2, "agenda_item/2/level": 0, "agenda_item/2/closed": false,
        "agenda_item/2/content_object_id": "motion/1",
        "agenda_item/3/id": 3, "agenda_item/3/item_number": "TOP 2.1", "agenda_item/3/type": "common",
        "agenda_item/3/weight": 1, "agenda_item/3/level": 1, "agenda_item/3/parent_id": 2,
        "agenda_item/3/closed": false, "agenda_item/3/content_object_id": "motion/2",
        "agenda_item/4/id": 4, "agenda_item/4/item_number": "", "agenda_item/4/type": "internal",
        "agenda_item/4/weight": 3, "agenda_item/4/level": 0, "agenda_item/4/closed": false,
        "agenda_item/4/content_object_id": "topic/2",
        "topic/1/id": 1, "topic/1/title": "Welcome and opening",
        "topic/1/text": "<p>Welcome to the <strong>delegates assembly</strong>.</p>",
        "topic/1/list_of_speakers_id": 1,
        "topic/2/id": 2, "topic/2/title": "Internal: catering",
        "motion/1/id": 1, "motion/1/number": "A1", "motion/1/sequential_number": 1,
        "motion/1/title": "Climate budget",
        "motion/1/text": "<p>The assembly decides:</p><ol><li>A climate budget of 2% of all spending.</li><li>Yearly reports to the assembly.</li></ol>",
        "motion/1/reason": "<p>We need to act now.</p>",
        "motion/1/state_id": 5, "motion/1/submitter_ids": [21, 22],
        "motion/1/list_of_speakers_id": 2,
        "motion/2/id": 2, "motion/2/number": "A2", "motion/2/sequential_number": 2,
        "motion/2/title": "Amendment: 3% instead of 2%",
        "motion/2/text": "<p>Replace 2% by 3%.</p>", "motion/2/state_id": 6,
        "motion/2/submitter_ids": [23], "motion/2/list_of_speakers_id": 3,
        "motion_state/5/id": 5, "motion_state/5/name": "submitted",
        "motion_state/6/id": 6, "motion_state/6/name": "permitted",
        "motion_submitter/21/id": 21, "motion_submitter/21/weight": 1,
        "motion_submitter/21/meeting_user_id": 12,
        "motion_submitter/22/id": 22, "motion_submitter/22/weight": 2,
        "motion_submitter/22/meeting_user_id": 13,
        "motion_submitter/23/id": 23, "motion_submitter/23/weight": 1,
        "motion_submitter/23/meeting_user_id": 14,
        "list_of_speakers/1/id": 1, "list_of_speakers/1/closed": true,
        "list_of_speakers/1/content_object_id": "topic/1", "list_of_speakers/1/speaker_ids": [],
        "list_of_speakers/2/id": 2, "list_of_speakers/2/closed": false,
        "list_of_speakers/2/content_object_id": "motion/1",
        "list_of_speakers/2/speaker_ids": [31, 32, 33],
        "list_of_speakers/3/id": 3, "list_of_speakers/3/closed": false,
        "list_of_speakers/3/content_object_id": "motion/2", "list_of_speakers/3/speaker_ids": [],
        "speaker/31/id": 31, "speaker/31/meeting_user_id": 12, "speaker/31/weight": 1,
        "speaker/31/begin_time": 1760000000, "speaker/31/end_time": 1760000120,
        "speaker/32/id": 32, "speaker/32/meeting_user_id": 13, "speaker/32/weight": 2,
        "speaker/32/begin_time": 1760000130, "speaker/32/speech_state": "pro",
        "speaker/33/id": 33, "speaker/33/meeting_user_id": 14, "speaker/33/weight": 3,
        "projector/1/id": 1, "projector/1/name": "Main projector", "projector/1/sequential_number": 1,
        "projector/1/current_projection_ids": [41, 42],
        "projector/2/id": 2, "projector/2/name": "Side screen", "projector/2/sequential_number": 2,
        "projector/2/current_projection_ids": [43],
        "projection/41/id": 41, "projection/41/content_object_id": "motion/1",
        "projection/41/type": null, "projection/41/stable": false, "projection/41/weight": 1,
        "projection/42/id": 42, "projection/42/content_object_id": "meeting/1",
        "projection/42/type": "clock", "projection/42/stable": true, "projection/42/weight": 0,
        "projection/43/id": 43, "projection/43/content_object_id": "meeting/1",
        "projection/43/type": "agenda_item_list", "projection/43/stable": false,
        "projection/43/weight": 1
    }"#,
    )
    .expect("valid demo data");
    v.as_object()
        .expect("object")
        .iter()
        .filter(|(_, v)| !v.is_null())
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}
