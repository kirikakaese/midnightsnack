// SPDX-License-Identifier: GPL-3.0-or-later
//! OSC address space (`/midnightsnack/...`) and state feedback.
//!
//! | Address                          | Arguments          | Does                               |
//! | -------------------------------- | ------------------ | ---------------------------------- |
//! | `/midnightsnack/go`              |                    | Start / next slide                 |
//! | `/midnightsnack/next`, `/prev`   |                    | Next / previous slide              |
//! | `/midnightsnack/next_cue`, `/prev_cue` |              | Next cue / start of cue            |
//! | `/midnightsnack/cue/{n}`         | `[slide]`          | Go to cue *n* (1-based), slide 1   |
//! | `/midnightsnack/goto`            | `cue slide`        | Go to cue and slide (1-based)      |
//! | `/midnightsnack/blackout`        | `[0\|1]`           | Set, or toggle without argument    |
//! | `/midnightsnack/freeze`, `/logo` | `[0\|1]`           | Same                               |
//! | `/midnightsnack/panic`           |                    | Logo now, release freeze           |
//! | `/midnightsnack/overlay/{id}`    | `[0\|1]`           | Show/hide/toggle an overlay        |
//! | `/midnightsnack/timer/{start,pause,reset}` |          | Slide timer                        |
//! | `/midnightsnack/countdown/{start,pause,reset}` |      | Countdown                          |
//! | `/midnightsnack/countdown/set`   | `seconds`          | Countdown duration                 |
//! | `/midnightsnack/media/{play,pause,restart}` |         | Media on the output                |
//! | `/midnightsnack/clear_drawing`   |                    | Remove drawings                    |
//! | `/midnightsnack/auth`            | `token`            | Authenticate this sender           |
//! | `/midnightsnack/subscribe`       | `[port]`           | Receive state feedback             |
//! | `/midnightsnack/unsubscribe`     |                    | Stop feedback                      |

use midnightsnack_protocol::{Action, StateSummary};
use rosc::{OscBundle, OscMessage, OscPacket, OscTime, OscType};

pub const PREFIX: &str = "/midnightsnack";

/// What an OSC message asks for.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Action(Action),
    /// 1-based cue number and slide.
    GoToCue {
        cue: u32,
        slide: u32,
    },
    /// New countdown duration (keeps the label).
    CountdownDuration(u32),
    Auth(String),
    /// Feedback to the sender's address, optionally on another port.
    Subscribe(Option<u16>),
    Unsubscribe,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OscError {
    Decode,
    UnknownAddress(String),
    BadArguments(String),
}

/// Decodes a datagram (message or bundle) into commands.
pub fn decode(datagram: &[u8]) -> Result<Vec<Result<Command, OscError>>, OscError> {
    let (_, packet) = rosc::decoder::decode_udp(datagram).map_err(|_| OscError::Decode)?;
    let mut out = Vec::new();
    collect(packet, &mut out);
    Ok(out)
}

fn collect(packet: OscPacket, out: &mut Vec<Result<Command, OscError>>) {
    match packet {
        OscPacket::Message(m) => out.push(parse(&m)),
        OscPacket::Bundle(b) => {
            for p in b.content {
                collect(p, out);
            }
        }
    }
}

