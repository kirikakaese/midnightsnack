// SPDX-License-Identifier: GPL-3.0-or-later
//! HTTPS with the generated certificate: the fingerprint the host shows is the certificate the
//! browser gets, the API and WebSocket work over TLS, and the certificate survives restarts.

use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use midnightsnack_core::Origin;
use midnightsnack_protocol::*;
use midnightsnack_server::{start, state::lock, ServerConfig, ServerHandle};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_tungstenite::tungstenite::Message;

/// Accepts exactly the certificate with the expected fingerprint (what a person does when they
/// compare it on the browser's warning page).
#[derive(Debug)]
struct Pinned {
    fingerprint: String,
    provider: Arc<rustls::crypto::CryptoProvider>,
}

impl ServerCertVerifier for Pinned {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _: &[CertificateDer<'_>],
        _: &ServerName<'_>,
        _: &[u8],
        _: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let got = Sha256::digest(end_entity.as_ref())
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(":");
        if got == self.fingerprint {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::General("fingerprint mismatch".into()))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

async fn host(data: &std::path::Path) -> ServerHandle {
    let mut config = ServerConfig::new(data.join("cache"));
    config.bind = "127.0.0.1:0".parse().unwrap();
    config.mdns = false;
    config.data_dir = Some(data.join("data"));
    config.restore_autosave = false;
    let handle = start(config).await.unwrap();
    handle
        .state
        .perform(Origin::LOCAL_ADMIN, Action::SetHttps { on: true })
        .await
        .unwrap();
    for _ in 0..100 {
        if lock(&handle.state.https_runtime).0.is_some() {
            return handle;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("HTTPS did not start");
}

type Tls = tokio_rustls::client::TlsStream<tokio::net::TcpStream>;

async fn tls(port: u16, fingerprint: &str) -> Result<Tls, std::io::Error> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()
        .unwrap()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(Pinned {
            fingerprint: fingerprint.into(),
            provider,
        }))
        .with_no_client_auth();
    let tcp = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .unwrap();
    tokio_rustls::TlsConnector::from(Arc::new(config))
        .connect(ServerName::try_from("127.0.0.1").unwrap(), tcp)
        .await
}

#[tokio::test]
async fn https_serves_the_api_and_websocket_with_the_shown_certificate() {
    let dir = tempfile::tempdir().unwrap();
    let handle = host(dir.path()).await;
    let https = handle.state.connectivity().https;
    assert!(https.enabled);
    let port = https.port.unwrap();
    let fingerprint = https.fingerprint.unwrap();

    let links = handle.state.pairing_info().links;
    assert!(links.iter().any(|l| l.kind == JoinKind::Https
        && l.url
            .starts_with(&format!("https://127.0.0.1:{port}/join#t="))));

    let mut s = tls(port, &fingerprint).await.unwrap();
    s.write_all(b"GET /api/v1/info HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    let mut response = Vec::new();
    s.read_to_end(&mut response).await.unwrap();
    let response = String::from_utf8_lossy(&response);
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    assert!(response.contains("protocol_version"));

    let s = tls(port, &fingerprint).await.unwrap();
    let url = format!("wss://127.0.0.1:{port}/api/v1/ws");
    let (mut ws, _) = tokio_tungstenite::client_async(url, s).await.unwrap();
    let hello = ClientMessage::Hello {
        protocol_version: PROTOCOL_VERSION,
        token: handle.stage_token.clone(),
    };
    ws.send(Message::Text(serde_json::to_string(&hello).unwrap().into()))
        .await
        .unwrap();
    let first = ws.next().await.unwrap().unwrap();
    let msg: ServerMessage = serde_json::from_str(first.to_text().unwrap()).unwrap();
    assert!(matches!(msg, ServerMessage::Welcome { .. }), "{msg:?}");

    // Another certificate is refused by the pin.
    assert!(tls(port, "00:11").await.is_err());
}

#[tokio::test]
async fn the_certificate_survives_restarts_until_renewed() {
    let dir = tempfile::tempdir().unwrap();
    let first = {
        let handle = host(dir.path()).await;
        handle.state.connectivity().https.fingerprint.unwrap()
    };
    tokio::time::sleep(Duration::from_millis(100)).await;
    let handle = host(dir.path()).await;
    assert_eq!(
        handle.state.connectivity().https.fingerprint.unwrap(),
        first,
        "same certificate after a restart"
    );
    handle
        .state
        .perform(Origin::LOCAL_ADMIN, Action::RenewCertificate)
        .await
        .unwrap();
    let mut renewed = None;
    for _ in 0..100 {
        let f = handle.state.connectivity().https.fingerprint;
        if f.as_ref().is_some_and(|f| *f != first) {
            renewed = f;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(renewed.is_some(), "a new certificate after renewing");

    handle
        .state
        .perform(Origin::LOCAL_ADMIN, Action::SetHttps { on: false })
        .await
        .unwrap();
    for _ in 0..100 {
        if handle.state.connectivity().https.port.is_none() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("HTTPS did not stop");
}
