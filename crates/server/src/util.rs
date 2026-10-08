// SPDX-License-Identifier: GPL-3.0-or-later
use std::path::Path;

use rand::RngCore;
use sha2::{Digest, Sha256};

/// 256-bit random token, hex-encoded.
pub fn random_token() -> String {
    let mut b = [0u8; 32];
    rand::rng().fill_bytes(&mut b);
    hex(&b)
}

/// Short random identifier (128 bit), hex-encoded.
pub fn random_id() -> String {
    let mut b = [0u8; 16];
    rand::rng().fill_bytes(&mut b);
    hex(&b)
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn sha256_hex(s: &str) -> String {
    hex(&Sha256::digest(s.as_bytes()))
}

/// Writes JSON atomically (temp file + rename).
pub fn write_json_atomic<T: serde::Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(value)?)?;
    std::fs::rename(tmp, path)
}

pub fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    let bytes = std::fs::read(path).ok()?;
    match serde_json::from_slice(&bytes) {
        Ok(v) => Some(v),
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "ignoring unreadable file");
            None
        }
    }
}
