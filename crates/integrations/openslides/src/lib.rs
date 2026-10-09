// SPDX-License-Identifier: GPL-3.0-or-later
//! OpenSlides 4 adapter (read-only): logs in through the auth service, subscribes to a
//! meeting through the autoupdate service and turns the data into [`OsMeetingData`] for
//! midnightsnack's native slides. See `docs/adr/0013-openslides-adapter.md`.
//!
//! [`OsMeetingData`]: midnightsnack_protocol::OsMeetingData

pub mod client;
pub mod html;
#[cfg(feature = "mock")]
pub mod mock;
pub mod request;
pub mod store;
pub mod view;

pub use client::{normalize_url, run, Config, Error, Session, Update};
pub use store::Store;
pub use view::pages;
