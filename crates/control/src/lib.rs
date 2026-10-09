// SPDX-License-Identifier: GPL-3.0-or-later
//! Control surfaces: MIDI controllers and OSC. Parsing and mapping are pure and unit-tested;
//! [`midi::MidiListener`] talks to the operating system's MIDI ports.

pub mod midi;
pub mod osc;
