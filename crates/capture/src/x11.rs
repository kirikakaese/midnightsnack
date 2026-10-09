// SPDX-License-Identifier: GPL-3.0-or-later
//! Linux capture over X11 (pure Rust, no system libraries): screens are RandR monitors, windows
//! come from the window manager's client list (EWMH), frames are read with `GetImage` over one
//! persistent connection per captured source.

use image::RgbaImage;
use midnightsnack_protocol::{CaptureSource, CaptureTarget};
use x11rb::connection::Connection;
use x11rb::protocol::randr::ConnectionExt as _;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ConnectionExt as _, ImageFormat, ImageOrder, MapState, Window,
};
use x11rb::rust_connection::RustConnection;

use super::{matches, CaptureError};

fn err(e: impl std::fmt::Display) -> CaptureError {
    CaptureError::Capture(e.to_string())
}

fn connect() -> Result<(RustConnection, Window), CaptureError> {
    if std::env::var("XDG_SESSION_TYPE").is_ok_and(|s| s == "wayland") {
        return Err(CaptureError::Capture(
            "screen capture needs an X11 session; Wayland is not supported yet".into(),
        ));
    }
    let (conn, screen) = x11rb::connect(None).map_err(err)?;
    let root = conn
        .setup()
        .roots
        .get(screen)
        .ok_or_else(|| err("no X11 screen"))?
        .root;
    Ok((conn, root))
}

fn atom(conn: &RustConnection, name: &str) -> Result<Atom, CaptureError> {
    Ok(conn
        .intern_atom(false, name.as_bytes())
        .map_err(err)?
        .reply()
        .map_err(err)?
        .atom)
}

struct Monitor {
    name: String,
    x: i16,
    y: i16,
    width: u16,
    height: u16,
}

fn monitors(conn: &RustConnection, root: Window) -> Result<Vec<Monitor>, CaptureError> {
    let reply = conn
        .randr_get_monitors(root, true)
        .map_err(err)?
        .reply()
        .map_err(err)?;
    let mut out = Vec::new();
    for m in reply.monitors {
        let name = conn
            .get_atom_name(m.name)
            .map_err(err)?
            .reply()
            .map(|r| String::from_utf8_lossy(&r.name).into_owned())
            .unwrap_or_default();
        out.push(Monitor {
            name,
            x: m.x,
            y: m.y,
            width: m.width,
            height: m.height,
        });
    }
    Ok(out)
}

struct Win {
    id: Window,
    app: String,
    title: String,
    width: u16,
    height: u16,
}

fn text_property(conn: &RustConnection, w: Window, prop: Atom, ty: Atom) -> Option<String> {
    let reply = conn
        .get_property(false, w, prop, ty, 0, 4096)
        .ok()?
        .reply()
        .ok()?;
    (!reply.value.is_empty()).then(|| String::from_utf8_lossy(&reply.value).into_owned())
}

/// Viewable top-level windows, front-most first.
fn windows(conn: &RustConnection, root: Window) -> Result<Vec<Win>, CaptureError> {
    let stacking = atom(conn, "_NET_CLIENT_LIST_STACKING")?;
    let client_list = atom(conn, "_NET_CLIENT_LIST")?;
    let net_wm_name = atom(conn, "_NET_WM_NAME")?;
    let utf8 = atom(conn, "UTF8_STRING")?;
    let mut ids: Vec<Window> = Vec::new();
    for list in [stacking, client_list] {
        let reply = conn
            .get_property(false, root, list, AtomEnum::WINDOW, 0, u32::MAX)
            .map_err(err)?
            .reply()
            .map_err(err)?;
        if let Some(v) = reply.value32() {
            ids = v.collect();
        }
        if !ids.is_empty() {
            break;
        }
    }
    let mut out = Vec::new();
    // The stacking list is bottom to top.
    for id in ids.into_iter().rev() {
        let viewable = conn
            .get_window_attributes(id)
            .ok()
            .and_then(|c| c.reply().ok())
            .is_some_and(|a| a.map_state == MapState::VIEWABLE);
        if !viewable {
            continue;
        }
        let title = text_property(conn, id, net_wm_name, utf8)
            .or_else(|| text_property(conn, id, AtomEnum::WM_NAME.into(), AtomEnum::STRING.into()))
            .unwrap_or_default();
        // WM_CLASS is "instance\0Class\0"; the class is the application name.
        let app = text_property(conn, id, AtomEnum::WM_CLASS.into(), AtomEnum::STRING.into())
            .map(|c| {
                let mut parts = c.split('\0').filter(|s| !s.is_empty());
                let instance = parts.next().unwrap_or_default().to_owned();
                parts.next().map(str::to_owned).unwrap_or(instance)
            })
            .unwrap_or_default();
        let Some(geometry) = conn.get_geometry(id).ok().and_then(|c| c.reply().ok()) else {
            continue;
        };
        out.push(Win {
            id,
            app,
            title,
            width: geometry.width,
            height: geometry.height,
        });
    }
    Ok(out)
}

pub fn list_targets_once() -> Result<Vec<CaptureTarget>, CaptureError> {
    let (conn, root) = connect()?;
    let mut out: Vec<CaptureTarget> = monitors(&conn, root)?
        .into_iter()
        .map(|m| CaptureTarget {
            source: CaptureSource::Screen { name: m.name },
            width: m.width.into(),
            height: m.height.into(),
        })
        .collect();
    for w in windows(&conn, root)? {
        if w.title.trim().is_empty() || w.app.to_ascii_lowercase().contains("midnightsnack") {
            continue;
        }
        out.push(CaptureTarget {
            source: CaptureSource::Window {
                app: w.app,
                title: w.title,
            },
            width: w.width.into(),
            height: w.height.into(),
        });
    }
    Ok(out)
}