fn number(arg: &OscType) -> Option<f64> {
    match arg {
        OscType::Int(i) => Some(*i as f64),
        OscType::Long(i) => Some(*i as f64),
        OscType::Float(f) => Some(*f as f64),
        OscType::Double(f) => Some(*f),
        OscType::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        OscType::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// `[0|1]` → `Some(bool)`, no argument → `None` (toggle).
fn switch(args: &[OscType], addr: &str) -> Result<Option<bool>, OscError> {
    match args.first() {
        None => Ok(None),
        Some(a) => number(a)
            .map(|n| Some(n >= 0.5))
            .ok_or_else(|| OscError::BadArguments(addr.to_owned())),
    }
}

fn positive(args: &[OscType], i: usize, addr: &str) -> Result<Option<u32>, OscError> {
    match args.get(i) {
        None => Ok(None),
        Some(a) => match number(a) {
            Some(n) if n >= 1.0 && n <= u32::MAX as f64 => Ok(Some(n as u32)),
            _ => Err(OscError::BadArguments(addr.to_owned())),
        },
    }
}

pub fn parse(m: &OscMessage) -> Result<Command, OscError> {
    let addr = m.addr.as_str();
    let unknown = || OscError::UnknownAddress(addr.to_owned());
    let path = addr.strip_prefix(PREFIX).ok_or_else(unknown)?;
    let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
    let args = &m.args;
    let action = |a: Action| Ok(Command::Action(a));
    match parts.as_slice() {
        ["go"] => action(Action::Go),
        ["next"] => action(Action::Next),
        ["prev"] => action(Action::Prev),
        ["next_cue"] => action(Action::NextCue),
        ["prev_cue"] => action(Action::PrevCue),
        ["panic"] => action(Action::Panic),
        ["clear_drawing"] => action(Action::ClearDrawing),
        ["blackout"] => action(match switch(args, addr)? {
            Some(on) => Action::SetBlackout { on },
            None => Action::ToggleBlackout,
        }),
        ["freeze"] => action(match switch(args, addr)? {
            Some(on) => Action::SetFreeze { on },
            None => Action::ToggleFreeze,
        }),
        ["logo"] => action(match switch(args, addr)? {
            Some(on) => Action::SetLogo { on },
            None => Action::ToggleLogo,
        }),
        ["overlay", id] if !id.is_empty() => action(match switch(args, addr)? {
            Some(visible) => Action::SetOverlayVisible {
                overlay_id: (*id).to_owned(),
                visible,
            },
            None => Action::ToggleOverlay {
                overlay_id: (*id).to_owned(),
            },
        }),
        ["cue", n] => {
            let cue: u32 = n.parse().ok().filter(|&c| c >= 1).ok_or_else(unknown)?;
            let slide = positive(args, 0, addr)?.unwrap_or(1);
            Ok(Command::GoToCue { cue, slide })
        }
        ["goto"] => {
            let cue =
                positive(args, 0, addr)?.ok_or_else(|| OscError::BadArguments(addr.to_owned()))?;
            let slide = positive(args, 1, addr)?.unwrap_or(1);
            Ok(Command::GoToCue { cue, slide })
        }
        ["timer", "start"] => action(Action::TimerStart),
        ["timer", "pause"] => action(Action::TimerPause),
        ["timer", "reset"] => action(Action::TimerReset),
        ["countdown", "start"] => action(Action::CountdownStart),
        ["countdown", "pause"] => action(Action::CountdownPause),
        ["countdown", "reset"] => action(Action::CountdownReset),
        ["countdown", "set"] => {
            let seconds = args
                .first()
                .and_then(number)
                .filter(|s| *s > 0.0 && *s <= 24.0 * 3600.0)
                .ok_or_else(|| OscError::BadArguments(addr.to_owned()))?;
            Ok(Command::CountdownDuration((seconds * 1000.0) as u32))
        }
        ["media", "play"] => action(Action::MediaPlay),
        ["media", "pause"] => action(Action::MediaPause),
        ["media", "restart"] => action(Action::MediaRestart),
        ["auth"] => match args.first() {
            Some(OscType::String(t)) => Ok(Command::Auth(t.clone())),
            _ => Err(OscError::BadArguments(addr.to_owned())),
        },
        ["subscribe"] => {
            let port = match args.first() {
                None => None,
                Some(a) => Some(
                    number(a)
                        .filter(|p| (1.0..=65535.0).contains(p))
                        .map(|p| p as u16)
                        .ok_or_else(|| OscError::BadArguments(addr.to_owned()))?,
                ),
            };
            Ok(Command::Subscribe(port))
        }
        ["unsubscribe"] => Ok(Command::Unsubscribe),
        _ => Err(unknown()),
    }
}

/// `m:ss`, negative with a leading `-` (overtime).
pub fn clock(ms: i64) -> String {
    let sign = if ms < 0 { "-" } else { "" };
    let s = ms.unsigned_abs() / 1000;
    if s >= 3600 {
        format!("{sign}{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
    } else {
        format!("{sign}{}:{:02}", s / 60, s % 60)
    }
}

fn msg(path: &str, args: Vec<OscType>) -> OscPacket {
    OscPacket::Message(OscMessage {
        addr: format!("{PREFIX}/state/{path}"),
        args,
    })
}

/// State feedback as a bundle of `/midnightsnack/state/...` messages.
pub fn feedback(s: &StateSummary) -> Vec<u8> {
    let int = |b: bool| OscType::Int(b as i32);
    let p = s.program.as_ref();
    let content = vec![
        msg("blackout", vec![int(s.masters.blackout)]),
        msg("freeze", vec![int(s.masters.freeze)]),
        msg("logo", vec![int(s.masters.logo)]),
        msg(
            "cue/number",
            vec![OscType::Int(p.map_or(0, |c| c.cue_number as i32))],
        ),
        msg(
            "cue/name",
            vec![OscType::String(
                p.map(|c| c.name.clone()).unwrap_or_default(),
            )],
        ),
        msg("slide", vec![OscType::Int(p.map_or(0, |c| c.slide as i32))]),
        msg(
            "slide_count",
            vec![OscType::Int(p.map_or(0, |c| c.slide_count as i32))],
        ),
        msg(
            "next/name",
            vec![OscType::String(
                s.next.as_ref().map(|c| c.name.clone()).unwrap_or_default(),
            )],
        ),
        msg(
            "countdown",
            vec![OscType::String(clock(s.countdown_remaining_ms))],
        ),
        msg("countdown/running", vec![int(s.countdown_running)]),
        msg("show_timer", vec![OscType::String(clock(s.show_timer_ms))]),
        msg(
            "slide_timer",
            vec![OscType::String(clock(s.slide_timer_ms))],
        ),
    ];
    rosc::encoder::encode(&OscPacket::Bundle(OscBundle {
        timetag: OscTime::from((0, 1)),
        content,
    }))
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use midnightsnack_protocol::{CueRef, Masters};

    fn m(addr: &str, args: Vec<OscType>) -> OscMessage {
        OscMessage {
            addr: addr.into(),
            args,
        }
    }

    fn cmd(addr: &str, args: Vec<OscType>) -> Result<Command, OscError> {
        parse(&m(addr, args))
    }

    #[test]
    fn navigation_and_masters() {
        assert_eq!(
            cmd("/midnightsnack/go", vec![]),
            Ok(Command::Action(Action::Go))
        );
        assert_eq!(
            cmd("/midnightsnack/next/", vec![]),
            Ok(Command::Action(Action::Next))
        );
        assert_eq!(
            cmd("/midnightsnack/blackout", vec![OscType::Int(1)]),
            Ok(Command::Action(Action::SetBlackout { on: true }))
        );
        assert_eq!(
            cmd("/midnightsnack/blackout", vec![OscType::Float(0.0)]),
            Ok(Command::Action(Action::SetBlackout { on: false }))
        );
        assert_eq!(
            cmd("/midnightsnack/freeze", vec![]),
            Ok(Command::Action(Action::ToggleFreeze))
        );
        assert_eq!(
            cmd("/midnightsnack/logo", vec![OscType::Bool(true)]),
            Ok(Command::Action(Action::SetLogo { on: true }))
        );
        assert_eq!(
            cmd("/midnightsnack/overlay/clock", vec![]),
            Ok(Command::Action(Action::ToggleOverlay {
                overlay_id: "clock".into()
            }))
        );
    }

    #[test]
    fn cues_timers_and_session() {
        assert_eq!(
            cmd("/midnightsnack/cue/3", vec![]),
            Ok(Command::GoToCue { cue: 3, slide: 1 })
        );
        assert_eq!(
            cmd(
                "/midnightsnack/goto",
                vec![OscType::Int(2), OscType::Int(5)]
            ),
            Ok(Command::GoToCue { cue: 2, slide: 5 })
        );
        assert!(cmd("/midnightsnack/cue/0", vec![]).is_err());
        assert!(cmd("/midnightsnack/goto", vec![]).is_err());
        assert_eq!(
            cmd("/midnightsnack/countdown/set", vec![OscType::Int(90)]),
            Ok(Command::CountdownDuration(90_000))
        );
        assert_eq!(
            cmd("/midnightsnack/auth", vec![OscType::String("t".into())]),
            Ok(Command::Auth("t".into()))
        );
        assert_eq!(
            cmd("/midnightsnack/subscribe", vec![OscType::Int(9000)]),
            Ok(Command::Subscribe(Some(9000)))
        );
        assert_eq!(
            cmd("/other/go", vec![]),
            Err(OscError::UnknownAddress("/other/go".into()))
        );
        assert!(cmd("/midnightsnack/blackout", vec![OscType::Nil]).is_err());
    }

    #[test]
    fn bundles_and_round_trip() {
        let packet = OscPacket::Bundle(OscBundle {
            timetag: OscTime::from((0, 1)),
            content: vec![
                OscPacket::Message(m("/midnightsnack/next", vec![])),
                OscPacket::Message(m("/midnightsnack/nope", vec![])),
            ],
        });
        let bytes = rosc::encoder::encode(&packet).unwrap();
        let cmds = decode(&bytes).unwrap();
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0], Ok(Command::Action(Action::Next)));
        assert!(cmds[1].is_err());
        assert_eq!(decode(b"garbage"), Err(OscError::Decode));
    }

    #[test]
    fn feedback_reports_state() {
        let s = StateSummary {
            show_title: "Gala".into(),
            cue_count: 2,
            program: Some(CueRef {
                cue_id: "c".into(),
                name: "Intro".into(),
                cue_number: 1,
                slide: 2,
                slide_count: 4,
            }),
            output: None,
            next: None,
            masters: Masters {
                blackout: true,
                freeze: false,
                logo: false,
            },
            show_timer_ms: 61_000,
            slide_timer_ms: 0,
            countdown_remaining_ms: -12_000,
            countdown_running: true,
            overlays_visible: vec![],
        };
        let (_, packet) = rosc::decoder::decode_udp(&feedback(&s)).unwrap();
        let OscPacket::Bundle(b) = packet else {
            panic!("bundle")
        };
        let find = |a: &str| {
            b.content.iter().find_map(|p| match p {
                OscPacket::Message(m) if m.addr == format!("/midnightsnack/state/{a}") => {
                    Some(m.args[0].clone())
                }
                _ => None,
            })
        };
        assert_eq!(find("blackout"), Some(OscType::Int(1)));
        assert_eq!(find("cue/name"), Some(OscType::String("Intro".into())));
        assert_eq!(find("slide"), Some(OscType::Int(2)));
        assert_eq!(find("countdown"), Some(OscType::String("-0:12".into())));
        assert_eq!(find("show_timer"), Some(OscType::String("1:01".into())));
    }

    #[test]
    fn clock_formats() {
        assert_eq!(clock(0), "0:00");
        assert_eq!(clock(59_999), "0:59");
        assert_eq!(clock(3_725_000), "1:02:05");
        assert_eq!(clock(-5_000), "-0:05");
    }
}
