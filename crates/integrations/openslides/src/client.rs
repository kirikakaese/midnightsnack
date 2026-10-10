// SPDX-License-Identifier: GPL-3.0-or-later
//! Talking to OpenSlides 4: login and token refresh (auth service), one-off and streaming
//! autoupdate requests, and the long-running connection that keeps a meeting's data current.

use std::pin::Pin;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use base64::Engine as _;
use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use midnightsnack_protocol::{OpenSlidesError, OpenSlidesState, OsMeetingData, OsMeetingRef};
use reqwest::header::{HeaderMap, HeaderValue, COOKIE, SET_COOKIE};
use reqwest::StatusCode;
use serde_json::{Map, Value};
use tokio::sync::mpsc;

use crate::store::Store;
use crate::{request, view};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Refresh the access token when less than this is left.
const REFRESH_MARGIN: Duration = Duration::from_secs(45);
/// How often meeting data is recomputed and sent while updates arrive.
const DATA_THROTTLE: Duration = Duration::from_millis(150);
const RETRY_MIN: Duration = Duration::from_secs(2);
const RETRY_MAX: Duration = Duration::from_secs(60);
/// Without a meeting selected, how often the meeting list is refreshed.
const MEETINGS_REFRESH: Duration = Duration::from_secs(60);
/// Longest line of the autoupdate stream accepted.
const MAX_LINE: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Base URL of the OpenSlides instance, e.g. `https://openslides.example.org`.
    pub url: String,
    /// Empty for public (anonymous) access.
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("invalid URL")]
    InvalidUrl,
    #[error("unreachable: {0}")]
    Unreachable(String),
    #[error("unauthorized")]
    Unauthorized,
    #[error("meeting not found")]
    MeetingNotFound,
    #[error("unexpected answer: {0}")]
    Incompatible(String),
}

impl Error {
    pub fn code(&self) -> OpenSlidesError {
        match self {
            Error::InvalidUrl => OpenSlidesError::InvalidUrl,
            Error::Unreachable(_) => OpenSlidesError::Unreachable,
            Error::Unauthorized => OpenSlidesError::Unauthorized,
            Error::MeetingNotFound => OpenSlidesError::MeetingNotFound,
            Error::Incompatible(_) => OpenSlidesError::Incompatible,
        }
    }
}

fn unreachable(e: reqwest::Error) -> Error {
    Error::Unreachable(e.to_string())
}

/// The instance's base URL without a trailing slash.
pub fn normalize_url(url: &str) -> Result<String, Error> {
    let url = url.trim().trim_end_matches('/');
    let parsed = reqwest::Url::parse(url).map_err(|_| Error::InvalidUrl)?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none_or(str::is_empty)
        || parsed.query().is_some()
    {
        return Err(Error::InvalidUrl);
    }
    Ok(url.to_owned())
}