pub struct Target {
    conn: RustConnection,
    drawable: Window,
    /// Region of the root window for screens; `None` = the whole window.
    region: Option<(i16, i16, u16, u16)>,
}

pub fn resolve(source: &CaptureSource) -> Option<Target> {
    let (conn, root) = connect().ok()?;
    let (drawable, region) = match source {
        CaptureSource::Screen { name } => {
            let m = monitors(&conn, root)
                .ok()?
                .into_iter()
                .find(|m| m.name == *name)?;
            (root, Some((m.x, m.y, m.width, m.height)))
        }
        CaptureSource::Window { app, title } => {
            let mut candidates: Vec<Win> = windows(&conn, root)
                .ok()?
                .into_iter()
                .filter(|w| matches(&w.app, app) && matches(&w.title, title))
                .collect();
            // Prefer an exact title match; otherwise keep the front-most window (stable sort).
            candidates.sort_by_key(|w| !w.title.eq_ignore_ascii_case(title.trim()));
            (candidates.into_iter().next()?.id, None)
        }
    };
    Some(Target {
        conn,
        drawable,
        region,
    })
}

impl Target {
    pub fn capture(&mut self) -> Result<RgbaImage, CaptureError> {
        let (x, y, w, h) = match self.region {
            Some(r) => r,
            None => {
                let g = self
                    .conn
                    .get_geometry(self.drawable)
                    .map_err(err)?
                    .reply()
                    .map_err(err)?;
                (0, 0, g.width, g.height)
            }
        };
        let reply = self
            .conn
            .get_image(ImageFormat::Z_PIXMAP, self.drawable, x, y, w, h, !0)
            .map_err(err)?
            .reply()
            .map_err(err)?;
        let setup = self.conn.setup();
        let bpp = setup
            .pixmap_formats
            .iter()
            .find(|f| f.depth == reply.depth)
            .map(|f| f.bits_per_pixel)
            .unwrap_or(0);
        if bpp != 32 || setup.image_byte_order != ImageOrder::LSB_FIRST {
            return Err(CaptureError::Capture(format!(
                "unsupported pixel format ({bpp} bpp)"
            )));
        }
        // BGRX -> RGBA
        let mut rgba = reply.data;
        for px in rgba.chunks_exact_mut(4) {
            px.swap(0, 2);
            px[3] = 255;
        }
        RgbaImage::from_raw(w.into(), h.into(), rgba)
            .ok_or_else(|| CaptureError::Capture("short image".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use x11rb::protocol::xproto::{CreateWindowAux, PropMode, WindowClass};
    use x11rb::wrapper::ConnectionExt as _;

    /// Needs an X11 display without a window manager, e.g. Xvfb (run with `--ignored`): plays
    /// the window manager by publishing the test window in `_NET_CLIENT_LIST`.
    #[test]
    #[ignore]
    fn lists_resolves_and_captures_a_window() {
        let (conn, screen) = x11rb::connect(None).unwrap();
        let s = &conn.setup().roots[screen];
        let (root, depth, visual, white) = (s.root, s.root_depth, s.root_visual, s.white_pixel);
        let win = conn.generate_id().unwrap();
        conn.create_window(
            depth,
            win,
            root,
            10,
            10,
            320,
            200,
            0,
            WindowClass::INPUT_OUTPUT,
            visual,
            &CreateWindowAux::new().background_pixel(white),
        )
        .unwrap();
        conn.change_property8(
            PropMode::REPLACE,
            win,
            AtomEnum::WM_NAME,
            AtomEnum::STRING,
            b"Quarterly numbers",
        )
        .unwrap();
        conn.change_property8(
            PropMode::REPLACE,
            win,
            AtomEnum::WM_CLASS,
            AtomEnum::STRING,
            b"calc\0Spreadsheet\0",
        )
        .unwrap();
        conn.map_window(win).unwrap();
        let list = atom(&conn, "_NET_CLIENT_LIST").unwrap();
        conn.change_property32(PropMode::REPLACE, root, list, AtomEnum::WINDOW, &[win])
            .unwrap();
        conn.sync().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(200));

        let targets = list_targets_once().unwrap();
        let source = CaptureSource::Window {
            app: "Spreadsheet".into(),
            title: "Quarterly numbers".into(),
        };
        let found = targets.iter().find(|t| t.source == source);
        conn.delete_property(root, list).unwrap();
        conn.sync().unwrap();
        let found = found.expect("window listed");
        assert_eq!((found.width, found.height), (320, 200));

        conn.change_property32(PropMode::REPLACE, root, list, AtomEnum::WINDOW, &[win])
            .unwrap();
        conn.sync().unwrap();
        let mut target = resolve(&CaptureSource::Window {
            app: "spread".into(),
            title: String::new(),
        })
        .expect("resolved by partial app name");
        let img = target.capture().unwrap();
        conn.delete_property(root, list).unwrap();
        conn.destroy_window(win).unwrap();
        conn.sync().unwrap();
        assert_eq!(img.dimensions(), (320, 200));
        assert_eq!(
            img.get_pixel(5, 5).0,
            [255, 255, 255, 255],
            "white background"
        );
    }
}
