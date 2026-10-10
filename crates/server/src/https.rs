// SPDX-License-Identifier: GPL-3.0-or-later
//! Optional HTTPS on the LAN with a self-signed certificate. Browsers warn about it once; the
//! fingerprint shown in the host UI lets people check they reached the right computer.
//! Plain HTTP keeps running next to it (host windows, devices that cannot accept the warning).

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use midnightsnack_core::now_ms;
use midnightsnack_protocol::DEFAULT_HTTPS_PORT;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::net::TcpListener;

use crate::state::{lock, AppState, Event};

/// Certificates are valid this long (Apple platforms refuse longer-lived server certificates).
const VALIDITY_DAYS: i64 = 820;
/// Renewed when less than this is left.
const RENEW_BEFORE_DAYS: i64 = 30;
const TLS_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// The certificate and key in DER form.
#[derive(Clone)]
pub struct CertBundle {
    pub cert: Vec<u8>,
    pub key: Vec<u8>,
    pub not_after_ms: i64,
}

#[derive(Serialize, Deserialize)]
struct Meta {
    not_after_ms: i64,
}

impl CertBundle {
    /// `AB:CD:…` (SHA-256 of the certificate), as browsers show it.
    pub fn fingerprint(&self) -> String {
        Sha256::digest(&self.cert)
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(":")
    }

    /// A new self-signed certificate for `names` (IP addresses and host names).
    pub fn generate(names: Vec<String>) -> Result<Self, rcgen::Error> {
        let mut params = rcgen::CertificateParams::new(names)?;
        params
            .distinguished_name
            .push(rcgen::DnType::CommonName, "midnightsnack host");
        params
            .distinguished_name
            .push(rcgen::DnType::OrganizationName, "midnightsnack");
        params.extended_key_usages = vec![rcgen::ExtendedKeyUsagePurpose::ServerAuth];
        let now = time::OffsetDateTime::now_utc();
        params.not_before = now - time::Duration::days(1);
        params.not_after = now + time::Duration::days(VALIDITY_DAYS);
        let key = rcgen::KeyPair::generate()?;
        let cert = params.self_signed(&key)?;
        Ok(CertBundle {
            cert: cert.der().to_vec(),
            key: key.serialize_der(),
            not_after_ms: (params.not_after.unix_timestamp()) * 1000,
        })
    }

    fn load(dir: &Path) -> Option<Self> {
        let meta: Meta = crate::util::read_json(&dir.join("meta.json"))?;
        Some(CertBundle {
            cert: std::fs::read(dir.join("cert.der")).ok()?,
            key: std::fs::read(dir.join("key.der")).ok()?,
            not_after_ms: meta.not_after_ms,
        })
    }

    fn save(&self, dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        crate::util::write_private(&dir.join("key.der"), &self.key)?;
        std::fs::write(dir.join("cert.der"), &self.cert)?;
        crate::util::write_json_atomic(
            &dir.join("meta.json"),
            &Meta {
                not_after_ms: self.not_after_ms,
            },
        )
    }

    fn expiring(&self) -> bool {
        self.not_after_ms - now_ms() < RENEW_BEFORE_DAYS * 24 * 3600 * 1000
    }

    fn server_config(&self) -> Result<rustls::ServerConfig, rustls::Error> {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let mut config = rustls::ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()?
            .with_no_client_auth()
            .with_single_cert(
                vec![CertificateDer::from(self.cert.clone())],
                PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(self.key.clone())),
            )?;
        config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
        Ok(config)
    }
}

fn cert_dir(state: &AppState) -> Option<PathBuf> {
    state.data_dir.as_ref().map(|d| d.join("https"))
}

/// The certificate in use, creating or renewing it as needed.
pub fn certificate(state: &AppState) -> Result<CertBundle, rcgen::Error> {
    if let Some(c) = lock(&state.https_cert).clone().filter(|c| !c.expiring()) {
        return Ok(c);
    }
    let dir = cert_dir(state);
    if let Some(c) = dir
        .as_deref()
        .and_then(CertBundle::load)
        .filter(|c| !c.expiring())
    {
        *lock(&state.https_cert) = Some(c.clone());
        return Ok(c);
    }
    let mut names: Vec<String> = lock(&state.interfaces)
        .iter()
        .map(|i| i.address.clone())
        .collect();
    names.extend(["localhost".to_owned(), "127.0.0.1".to_owned()]);
    let local_name = format!(
        "{}.local",
        state
            .host
            .name
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect::<String>()
    );
    names.push(local_name);
    names.dedup();
    let c = CertBundle::generate(names)?;
    if let Some(dir) = dir {
        if let Err(e) = c.save(&dir) {
            tracing::error!(error = %e, "failed to save the HTTPS certificate");
        }
    }
    tracing::info!(fingerprint = %c.fingerprint(), "created an HTTPS certificate");
    *lock(&state.https_cert) = Some(c.clone());
    Ok(c)
}

