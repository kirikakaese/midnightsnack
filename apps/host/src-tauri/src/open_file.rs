// SPDX-License-Identifier: GPL-3.0-or-later
//! Opening `.msnack` files from the file manager: as the launch argument, from a second launch
//! (forwarded to the running app by the single-instance plugin) or, on macOS, as an "open
//! documents" event. The operator window opens the file after asking about unsaved changes.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use midnightsnack_core::bundle::SHOW_FILE_EXTENSION;
use tauri::{AppHandle, Emitter, Manager, State};

/// The show waiting for the operator window.
#[derive(Default)]
pub struct PendingOpen(Mutex<Option<String>>);

/// The first show file among command line arguments (the program itself excluded).
pub fn show_in_args<I: IntoIterator<Item = String>>(args: I) -> Option<PathBuf> {
    args.into_iter()
        .skip(1)
        .map(PathBuf::from)
        .find(|p| is_show(p))
        .and_then(|p| std::fs::canonicalize(p).ok())
}

fn is_show(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(SHOW_FILE_EXTENSION))
        && path.is_file()
}

/// Remembers `path` and tells the operator window, bringing it to the front.
pub fn request(app: &AppHandle, path: PathBuf) {
    tracing::info!(path = %path.display(), "show opened from the system");
    *app.state::<PendingOpen>()
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some(path.to_string_lossy().into_owned());
    if let Some(w) = app.get_webview_window("operator") {
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
    let _ = app.emit_to("operator", "open-show-requested", ());
}

/// Called by the operator window (on start and on `open-show-requested`).
#[tauri::command]
pub fn take_pending_open(pending: State<'_, PendingOpen>) -> Option<String> {
    pending.0.lock().unwrap_or_else(|e| e.into_inner()).take()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_show_among_arguments() {
        let dir = tempfile::tempdir().unwrap();
        let show = dir.path().join("Gala.MSNACK");
        std::fs::write(&show, b"x").unwrap();
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let found = show_in_args(args(&["midnightsnack", "--flag", show.to_str().unwrap()]));
        assert_eq!(found, Some(std::fs::canonicalize(&show).unwrap()));
        // The program path itself is never taken, nor missing files or other types.
        assert_eq!(show_in_args(args(&[show.to_str().unwrap()])), None);
        assert_eq!(
            show_in_args(args(&["m", "/nope/x.msnack", "notes.txt"])),
            None
        );
    }
}
