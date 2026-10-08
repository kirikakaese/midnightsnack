// SPDX-License-Identifier: GPL-3.0-or-later
//! Pairing: one-time join token (QR) + PIN + operator approval.

use std::collections::HashMap;
use std::net::IpAddr;
use std::time::{Duration, Instant};

use midnightsnack_core::now_ms;
use midnightsnack_protocol::{ErrorCode, PairStatus, PendingPairing, Role};
use rand::Rng;

use crate::util::random_id;

/// Failed PIN attempts per client before it is locked out.
pub const MAX_FAILURES_PER_CLIENT: u32 = 5;
/// Failed attempts across all clients (per window) before pairing is locked globally.
pub const MAX_FAILURES_GLOBAL: u32 = 20;
pub const LOCKOUT: Duration = Duration::from_secs(60);
const FAILURE_WINDOW: Duration = Duration::from_secs(60);
const REQUEST_TTL: Duration = Duration::from_secs(300);

#[derive(Debug)]
enum RequestState {
    Pending,
    Approved {
        token: String,
        device_id: String,
        role: Role,
    },
    Denied,
}

#[derive(Debug)]
struct Request {
    device_name: String,
    address: IpAddr,
    created: Instant,
    created_ms: i64,
    state: RequestState,
}

#[derive(Debug, Default)]
struct Failures {
    count: u32,
    window_start: Option<Instant>,
    locked_until: Option<Instant>,
}

impl Failures {
    fn locked(&self, now: Instant) -> bool {
        self.locked_until.is_some_and(|t| now < t)
    }

    /// Records a failure; returns true if this locks the client.
    fn record(&mut self, now: Instant, max: u32) -> bool {
        if self
            .window_start
            .is_none_or(|s| now.duration_since(s) > FAILURE_WINDOW)
        {
            self.window_start = Some(now);
            self.count = 0;
        }
        self.count += 1;
        if self.count >= max {
            self.locked_until = Some(now + LOCKOUT);
            self.count = 0;
            self.window_start = None;
            return true;
        }
        false
    }
}

pub struct Pairing {
    pin: String,
    join_token: String,
    pub auto_approve: Option<Role>,
    requests: HashMap<String, Request>,
    failures: HashMap<IpAddr, Failures>,
    global: Failures,
}

/// Outcome of a valid pairing request.
pub enum Submitted {
    /// Waiting for the operator.
    Pending(String),
    /// Auto-approved; the caller must create the device and call [`Pairing::approve`].
    AutoApprove(String, Role),
}

impl Pairing {
    pub fn new(auto_approve: Option<Role>) -> Self {
        Pairing {
            pin: new_pin(),
            join_token: random_id(),
            auto_approve,
            requests: HashMap::new(),
            failures: HashMap::new(),
            global: Failures::default(),
        }
    }

    pub fn pin(&self) -> &str {
        &self.pin
    }

    pub fn join_token(&self) -> &str {
        &self.join_token
    }

    /// Creates a new PIN and join token (invalidates the shown QR code).
    pub fn rotate(&mut self) {
        self.pin = new_pin();
        self.join_token = random_id();
    }

    fn gc(&mut self, now: Instant) {
        self.requests
            .retain(|_, r| now.duration_since(r.created) < REQUEST_TTL);
        self.failures
            .retain(|_, f| f.locked(now) || f.window_start.is_some());
    }

    /// Validates a pairing attempt. Consumes the join token on success.
    pub fn submit(
        &mut self,
        join_token: &str,
        pin: &str,
        device_name: &str,
        address: IpAddr,
    ) -> Result<Submitted, ErrorCode> {
        let now = Instant::now();
        self.gc(now);
        if self.global.locked(now) || self.failures.get(&address).is_some_and(|f| f.locked(now)) {
            return Err(ErrorCode::PairingLocked);
        }
        let token_ok = constant_time_eq(join_token, &self.join_token);
        let pin_ok = constant_time_eq(pin.trim(), &self.pin);
        if !(token_ok && pin_ok) {
            let locked = self
                .failures
                .entry(address)
                .or_default()
                .record(now, MAX_FAILURES_PER_CLIENT);
            let global_locked = self.global.record(now, MAX_FAILURES_GLOBAL);
            if locked || global_locked {
                tracing::warn!(%address, global_locked, "pairing locked after failed attempts");
                return Err(ErrorCode::PairingLocked);
            }
            return Err(if token_ok {
                ErrorCode::InvalidPin
            } else {
                ErrorCode::InvalidJoinToken
            });
        }
        self.failures.remove(&address);
        // One-time token: the QR code changes after every successful submission.
        self.join_token = random_id();

        let name: String = device_name
            .chars()
            .filter(|c| !c.is_control())
            .take(64)
            .collect::<String>();
        let name = if name.trim().is_empty() {
            "Remote".to_owned()
        } else {
            name.trim().to_owned()
        };
        let id = random_id();
        self.requests.insert(
            id.clone(),
            Request {
                device_name: name,
                address,
                created: now,
                created_ms: now_ms(),
                state: RequestState::Pending,
            },
        );
        Ok(match self.auto_approve {
            Some(role) => Submitted::AutoApprove(id, role),
            None => Submitted::Pending(id),
        })
    }

    pub fn device_name(&self, request_id: &str) -> Option<String> {
        self.requests
            .get(request_id)
            .filter(|r| matches!(r.state, RequestState::Pending))
            .map(|r| r.device_name.clone())
    }

    pub fn approve(
        &mut self,
        request_id: &str,
        device_id: String,
        token: String,
        role: Role,
    ) -> bool {
        match self.requests.get_mut(request_id) {
            Some(r) if matches!(r.state, RequestState::Pending) => {
                r.state = RequestState::Approved {
                    token,
                    device_id,
                    role,
                };
                true
            }
            _ => false,
        }
    }