/// Reads `exp` and `userId` from a JWT without checking its signature (the server checks it;
/// the client only needs to know when to refresh).
fn token_claims(token: &str) -> Option<(u64, u32)> {
    let jwt = token
        .strip_prefix("bearer ")
        .or_else(|| token.strip_prefix("Bearer "))
        .unwrap_or(token);
    let payload = jwt.split('.').nth(1)?;
    let bytes = B64.decode(payload.trim_end_matches('=')).ok()?;
    let v: Value = serde_json::from_slice(&bytes).ok()?;
    let exp = v.get("exp")?.as_u64()?;
    let user = v.get("userId").and_then(Value::as_u64).unwrap_or(0);
    Some((exp, u32::try_from(user).ok()?))
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// `refreshId=…` from the response's cookies.
fn refresh_cookie(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find_map(|c| {
            let pair = c.split(';').next()?.trim();
            pair.starts_with("refreshId=").then(|| pair.to_owned())
        })
}

/// A logged-in (or anonymous) session.
pub struct Session {
    http: reqwest::Client,
    base: String,
    config: Config,
    /// Value of the `authentication` header (`bearer …`).
    token: Option<String>,
    cookie: Option<String>,
    expires: u64,
    user_id: u32,
}

impl Session {
    /// Logs in, or opens an anonymous session when no username is configured.
    pub async fn open(config: &Config) -> Result<Session, Error> {
        let base = normalize_url(&config.url)?;
        let http = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            // Credentials go to the configured server only, never to where it redirects.
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(concat!("midnightsnack/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(unreachable)?;
        let mut s = Session {
            http,
            base,
            config: config.clone(),
            token: None,
            cookie: None,
            expires: 0,
            user_id: 0,
        };
        if !config.username.is_empty() {
            s.login().await?;
        }
        Ok(s)
    }

    pub fn user_id(&self) -> u32 {
        self.user_id
    }

    pub fn anonymous(&self) -> bool {
        self.token.is_none()
    }

    /// Time left on the access token (`None` for anonymous sessions).
    pub fn valid_for(&self) -> Option<Duration> {
        self.token
            .as_ref()
            .map(|_| Duration::from_secs(self.expires.saturating_sub(now_secs())))
    }

    fn take_token(&mut self, headers: &HeaderMap) -> Result<(), Error> {
        let token = headers
            .get("authentication")
            .and_then(|v| v.to_str().ok())
            .filter(|t| !t.is_empty())
            .ok_or_else(|| Error::Incompatible("no access token".into()))?
            .to_owned();
        let (exp, user) =
            token_claims(&token).ok_or_else(|| Error::Incompatible("unreadable token".into()))?;
        if let Some(c) = refresh_cookie(headers) {
            self.cookie = Some(c);
        }
        self.token = Some(token);
        self.expires = exp;
        self.user_id = user;
        Ok(())
    }

    async fn login(&mut self) -> Result<(), Error> {
        let res = self
            .http
            .post(format!("{}/system/auth/login", self.base))
            .timeout(REQUEST_TIMEOUT)
            .json(&serde_json::json!({
                "username": self.config.username,
                "password": self.config.password,
            }))
            .send()
            .await
            .map_err(unreachable)?;
        match res.status() {
            s if s.is_success() => {}
            StatusCode::FORBIDDEN | StatusCode::UNAUTHORIZED | StatusCode::BAD_REQUEST => {
                return Err(Error::Unauthorized)
            }
            StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED => {
                return Err(Error::Incompatible("not an OpenSlides 4 server".into()))
            }
            s => return Err(Error::Unreachable(format!("login answered {s}"))),
        }
        self.take_token(res.headers())
    }

    /// Gets a new access token with the refresh cookie, or logs in again.
    pub async fn refresh(&mut self) -> Result<(), Error> {
        if self.config.username.is_empty() {
            return Ok(());
        }
        if let Some(cookie) = self.cookie.clone() {
            let res = self
                .http
                .post(format!("{}/system/auth/who-am-i/", self.base))
                .timeout(REQUEST_TIMEOUT)
                .header(COOKIE, cookie)
                .send()
                .await
                .map_err(unreachable)?;
            if res.status().is_success() && self.take_token(res.headers()).is_ok() {
                return Ok(());
            }
            tracing::debug!(status = %res.status(), "OpenSlides refresh failed, logging in again");
        }
        self.login().await
    }

    /// Makes sure the token is valid for at least `REFRESH_MARGIN`.
    pub async fn ensure_fresh(&mut self) -> Result<(), Error> {
        match self.valid_for() {
            Some(left) if left < REFRESH_MARGIN => self.refresh().await,
            _ => Ok(()),
        }
    }

    fn autoupdate(&self, single: bool, body: &Value) -> reqwest::RequestBuilder {
        let url = format!(
            "{}/system/autoupdate{}",
            self.base,
            if single { "?single=1" } else { "" }
        );
        let mut req = self.http.post(url).json(body);
        if let Some(t) = &self.token {
            if let Ok(v) = HeaderValue::from_str(t) {
                req = req.header("authentication", v);
            }
        }
        if single {
            req = req.timeout(REQUEST_TIMEOUT);
        }
        req
    }

    async fn check(res: reqwest::Response) -> Result<reqwest::Response, Error> {
        match res.status() {
            s if s.is_success() => Ok(res),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(Error::Unauthorized),
            s => {
                let body = read_limited(res, 4096).await.unwrap_or_default();
                Err(Error::Incompatible(format!(
                    "autoupdate answered {s}: {}",
                    String::from_utf8_lossy(&body)
                        .chars()
                        .take(200)
                        .collect::<String>()
                )))
            }
        }
    }

    /// One autoupdate answer (`?single=1`).
    pub async fn fetch(&mut self, body: &Value) -> Result<Map<String, Value>, Error> {
        self.ensure_fresh().await?;
        let res = self
            .autoupdate(true, body)
            .send()
            .await
            .map_err(unreachable)?;
        let res = Self::check(res).await?;
        let bytes = read_limited(res, MAX_LINE).await?;
        let line = bytes
            .split(|&b| b == b'\n')
            .find(|l| !l.is_empty())
            .unwrap_or_default();
        parse_line(line)
    }

    /// The autoupdate stream: the full data first, then changes.
    pub async fn subscribe(&mut self, body: &Value) -> Result<Updates, Error> {
        self.ensure_fresh().await?;
        let res = self
            .autoupdate(false, body)
            .send()
            .await
            .map_err(unreachable)?;
        let res = Self::check(res).await?;
        Ok(lines(res.bytes_stream()))
    }
}

fn parse_line(line: &[u8]) -> Result<Map<String, Value>, Error> {
    let v: Value =
        serde_json::from_slice(line).map_err(|e| Error::Incompatible(format!("not JSON: {e}")))?;
    let Value::Object(map) = v else {
        return Err(Error::Incompatible("not a JSON object".into()));
    };
    // Errors arrive in the stream as `{"error": {"type": …, "msg": …}}`.
    if let Some(Value::Object(err)) = map.get("error") {
        let kind = err.get("type").and_then(Value::as_str).unwrap_or_default();
        let msg = err.get("msg").and_then(Value::as_str).unwrap_or_default();
        return Err(if kind.contains("auth") || kind.contains("permission") {
            Error::Unauthorized
        } else {
            Error::Incompatible(format!("{kind}: {msg}"))
        });
    }
    Ok(map)
}

pub type Updates = Pin<Box<dyn Stream<Item = Result<Map<String, Value>, Error>> + Send>>;

/// Splits a byte stream into newline-terminated JSON messages.
fn lines(body: impl Stream<Item = Result<Bytes, reqwest::Error>> + Send + 'static) -> Updates {
    struct State<S> {
        body: Pin<Box<S>>,
        buf: Vec<u8>,
        done: bool,
    }
    let state = State {
        body: Box::pin(body),
        buf: Vec::new(),
        done: false,
    };
    Box::pin(futures_util::stream::unfold(state, |mut st| async move {
        loop {
            if let Some(i) = st.buf.iter().position(|&b| b == b'\n') {
                let line: Vec<u8> = st.buf.drain(..=i).collect();
                if line.len() <= 1 {
                    continue;
                }
                let item = parse_line(&line[..line.len() - 1]);
                return Some((item, st));
            }
            if st.done {
                if st.buf.iter().all(u8::is_ascii_whitespace) {
                    return None;
                }
                let line = std::mem::take(&mut st.buf);
                return Some((parse_line(&line), st));
            }
            match st.body.next().await {
                Some(Ok(chunk)) => {
                    st.buf.extend_from_slice(&chunk);
                    if st.buf.len() > MAX_LINE {
                        st.done = true;
                        st.buf.clear();
                        return Some((Err(Error::Incompatible("message too large".into())), st));
                    }
                }
                Some(Err(e)) => {
                    st.done = true;
                    st.buf.clear();
                    return Some((Err(unreachable(e)), st));
                }
                None => st.done = true,
            }
        }
    }))
}

/// What the long-running connection reports.
#[derive(Debug, Clone, PartialEq)]
pub enum Update {
    State(OpenSlidesState, Option<OpenSlidesError>),
    Meetings(Vec<OsMeetingRef>),
    /// `None` when the meeting is not (or no longer) available.
    Data(Option<OsMeetingData>),
}

/// Keeps a meeting's data current until `tx` is closed: logs in, lists the user's meetings,
/// subscribes to the meeting, refreshes the token before it expires, and retries with backoff.
/// Clients keep the last data on transient failures.
pub async fn run(config: Config, meeting: Option<u32>, tx: mpsc::Sender<Update>) {
    let mut backoff = RETRY_MIN;
    // Kept across stream renewals so the refresh cookie is used instead of the password.
    let mut session: Option<Session> = None;
    loop {
        if tx.is_closed() {
            return;
        }
        if session.is_none() {
            let _ = tx
                .send(Update::State(OpenSlidesState::Connecting, None))
                .await;
        }
        let result = connect_and_follow(&config, &mut session, meeting, &tx).await;
        let error = match result {
            Ok(()) => {
                backoff = RETRY_MIN;
                continue;
            }
            Err(e) => {
                session = None;
                e
            }
        };
        tracing::warn!(url = %config.url, error = %error, "OpenSlides connection failed");
        if error == Error::MeetingNotFound {
            let _ = tx.send(Update::Data(None)).await;
        }
        let _ = tx
            .send(Update::State(OpenSlidesState::Failed, Some(error.code())))
            .await;
        tokio::select! {
            _ = tokio::time::sleep(backoff) => {}
            _ = tx.closed() => return,
        }
        backoff = (backoff * 2).min(RETRY_MAX);
    }
}

/// Returns `Ok` when the stream ended normally (token renewal), to reconnect at once with the
/// same session.
async fn connect_and_follow(
    config: &Config,
    slot: &mut Option<Session>,
    meeting: Option<u32>,
    tx: &mpsc::Sender<Update>,
) -> Result<(), Error> {
    let fresh = slot.is_none();
    if fresh {
        *slot = Some(Session::open(config).await?);
    }
    let session = slot.as_mut().expect("session just opened");
    // The meeting list is fetched on login, and again while waiting for a meeting choice.
    if (fresh || meeting.is_none()) && !session.anonymous() {
        let raw = session.fetch(&request::meetings(session.user_id())).await?;
        let mut store = Store::new();
        store.apply(raw);
        let _ = tx
            .send(Update::Meetings(view::meetings(&store, session.user_id())))
            .await;
    }
    let Some(meeting) = meeting else {
        let _ = tx
            .send(Update::State(OpenSlidesState::SelectMeeting, None))
            .await;
        tokio::select! {
            _ = tokio::time::sleep(MEETINGS_REFRESH) => return Ok(()),
            _ = tx.closed() => return Ok(()),
        }
    };

    let mut updates = session.subscribe(&request::meeting(meeting)).await?;
    // Restart the stream with a fresh token shortly before the current one expires.
    let renew = session.valid_for().map(|left| {
        left.saturating_sub(REFRESH_MARGIN)
            .max(Duration::from_secs(5))
    });
    let renew_at = renew.map(|d| tokio::time::Instant::now() + d);
    let mut store = Store::new();
    let mut dirty = false;
    let mut first = true;
    let mut tick = tokio::time::interval(DATA_THROTTLE);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            item = updates.next() => {
                let Some(item) = item else {
                    return Err(Error::Unreachable("autoupdate stream ended".into()));
                };
                if store.apply(item?) {
                    dirty = true;
                }
                if first {
                    first = false;
                    let data = view::meeting(&store, meeting).ok_or(Error::MeetingNotFound)?;
                    let _ = tx.send(Update::Data(Some(data))).await;
                    let _ = tx.send(Update::State(OpenSlidesState::Connected, None)).await;
                    dirty = false;
                }
            }
            _ = tick.tick(), if dirty => {
                dirty = false;
                let data = view::meeting(&store, meeting).ok_or(Error::MeetingNotFound)?;
                let _ = tx.send(Update::Data(Some(data))).await;
            }
            _ = sleep_until(renew_at) => {
                tracing::debug!("renewing the OpenSlides stream before the token expires");
                return Ok(());
            }
            _ = tx.closed() => return Ok(()),
        }
    }
}

async fn sleep_until(at: Option<tokio::time::Instant>) {
    match at {
        Some(at) => tokio::time::sleep_until(at).await,
        None => std::future::pending().await,
    }
}

/// Reads a response body up to `max` bytes; a larger one is an error, not unbounded memory.
async fn read_limited(res: reqwest::Response, max: usize) -> Result<Vec<u8>, Error> {
    let mut out = Vec::new();
    let mut body = res.bytes_stream();
    while let Some(chunk) = body.next().await {
        let chunk = chunk.map_err(unreachable)?;
        if out.len() + chunk.len() > max {
            return Err(Error::Incompatible("answer too large".into()));
        }
        out.extend_from_slice(&chunk);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_are_validated() {
        assert_eq!(
            normalize_url(" https://os.example.org/ ").unwrap(),
            "https://os.example.org"
        );
        for bad in ["", "os.example.org", "ftp://x", "https://x/?a=1"] {
            assert_eq!(normalize_url(bad), Err(Error::InvalidUrl), "{bad}");
        }
    }

    #[test]
    fn token_claims_are_read() {
        let payload = B64.encode(br#"{"exp":1700000000,"userId":7,"sessionId":"x"}"#);
        let token = format!("bearer aaa.{payload}.sig");
        assert_eq!(token_claims(&token), Some((1_700_000_000, 7)));
        assert_eq!(token_claims("bearer garbage"), None);
    }

    #[test]
    fn refresh_cookie_is_found() {
        let mut h = HeaderMap::new();
        h.append(SET_COOKIE, HeaderValue::from_static("other=1; Path=/"));
        h.append(
            SET_COOKIE,
            HeaderValue::from_static("refreshId=bearer%20abc; HttpOnly; Path=/system/auth"),
        );
        assert_eq!(
            refresh_cookie(&h).as_deref(),
            Some("refreshId=bearer%20abc")
        );
    }

    #[test]
    fn stream_errors_are_recognized() {
        assert_eq!(
            parse_line(br#"{"error":{"type":"auth","msg":"token expired"}}"#),
            Err(Error::Unauthorized)
        );
        assert!(parse_line(b"[1]").is_err());
        assert_eq!(parse_line(br#"{"motion/1/id":1}"#).unwrap().len(), 1);
    }
}
