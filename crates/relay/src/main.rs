// SPDX-License-Identifier: GPL-3.0-or-later
//! `deck-relay` — see `docs/user/relay.md`.
//!
//! Options (each also as an environment variable):
//! - `--listen ADDR` (`MIDNIGHTSNACK_RELAY_LISTEN`, default `0.0.0.0:8080`)
//! - `--access-token TOKEN` (`MIDNIGHTSNACK_RELAY_ACCESS_TOKEN`): hosts must present it
//! - `--max-hosts N` (`MIDNIGHTSNACK_RELAY_MAX_HOSTS`, default 200)
//! - `--max-remotes-per-host N` (`MIDNIGHTSNACK_RELAY_MAX_REMOTES`, default 64)
//! - `--max-queued-mb N` (`MIDNIGHTSNACK_RELAY_MAX_QUEUED_MB`, default 256): memory for
//!   messages waiting for remotes, across all hosts
//! - `--healthcheck`: checks a running relay on the listen port and exits (for containers)

use std::net::SocketAddr;
use std::process::ExitCode;

use midnightsnack_relay::{serve, Relay, RelayConfig, VERSION};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

fn usage() -> ExitCode {
    eprintln!(
        "usage: deck-relay [--listen ADDR] [--access-token TOKEN] [--max-hosts N] \
         [--max-remotes-per-host N] [--max-queued-mb N] [--healthcheck] [--version]"
    );
    ExitCode::from(2)
}

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let mut listen = env("MIDNIGHTSNACK_RELAY_LISTEN").unwrap_or_else(|| "0.0.0.0:8080".into());
    let mut config = RelayConfig {
        access_token: env("MIDNIGHTSNACK_RELAY_ACCESS_TOKEN"),
        ..RelayConfig::default()
    };
    if let Some(n) = env("MIDNIGHTSNACK_RELAY_MAX_HOSTS").and_then(|v| v.parse().ok()) {
        config.max_hosts = n;
    }
    if let Some(n) = env("MIDNIGHTSNACK_RELAY_MAX_REMOTES").and_then(|v| v.parse().ok()) {
        config.max_remotes_per_host = n;
    }
    if let Some(n) = env("MIDNIGHTSNACK_RELAY_MAX_QUEUED_MB").and_then(|v| v.parse::<usize>().ok())
    {
        config.max_queued_bytes = n * 1024 * 1024;
    }
    let mut healthcheck = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut value = || args.next();
        match a.as_str() {
            "--listen" => match value() {
                Some(v) => listen = v,
                None => return usage(),
            },
            "--access-token" => config.access_token = value().filter(|v| !v.is_empty()),
            "--max-hosts" => match value().and_then(|v| v.parse().ok()) {
                Some(n) => config.max_hosts = n,
                None => return usage(),
            },
            "--max-remotes-per-host" => match value().and_then(|v| v.parse().ok()) {
                Some(n) => config.max_remotes_per_host = n,
                None => return usage(),
            },
            "--max-queued-mb" => match value().and_then(|v| v.parse::<usize>().ok()) {
                Some(n) => config.max_queued_bytes = n * 1024 * 1024,
                None => return usage(),
            },
            "--healthcheck" => healthcheck = true,
            "--version" => {
                println!("deck-relay {VERSION}");
                return ExitCode::SUCCESS;
            }
            _ => return usage(),
        }
    }
    let Ok(addr) = listen.parse::<SocketAddr>() else {
        eprintln!("invalid listen address {listen}");
        return ExitCode::from(2);
    };
    if healthcheck {
        return check(addr).await;
    }

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("cannot listen on {addr}: {e}");
            return ExitCode::FAILURE;
        }
    };
    tracing::info!(
        %addr,
        version = VERSION,
        access_token = config.access_token.is_some(),
        "relay listening"
    );
    let relay = Relay::new(config);
    tokio::select! {
        r = serve(listener, relay) => {
            if let Err(e) = r {
                tracing::error!(error = %e, "relay stopped");
                return ExitCode::FAILURE;
            }
        }
        _ = shutdown() => tracing::info!("shutting down"),
    }
    ExitCode::SUCCESS
}

async fn shutdown() {
    #[cfg(unix)]
    {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("signal handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = term.recv() => {}
        }
    }
    #[cfg(not(unix))]
    let _ = tokio::signal::ctrl_c().await;
}

/// `GET /healthz` on the local listen port.
async fn check(addr: SocketAddr) -> ExitCode {
    let target = if addr.ip().is_unspecified() {
        SocketAddr::from(([127, 0, 0, 1], addr.port()))
    } else {
        addr
    };
    let attempt = async {
        let mut s = tokio::net::TcpStream::connect(target).await.ok()?;
        s.write_all(b"GET /healthz HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .await
            .ok()?;
        let mut buf = Vec::new();
        s.read_to_end(&mut buf).await.ok()?;
        Some(buf.starts_with(b"HTTP/1.1 200"))
    };
    match tokio::time::timeout(std::time::Duration::from_secs(5), attempt).await {
        Ok(Some(true)) => ExitCode::SUCCESS,
        _ => ExitCode::FAILURE,
    }
}