    pub fn deny(&mut self, request_id: &str) -> bool {
        match self.requests.get_mut(request_id) {
            Some(r) if matches!(r.state, RequestState::Pending) => {
                r.state = RequestState::Denied;
                true
            }
            _ => false,
        }
    }

    /// Polled by the remote. An approved token is handed out exactly once.
    pub fn status(&mut self, request_id: &str) -> Option<PairStatus> {
        let r = self.requests.get(request_id)?;
        match &r.state {
            RequestState::Pending => Some(PairStatus::Pending),
            RequestState::Denied => {
                self.requests.remove(request_id);
                Some(PairStatus::Denied)
            }
            RequestState::Approved { .. } => {
                let r = self.requests.remove(request_id)?;
                let RequestState::Approved {
                    token,
                    device_id,
                    role,
                } = r.state
                else {
                    unreachable!()
                };
                Some(PairStatus::Approved {
                    token,
                    device_id,
                    role,
                })
            }
        }
    }

    pub fn pending(&self) -> Vec<PendingPairing> {
        let mut v: Vec<_> = self
            .requests
            .iter()
            .filter(|(_, r)| matches!(r.state, RequestState::Pending))
            .map(|(id, r)| PendingPairing {
                request_id: id.clone(),
                device_name: r.device_name.clone(),
                address: r.address.to_string(),
                requested_at_ms: r.created_ms,
            })
            .collect();
        v.sort_by_key(|p| p.requested_at_ms);
        v
    }
}

fn new_pin() -> String {
    format!("{:06}", rand::rng().random_range(0..1_000_000))
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    const IP: IpAddr = IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 1, 20));
    const IP2: IpAddr = IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 1, 21));

    fn ok(p: &mut Pairing, ip: IpAddr) -> Result<Submitted, ErrorCode> {
        let (t, pin) = (p.join_token().to_owned(), p.pin().to_owned());
        p.submit(&t, &pin, "Phone", ip)
    }

    #[test]
    fn happy_path_hands_out_token_once() {
        let mut p = Pairing::new(None);
        let Submitted::Pending(id) = ok(&mut p, IP).unwrap() else {
            panic!()
        };
        assert_eq!(p.status(&id), Some(PairStatus::Pending));
        assert_eq!(p.pending().len(), 1);
        assert!(p.approve(&id, "dev".into(), "tok".into(), Role::Operator));
        assert_eq!(
            p.status(&id),
            Some(PairStatus::Approved {
                token: "tok".into(),
                device_id: "dev".into(),
                role: Role::Operator
            })
        );
        assert_eq!(p.status(&id), None);
    }

    #[test]
    fn join_token_is_one_time() {
        let mut p = Pairing::new(None);
        let (t, pin) = (p.join_token().to_owned(), p.pin().to_owned());
        assert!(p.submit(&t, &pin, "A", IP).is_ok());
        assert_eq!(
            p.submit(&t, &pin, "B", IP2).err(),
            Some(ErrorCode::InvalidJoinToken)
        );
    }

    #[test]
    fn wrong_pin_locks_client_after_limit() {
        let mut p = Pairing::new(None);
        let t = p.join_token().to_owned();
        let wrong = if p.pin() == "000000" {
            "111111"
        } else {
            "000000"
        };
        for _ in 0..MAX_FAILURES_PER_CLIENT - 1 {
            assert_eq!(
                p.submit(&t, wrong, "x", IP).err(),
                Some(ErrorCode::InvalidPin)
            );
        }
        assert_eq!(
            p.submit(&t, wrong, "x", IP).err(),
            Some(ErrorCode::PairingLocked)
        );
        // Even the right PIN is refused while locked.
        let pin = p.pin().to_owned();
        assert_eq!(
            p.submit(&t, &pin, "x", IP).err(),
            Some(ErrorCode::PairingLocked)
        );
        // Other clients are unaffected.
        assert!(p.submit(&t, &pin, "y", IP2).is_ok());
    }

    #[test]
    fn global_lockout() {
        let mut p = Pairing::new(None);
        let t = p.join_token().to_owned();
        let wrong = if p.pin() == "000000" {
            "111111"
        } else {
            "000000"
        };
        let mut locked = false;
        for i in 0..MAX_FAILURES_GLOBAL {
            let ip = IpAddr::V4(std::net::Ipv4Addr::new(
                10,
                0,
                (i / 250) as u8,
                (i % 250) as u8,
            ));
            locked |= p.submit(&t, wrong, "x", ip).err() == Some(ErrorCode::PairingLocked);
        }
        assert!(locked);
        assert_eq!(ok(&mut p, IP).err(), Some(ErrorCode::PairingLocked));
    }

    #[test]
    fn deny_and_auto_approve() {
        let mut p = Pairing::new(None);
        let Submitted::Pending(id) = ok(&mut p, IP).unwrap() else {
            panic!()
        };
        assert!(p.deny(&id));
        assert_eq!(p.status(&id), Some(PairStatus::Denied));

        p.auto_approve = Some(Role::Presenter);
        assert!(matches!(
            ok(&mut p, IP).unwrap(),
            Submitted::AutoApprove(_, Role::Presenter)
        ));
    }

    #[test]
    fn device_names_are_sanitized() {
        let mut p = Pairing::new(None);
        let (t, pin) = (p.join_token().to_owned(), p.pin().to_owned());
        let Submitted::Pending(id) = p.submit(&t, &pin, "  \u{1b}[31m  ", IP).unwrap() else {
            panic!()
        };
        assert_eq!(p.device_name(&id).unwrap(), "[31m");
    }
}
