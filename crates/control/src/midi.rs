// SPDX-License-Identifier: GPL-3.0-or-later
//! MIDI input: decoding note/CC messages into triggers, matching bindings, and listening on all
//! connected input ports (re-scanned so controllers can be plugged in during a show).

use std::collections::HashSet;

use midnightsnack_protocol::{Action, MidiBinding, MidiKind, MidiTrigger};

/// A decoded, button-like MIDI event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Press {
    pub trigger: MidiTrigger,
}

/// Turns raw MIDI messages into presses. Control changes count as a press when they cross from
/// below 64 to 64 or above, so faders and buttons that send 127/0 both behave like buttons.
#[derive(Debug, Default)]
pub struct Decoder {
    cc_high: HashSet<(u8, u8)>,
}

impl Decoder {
    pub fn decode(&mut self, message: &[u8]) -> Option<Press> {
        let (&status, data) = message.split_first()?;
        let channel = status & 0x0F;
        match (status & 0xF0, data) {
            (0x90, &[note, velocity, ..]) if velocity > 0 => Some(Press {
                trigger: MidiTrigger {
                    kind: MidiKind::Note,
                    channel,
                    number: note & 0x7F,
                },
            }),
            (0xB0, &[controller, value, ..]) => {
                let key = (channel, controller);
                if value >= 64 {
                    // Only the rising edge presses.
                    self.cc_high.insert(key).then_some(Press {
                        trigger: MidiTrigger {
                            kind: MidiKind::ControlChange,
                            channel,
                            number: controller & 0x7F,
                        },
                    })
                } else {
                    self.cc_high.remove(&key);
                    None
                }
            }
            _ => None,
        }
    }
}

/// The action bound to a trigger (the first binding wins).
pub fn action_for<'a>(bindings: &'a [MidiBinding], trigger: &MidiTrigger) -> Option<&'a Action> {
    bindings
        .iter()
        .find(|b| b.trigger == *trigger)
        .map(|b| &b.action)
}

/// Adds or replaces the binding for `trigger` (one action per trigger).
pub fn bind(bindings: &mut Vec<MidiBinding>, trigger: MidiTrigger, action: Action) {
    bindings.retain(|b| b.trigger != trigger);
    bindings.push(MidiBinding { trigger, action });
}

/// Talking to the operating system's MIDI ports.
#[cfg(feature = "midi-io")]
mod io {
    use std::collections::HashMap;
    use std::sync::mpsc;
    use std::time::Duration;

    use super::{Decoder, Press};

    /// Names of the MIDI input ports currently available.
    pub fn port_names() -> Vec<String> {
        let Ok(input) = midir::MidiInput::new("DECK") else {
            return Vec::new();
        };
        input
            .ports()
            .iter()
            .filter_map(|p| input.port_name(p).ok())
            .collect()
    }

    /// Listens on every MIDI input port and sends decoded presses. Ports are re-scanned every
    /// two seconds; the listener stops when the receiver is dropped.
    pub struct MidiListener;

    impl MidiListener {
        pub fn spawn(tx: mpsc::Sender<Press>) {
            std::thread::Builder::new()
                .name("midnightsnack-midi".into())
                .spawn(move || run(tx))
                .expect("spawn MIDI thread");
        }
    }

    fn run(tx: mpsc::Sender<Press>) {
        let (raw_tx, raw_rx) = mpsc::channel::<Vec<u8>>();
        let mut connections: HashMap<String, midir::MidiInputConnection<()>> = HashMap::new();
        let mut decoder = Decoder::default();
        loop {
            // Connect new ports, forget vanished ones.
            if let Ok(input) = midir::MidiInput::new("DECK") {
                let ports = input.ports();
                let names: Vec<(String, midir::MidiInputPort)> = ports
                    .into_iter()
                    .filter_map(|p| input.port_name(&p).ok().map(|n| (n, p)))
                    .collect();
                connections.retain(|name, _| names.iter().any(|(n, _)| n == name));
                for (name, port) in names {
                    if connections.contains_key(&name) || name.contains("DECK") {
                        continue;
                    }
                    let Ok(input) = midir::MidiInput::new("DECK") else {
                        continue;
                    };
                    let raw = raw_tx.clone();
                    match input.connect(
                        &port,
                        "midnightsnack-in",
                        move |_, msg, _| {
                            let _ = raw.send(msg.to_vec());
                        },
                        (),
                    ) {
                        Ok(conn) => {
                            tracing::info!(port = %name, "MIDI input connected");
                            connections.insert(name, conn);
                        }
                        Err(e) => {
                            tracing::warn!(port = %name, error = %e, "cannot open MIDI input")
                        }
                    }
                }
            }
            // Forward messages until the next rescan.
            let deadline = std::time::Instant::now() + Duration::from_secs(2);
            while let Some(left) = deadline.checked_duration_since(std::time::Instant::now()) {
                match raw_rx.recv_timeout(left) {
                    Ok(msg) => {
                        if let Some(press) = decoder.decode(&msg) {
                            if tx.send(press).is_err() {
                                return;
                            }
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => break,
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                }
            }
        }
    }
}

#[cfg(feature = "midi-io")]
pub use io::{port_names, MidiListener};

#[cfg(test)]
mod tests {
    use super::*;

    fn trigger(kind: MidiKind, channel: u8, number: u8) -> MidiTrigger {
        MidiTrigger {
            kind,
            channel,
            number,
        }
    }

    #[test]
    fn notes_press_on_note_on_only() {
        let mut d = Decoder::default();
        assert_eq!(
            d.decode(&[0x91, 60, 100]).unwrap().trigger,
            trigger(MidiKind::Note, 1, 60)
        );
        assert!(
            d.decode(&[0x91, 60, 0]).is_none(),
            "note on with velocity 0"
        );
        assert!(d.decode(&[0x81, 60, 64]).is_none(), "note off");
        assert!(d.decode(&[0x90]).is_none(), "truncated");
        assert!(d.decode(&[]).is_none());
    }

    #[test]
    fn control_changes_press_on_the_rising_edge() {
        let mut d = Decoder::default();
        let cc = trigger(MidiKind::ControlChange, 0, 20);
        assert_eq!(d.decode(&[0xB0, 20, 127]).unwrap().trigger, cc);
        assert!(d.decode(&[0xB0, 20, 100]).is_none(), "still held");
        assert!(d.decode(&[0xB0, 20, 0]).is_none());
        assert!(d.decode(&[0xB0, 20, 64]).is_some(), "pressed again");
        assert!(d.decode(&[0xE0, 0, 64]).is_none(), "pitch bend ignored");
    }

    #[test]
    fn bindings_map_triggers_to_actions() {
        let a = trigger(MidiKind::Note, 0, 36);
        let b = trigger(MidiKind::ControlChange, 0, 36);
        let mut bindings = Vec::new();
        bind(&mut bindings, a, Action::Next);
        bind(&mut bindings, b, Action::ToggleBlackout);
        assert_eq!(action_for(&bindings, &a), Some(&Action::Next));
        assert_eq!(action_for(&bindings, &b), Some(&Action::ToggleBlackout));
        bind(&mut bindings, a, Action::Prev);
        assert_eq!(bindings.len(), 2, "rebinding replaces");
        assert_eq!(action_for(&bindings, &a), Some(&Action::Prev));
        assert!(action_for(&bindings, &trigger(MidiKind::Note, 1, 36)).is_none());
    }
}
