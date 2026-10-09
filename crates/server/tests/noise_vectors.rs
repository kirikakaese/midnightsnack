// SPDX-License-Identifier: GPL-3.0-or-later
//! Fixed Noise NK vectors produced with `snow`, shared with the web remote's own Noise
//! implementation (`packages/ui/src/client/relay/noise.test.ts`). Run with
//! `UPDATE_NOISE_VECTORS=1` to regenerate (only needed if the pattern or prologue changes).

use std::path::PathBuf;

use midnightsnack_protocol::{RELAY_NOISE_PATTERN, RELAY_PROLOGUE};
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Vectors {
    pattern: String,
    prologue: String,
    responder_static_private: String,
    responder_static_public: String,
    initiator_ephemeral: String,
    responder_ephemeral: String,
    /// `e, es` with an empty payload.
    message1: String,
    /// `e, ee` with an empty payload.
    message2: String,
    /// (sender, plaintext hex, ciphertext hex) in order.
    transport: Vec<(String, String, String)>,
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn compute(
    static_private: &[u8],
    static_public: &[u8],
    ie: &[u8],
    re: &[u8],
    prologue: &str,
) -> Vectors {
    let params = || RELAY_NOISE_PATTERN.parse().unwrap();
    let mut i = snow::Builder::new(params())
        .remote_public_key(static_public)
        .unwrap()
        .prologue(prologue.as_bytes())
        .unwrap()
        .fixed_ephemeral_key_for_testing_only(ie)
        .build_initiator()
        .unwrap();
    let mut r = snow::Builder::new(params())
        .local_private_key(static_private)
        .unwrap()
        .prologue(prologue.as_bytes())
        .unwrap()
        .fixed_ephemeral_key_for_testing_only(re)
        .build_responder()
        .unwrap();
    let mut buf = vec![0u8; 65535];
    let mut tmp = vec![0u8; 65535];
    let n = i.write_message(&[], &mut buf).unwrap();
    let m1 = buf[..n].to_vec();
    r.read_message(&m1, &mut tmp).unwrap();
    let n = r.write_message(&[], &mut buf).unwrap();
    let m2 = buf[..n].to_vec();
    i.read_message(&m2, &mut tmp).unwrap();
    let mut i = i.into_transport_mode().unwrap();
    let mut r = r.into_transport_mode().unwrap();
    let mut transport = Vec::new();
    let msgs: [(&str, &[u8]); 4] = [
        ("initiator", b"hello host"),
        ("responder", b"hello remote"),
        ("initiator", b""),
        ("initiator", &[7u8; 300]),
    ];
    for (who, plain) in msgs {
        let n = if who == "initiator" {
            i.write_message(plain, &mut buf).unwrap()
        } else {
            r.write_message(plain, &mut buf).unwrap()
        };
        let c = buf[..n].to_vec();
        let back = if who == "initiator" {
            r.read_message(&c, &mut tmp).unwrap()
        } else {
            i.read_message(&c, &mut tmp).unwrap()
        };
        assert_eq!(&tmp[..back], plain);
        transport.push((who.to_owned(), hex(plain), hex(&c)));
    }
    Vectors {
        pattern: RELAY_NOISE_PATTERN.into(),
        prologue: prologue.into(),
        responder_static_private: hex(static_private),
        responder_static_public: hex(static_public),
        initiator_ephemeral: hex(ie),
        responder_ephemeral: hex(re),
        message1: hex(&m1),
        message2: hex(&m2),
        transport,
    }
}

#[test]
fn noise_vectors_match_snow() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/ui/src/client/relay/noise-vectors.json");
    let prologue = format!("{RELAY_PROLOGUE}AAAAAAAAAAAAAAAAAAAAAA");
    if std::env::var_os("UPDATE_NOISE_VECTORS").is_some() {
        let kp = snow::Builder::new(RELAY_NOISE_PATTERN.parse().unwrap())
            .generate_keypair()
            .unwrap();
        let v = compute(&kp.private, &kp.public, &[0x11; 32], &[0x22; 32], &prologue);
        std::fs::write(&path, serde_json::to_string_pretty(&v).unwrap() + "\n").unwrap();
    }
    let stored: Vectors =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("vector file")).unwrap();
    let again = compute(
        &unhex(&stored.responder_static_private),
        &unhex(&stored.responder_static_public),
        &unhex(&stored.initiator_ephemeral),
        &unhex(&stored.responder_ephemeral),
        &stored.prologue,
    );
    assert_eq!(again, stored);
}