/// Forgets the certificate so the next start creates a new one.
pub fn renew(state: &AppState) {
    *lock(&state.https_cert) = None;
    if let Some(dir) = cert_dir(state) {
        let _ = std::fs::remove_dir_all(dir);
    }
}

fn set_status(state: &AppState, port: Option<u16>, fingerprint: Option<String>) {
    let changed = {
        let mut s = lock(&state.https_runtime);
        let changed = s.0 != port || s.1 != fingerprint;
        *s = (port, fingerprint);
        changed
    };
    if changed {
        crate::state::refresh_base_urls(state);
        state.emit(Event::Connectivity);
    }
}

/// Serves HTTPS while it is enabled, restarting when settings or the certificate change.
pub async fn run(state: Arc<AppState>, router: Router, bind: std::net::IpAddr, port: u16) {
    loop {
        if !lock(&state.settings).https {
            set_status(&state, None, None);
            state.https_restart.notified().await;
            continue;
        }
        let started = async {
            let cert = certificate(&state).map_err(|e| e.to_string())?;
            let config = cert.server_config().map_err(|e| e.to_string())?;
            let listener = match TcpListener::bind(SocketAddr::new(bind, port)).await {
                Ok(l) => l,
                Err(e) if port != 0 => {
                    tracing::warn!(error = %e, port, "HTTPS port busy, using a free port");
                    TcpListener::bind(SocketAddr::new(bind, 0))
                        .await
                        .map_err(|e| e.to_string())?
                }
                Err(e) => return Err(e.to_string()),
            };
            Ok::<_, String>((listener, config, cert.fingerprint()))
        }
        .await;
        match started {
            Ok((listener, config, fingerprint)) => {
                let port = listener.local_addr().map(|a| a.port()).ok();
                tracing::info!(?port, "HTTPS listening");
                set_status(&state, port, Some(fingerprint));
                tokio::select! {
                    _ = serve(listener, Arc::new(config), router.clone()) => {}
                    _ = state.https_restart.notified() => {}
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "cannot start HTTPS");
                set_status(&state, None, None);
                state.https_restart.notified().await;
            }
        }
    }
}

async fn serve(listener: TcpListener, config: Arc<rustls::ServerConfig>, router: Router) {
    let acceptor = tokio_rustls::TlsAcceptor::from(config);
    let slots = Arc::new(tokio::sync::Semaphore::new(crate::listen::MAX_CONNECTIONS));
    // Connections end with the listener: dropping this set aborts them on restart.
    let mut connections = tokio::task::JoinSet::new();
    loop {
        let permit = slots
            .clone()
            .acquire_owned()
            .await
            .expect("semaphore is never closed");
        let (tcp, addr) = tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok(a) => a,
                Err(e) => {
                    tracing::debug!(error = %e, "HTTPS accept failed");
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    continue;
                }
            },
            Some(_) = connections.join_next(), if !connections.is_empty() => continue,
        };
        let _ = tcp.set_nodelay(true);
        let acceptor = acceptor.clone();
        let router = router.clone();
        connections.spawn(async move {
            if let Ok(Ok(tls)) =
                tokio::time::timeout(TLS_HANDSHAKE_TIMEOUT, acceptor.accept(tcp)).await
            {
                crate::listen::serve_connection(tls, addr, true, router).await;
            }
            drop(permit);
        });
    }
}

/// Port to try first for HTTPS.
pub fn default_port(http_port: u16) -> u16 {
    if http_port == 0 {
        0
    } else {
        DEFAULT_HTTPS_PORT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn certificates_have_a_fingerprint_and_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let c = CertBundle::generate(vec!["192.168.1.5".into(), "localhost".into()]).unwrap();
        assert_eq!(c.fingerprint().len(), 32 * 3 - 1);
        assert!(!c.expiring());
        c.save(dir.path()).unwrap();
        let loaded = CertBundle::load(dir.path()).unwrap();
        assert_eq!(loaded.fingerprint(), c.fingerprint());
        assert!(loaded.server_config().is_ok());
    }
}
