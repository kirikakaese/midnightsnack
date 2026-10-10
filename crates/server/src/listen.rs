// SPDX-License-Identifier: GPL-3.0-or-later
//! Serves HTTP connections (plain or after TLS) with limits that keep slow or idle clients
//! from tying up the host: a header read timeout and a cap on open connections.

use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::ConnectInfo;
use axum::Router;
use hyper_util::rt::{TokioExecutor, TokioIo, TokioTimer};
use hyper_util::service::TowerToHyperService;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpListener;
use tokio::sync::Semaphore;

use crate::ws::Secure;

/// A client must send a request's headers within this time.
pub const HEADER_READ_TIMEOUT: Duration = Duration::from_secs(30);
/// Open connections per listener; more wait until one closes.
pub const MAX_CONNECTIONS: usize = 1024;

/// Serves one connection with the router, marking requests with the peer's address.
pub async fn serve_connection<I>(io: I, addr: SocketAddr, secure: bool, router: Router)
where
    I: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let svc = tower::ServiceExt::map_request(
        router,
        move |mut req: axum::http::Request<hyper::body::Incoming>| {
            req.extensions_mut().insert(ConnectInfo(addr));
            if secure {
                req.extensions_mut().insert(Secure);
            }
            req
        },
    );
    let mut builder = hyper_util::server::conn::auto::Builder::new(TokioExecutor::new());
    builder
        .http1()
        .timer(TokioTimer::new())
        .header_read_timeout(HEADER_READ_TIMEOUT);
    builder.http2().timer(TokioTimer::new());
    if let Err(e) = builder
        .serve_connection_with_upgrades(TokioIo::new(io), TowerToHyperService::new(svc))
        .await
    {
        tracing::debug!(%addr, error = %e, "connection ended");
    }
}

/// Accepts plain HTTP connections until `shutdown` resolves; open connections end with it.
pub async fn serve_plain(
    listener: TcpListener,
    router: Router,
    shutdown: impl Future<Output = ()>,
) {
    let slots = Arc::new(Semaphore::new(MAX_CONNECTIONS));
    let mut connections = tokio::task::JoinSet::new();
    tokio::pin!(shutdown);
    loop {
        let permit = tokio::select! {
            p = slots.clone().acquire_owned() => p.expect("semaphore is never closed"),
            _ = &mut shutdown => return,
        };
        let (tcp, addr) = tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok(a) => a,
                Err(e) => {
                    tracing::debug!(error = %e, "accept failed");
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    continue;
                }
            },
            Some(_) = connections.join_next(), if !connections.is_empty() => continue,
            _ = &mut shutdown => return,
        };
        // Live control sends many tiny messages; never let Nagle's algorithm delay them.
        if let Err(e) = tcp.set_nodelay(true) {
            tracing::debug!(error = %e, "could not set TCP_NODELAY");
        }
        let router = router.clone();
        connections.spawn(async move {
            serve_connection(tcp, addr, false, router).await;
            drop(permit);
        });
    }
}
