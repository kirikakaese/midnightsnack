// SPDX-License-Identifier: GPL-3.0-or-later
//! The audience-facing output window.
//!
//! The output is a borderless, always-on-top window covering the chosen display (not OS
//! fullscreen, which on macOS animates into a separate Space). It never takes focus, hides the
//! cursor, and keeps the screen awake while open.

use serde::Serialize;
use tauri::{AppHandle, Manager, Monitor, WebviewUrl, WebviewWindowBuilder};

pub const OUTPUT_LABEL: &str = "output-1";
pub const STAGE_LABEL: &str = "stage-1";

/// The two kinds of display windows the host opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayWindow {
    /// Audience output: borderless, no cursor.
    Output,
    /// Stage display for presenters: borderless on a display, cursor allowed.
    Stage,
}

impl DisplayWindow {
    pub fn label(self) -> &'static str {
        match self {
            DisplayWindow::Output => OUTPUT_LABEL,
            DisplayWindow::Stage => STAGE_LABEL,
        }
    }

    fn route(self) -> &'static str {
        match self {
            DisplayWindow::Output => "output",
            DisplayWindow::Stage => "stage",
        }
    }

    fn title(self) -> &'static str {
        match self {
            DisplayWindow::Output => "midnightsnack output",
            DisplayWindow::Stage => "midnightsnack stage display",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
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

/// Opens (or moves) the output window. `display` is a name from [`list_displays`]; `None`
/// picks the first non-primary display, falling back to a window.
pub fn open(
    app: &AppHandle,
    kind: DisplayWindow,
    display: Option<&str>,
    windowed: bool,
) -> tauri::Result<()> {
    let monitors = app.available_monitors()?;
    let primary = app.primary_monitor()?.and_then(|m| m.name().cloned());
    let target = monitors
        .iter()
        .enumerate()
        .find(|(i, m)| display.is_some_and(|d| display_name(m, *i) == d))
        .or_else(|| {
            monitors
                .iter()
                .enumerate()
                .find(|(_, m)| m.name().cloned() != primary)
        })
        .map(|(_, m)| m.clone());
    let windowed = windowed || target.is_none();

    if let Some(w) = app.get_webview_window(kind.label()) {
        w.close()?;
    }

    let mut builder = WebviewWindowBuilder::new(
        app,
        kind.label(),
        WebviewUrl::App(format!("index.html#/{}/{}", kind.route(), kind.label()).into()),
    )
    .title(kind.title())
    .focused(false)
    .background_color(tauri::window::Color(0, 0, 0, 255));

    if windowed {
        builder = builder.inner_size(960.0, 540.0).resizable(true);
    } else {
        let m = target.as_ref().expect("checked above");
        let scale = m.scale_factor();
        let pos = m.position().to_logical::<f64>(scale);
        let size = m.size().to_logical::<f64>(scale);
        builder = builder
            .decorations(false)
            .resizable(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .shadow(false)
            .position(pos.x, pos.y)
            .inner_size(size.width, size.height);
    }
    let window = builder.build()?;
    if !windowed {
        // Some window managers ignore the initial position; enforce it.
        if let Some(m) = &target {
            let _ = window.set_position(*m.position());
            let _ = window.set_size(*m.size());
        }
        if kind == DisplayWindow::Output {
            let _ = window.set_cursor_visible(false);
        }
    }
    Ok(())
}

pub fn close(app: &AppHandle, kind: DisplayWindow) -> tauri::Result<()> {
    if let Some(w) = app.get_webview_window(kind.label()) {
        w.close()?;
    }
    Ok(())
}

pub fn is_open(app: &AppHandle, kind: DisplayWindow) -> bool {
    app.get_webview_window(kind.label()).is_some()
}
