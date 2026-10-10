// SPDX-License-Identifier: GPL-3.0-or-later
//! Output windows, one per output of the show.
//!
//! An output is a borderless, always-on-top window covering its display (not OS fullscreen,
//! which on macOS animates into a separate Space). It never takes focus and hides the cursor.
//! The window holds a page webview (the Stage renderer) and, while a web page cue is live or
//! next, child webviews for those pages (see `web.rs`).

use serde::Serialize;
use tauri::webview::WebviewBuilder;
use tauri::window::WindowBuilder;
use tauri::{AppHandle, LogicalPosition, Manager, Monitor, WebviewUrl, Window};

pub const LABEL_PREFIX: &str = "output-";

pub fn label(output_id: &str) -> String {
    format!("{LABEL_PREFIX}{output_id}")
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DisplayInfo {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    pub scale: f64,
    pub primary: bool,
}

fn display_name(m: &Monitor, index: usize) -> String {
    m.name()
        .cloned()
        .unwrap_or_else(|| format!("Display {}", index + 1))
}

pub fn list_displays(app: &AppHandle) -> Vec<DisplayInfo> {
    let primary = app
        .primary_monitor()
        .ok()
        .flatten()
        .and_then(|m| m.name().cloned());
    app.available_monitors()
        .unwrap_or_default()
        .iter()
        .enumerate()
        .map(|(i, m)| DisplayInfo {
            name: display_name(m, i),
            width: m.size().width,
            height: m.size().height,
            x: m.position().x,
            y: m.position().y,
            scale: m.scale_factor(),
            primary: primary.as_deref() == m.name().map(String::as_str),
        })
        .collect()
}

fn find_monitor(app: &AppHandle, display: Option<&str>) -> Option<Monitor> {
    let monitors = app.available_monitors().ok()?;
    let primary = app
        .primary_monitor()
        .ok()
        .flatten()
        .and_then(|m| m.name().cloned());
    monitors
        .iter()
        .enumerate()
        .find(|(i, m)| display.is_some_and(|d| display_name(m, *i) == d))
        .or_else(|| {
            // Without a choice, use the first display that is not the operator's.
            if display.is_some() {
                return None;
            }
            monitors
                .iter()
                .enumerate()
                .find(|(_, m)| m.name().cloned() != primary)
        })
        .map(|(_, m)| m.clone())
}

/// Moves and sizes a window to cover a monitor.
pub fn place(window: &Window, monitor: &Monitor) {
    let _ = window.set_position(*monitor.position());
    let _ = window.set_size(*monitor.size());
}

/// Opens (or re-opens) the window of an output. Returns whether it covers a display (`false`:
/// windowed, because asked to or because the display is not connected).
pub fn open(
    app: &AppHandle,
    output_id: &str,
    display: Option<&str>,
    windowed: bool,
) -> tauri::Result<bool> {
    let label = label(output_id);
    if let Some(w) = app.get_window(&label) {
        w.destroy()?;
    }
    let target = if windowed {
        None
    } else {
        find_monitor(app, display)
    };

    let mut builder = WindowBuilder::new(app, &label)
        .title(format!("DECK — {output_id}"))
        .focused(false)
        .background_color(tauri::window::Color(0, 0, 0, 255));
    builder = match &target {
        None => builder.inner_size(960.0, 540.0).resizable(true),
        Some(m) => {
            let scale = m.scale_factor();
            let pos = m.position().to_logical::<f64>(scale);
            let size = m.size().to_logical::<f64>(scale);
            builder
                .decorations(false)
                .resizable(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .shadow(false)
                .position(pos.x, pos.y)
                .inner_size(size.width, size.height)
        }
    };
    let window = builder.build()?;
    if let Some(m) = &target {
        // Some window managers ignore the initial position; enforce it.
        place(&window, m);
    }

    let url = format!("index.html#/output/{output_id}");
    let page = WebviewBuilder::new(&label, WebviewUrl::App(url.into())).auto_resize();
    let size = window.inner_size()?;
    window.add_child(page, LogicalPosition::new(0.0, 0.0), size)?;
    if target.is_some() {
        let _ = window.set_cursor_visible(false);
    }
    Ok(target.is_some())
}

pub fn close(app: &AppHandle, output_id: &str) -> tauri::Result<()> {
    if let Some(w) = app.get_window(&label(output_id)) {
        w.destroy()?;
    }
    Ok(())
}

pub fn window(app: &AppHandle, output_id: &str) -> Option<Window> {
    app.get_window(&label(output_id))
}

/// Re-places a covering output window on its display (after a display was re-connected).
pub fn restore(app: &AppHandle, output_id: &str, display: &str) -> bool {
    let (Some(w), Some(m)) = (window(app, output_id), find_monitor(app, Some(display))) else {
        return false;
    };
    place(&w, &m);
    let _ = w.show();
    true
}

/// Output id from a window or webview label.
pub fn output_id(label: &str) -> Option<&str> {
    label.strip_prefix(LABEL_PREFIX)
}
