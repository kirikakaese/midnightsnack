// SPDX-License-Identifier: GPL-3.0-or-later
//! Plaintext framing inside the relay's Noise channel: `[kind][stream u32 BE][flags][payload]`.
//! Stream 0 carries the WebSocket session, further streams one HTTP exchange each.

use midnightsnack_protocol::{tunnel, TUNNEL_HEADER, TUNNEL_MAX_PAYLOAD};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub kind: u8,
    pub stream: u32,
    pub fin: bool,
    pub payload: Vec<u8>,
}

impl Frame {
    pub fn new(kind: u8, stream: u32, fin: bool, payload: Vec<u8>) -> Self {
        debug_assert!(payload.len() <= TUNNEL_MAX_PAYLOAD);
        Frame {
            kind,
            stream,
            fin,
            payload,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(TUNNEL_HEADER + self.payload.len());
        out.push(self.kind);
        out.extend_from_slice(&self.stream.to_be_bytes());
        out.push(if self.fin { tunnel::FIN } else { 0 });
        out.extend_from_slice(&self.payload);
        out
    }

    pub fn decode(bytes: &[u8]) -> Option<Frame> {
        if bytes.len() < TUNNEL_HEADER {
            return None;
        }
        let stream = u32::from_be_bytes(bytes[1..5].try_into().ok()?);
        Some(Frame {
            kind: bytes[0],
            stream,
            fin: bytes[5] & tunnel::FIN != 0,
            payload: bytes[TUNNEL_HEADER..].to_vec(),
        })
    }
}

/// Splits a message or body into frames of at most [`TUNNEL_MAX_PAYLOAD`] bytes, FIN on the last.
/// An empty message is one empty FIN frame.
pub fn split(kind: u8, stream: u32, data: &[u8], fin: bool) -> Vec<Frame> {
    if data.is_empty() {
        return vec![Frame::new(kind, stream, fin, Vec::new())];
    }
    let chunks: Vec<&[u8]> = data.chunks(TUNNEL_MAX_PAYLOAD).collect();
    let last = chunks.len() - 1;
    chunks
        .into_iter()
        .enumerate()
        .map(|(i, c)| Frame::new(kind, stream, fin && i == last, c.to_vec()))
        .collect()
}

/// Collects fragments of one message, refusing messages above `max` bytes.
#[derive(Debug, Default)]
pub struct Reassembler {
    buf: Vec<u8>,
    overflow: bool,
}

impl Reassembler {
    /// Adds a fragment. Returns the message when `fin` completes it; `Err` if it got too large
    /// (the rest of that message is then dropped).
    pub fn push(&mut self, payload: &[u8], fin: bool, max: usize) -> Result<Option<Vec<u8>>, ()> {
        if !self.overflow {
            if self.buf.len() + payload.len() > max {
                self.overflow = true;
                self.buf = Vec::new();
            } else {
                self.buf.extend_from_slice(payload);
            }
        }
        if !fin {
            return Ok(None);
        }
        if std::mem::take(&mut self.overflow) {
            return Err(());
        }
        Ok(Some(std::mem::take(&mut self.buf)))
    }
}

/// Head of a tunneled HTTP request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestHead {
    pub method: String,
    /// Path and query, e.g. `/api/v1/media/slide/c1/0?k=…`.
    pub path: String,
    #[serde(default)]
    pub headers: Vec<(String, String)>,
}

/// Head of a tunneled HTTP response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResponseHead {
    pub status: u16,
    #[serde(default)]
    pub headers: Vec<(String, String)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_round_trip() {
        let f = Frame::new(tunnel::REQUEST_HEAD, 7, true, b"{}".to_vec());
        let bytes = f.encode();
        assert_eq!(
            &bytes[..6],
            &[tunnel::REQUEST_HEAD, 0, 0, 0, 7, tunnel::FIN]
        );
        assert_eq!(Frame::decode(&bytes), Some(f));
        assert_eq!(Frame::decode(&[1, 0, 0]), None);
    }

    #[test]
    fn large_messages_are_split_and_reassembled() {
        let data: Vec<u8> = (0..(TUNNEL_MAX_PAYLOAD * 2 + 10))
            .map(|i| i as u8)
            .collect();
        let frames = split(tunnel::WS_MESSAGE, 0, &data, true);
        assert_eq!(frames.len(), 3);
        assert!(frames[..2].iter().all(|f| !f.fin));
        assert!(frames[2].fin);
        let mut r = Reassembler::default();
        let mut out = None;
        for f in &frames {
            out = r.push(&f.payload, f.fin, 1 << 20).unwrap();
        }
        assert_eq!(out.unwrap(), data);
        assert_eq!(split(tunnel::WS_MESSAGE, 0, &[], true).len(), 1);
    }

    #[test]
    fn oversized_messages_are_refused_and_the_next_one_works() {
        let mut r = Reassembler::default();
        assert_eq!(r.push(&[0; 10], false, 15), Ok(None));
        assert_eq!(r.push(&[0; 10], true, 15), Err(()));
        assert_eq!(r.push(b"ok", true, 15), Ok(Some(b"ok".to_vec())));
    }
}
