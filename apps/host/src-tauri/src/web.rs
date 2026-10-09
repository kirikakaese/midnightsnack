// SPDX-License-Identifier: GPL-3.0-or-later
//! Web page cues: isolated child webviews inside output windows.
//!
//! For every open program output the manager keeps a webview for the web cue it shows (visible)
//! and for the next cue if that is a web page (loaded hidden, so it is ready when it goes live).
//! Pages run with remote content only: they get no IPC access to the host.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use midnightsnack_core::{CueContent, Engine};
use midnightsnack_protocol::{OutputFeed, Position, WebInfo};
use midnightsnack_server::{state::lock, AppState, Event};
use tauri::webview::WebviewBuilder;
use tauri::{AppHandle, LogicalPosition, Manager, Url, Webview, WebviewUrl};

use crate::output;

#[derive(Debug, Clone, PartialEq)]
struct Wanted {
    output_id: String,
    cue_id: String,
    web: WebInfo,
    visible: bool,
}

fn webview_label(output_id: &str, cue_id: &str) -> String {
    format!("web-{output_id}-{cue_id}")
}

fn web_of(engine: &Engine, pos: Option<&Position>) -> Option<(String, WebInfo)> {
    let cue = engine.show().cue(&pos?.cue_id)?;
    match &cue.content {
        CueContent::Web { web } => Some((cue.id.clone(), web.clone())),
        _ => None,
    }
}

/// What should exist right now, per the engine.
fn wanted(engine: &Engine, now_ms: i64) -> Vec<Wanted> {
    let live = engine.live_state(now_ms);
    // Masters and test patterns are drawn by the page underneath; web views must not cover them.
    let covered = live.masters.blackout || live.masters.logo || live.test_pattern.is_some();
    let next = web_of(engine, live.next.as_ref());
    let mut out = Vec::new();
    for o in engine
        .show()
        .outputs
        .iter()
        .filter(|o| o.feed == OutputFeed::Program)
    {
        let current = web_of(engine, engine.output_position(&o.id));
        if let Some((cue_id, web)) = &current {
            out.push(Wanted {
                output_id: o.id.clone(),
                cue_id: cue_id.clone(),
                web: web.clone(),
                visible: !covered,
            });
        }
        if let Some((cue_id, web)) = &next {
            let targeted = engine
                .show()
                .cue(cue_id)
                .is_some_and(|c| c.targets.as_ref().is_none_or(|t| t.contains(&o.id)));
            let already = current.as_ref().is_some_and(|(id, _)| id == cue_id);
            if targeted && !already {
                out.push(Wanted {
                    output_id: o.id.clone(),
                    cue_id: cue_id.clone(),
                    web: web.clone(),
                    visible: false,
                });
            }
        }
    }
    out
}

fn same_origin(a: &Url, b: &Url) -> bool {
    a.scheme() == b.scheme()
        && a.host_str() == b.host_str()
        && a.port_or_known_default() == b.port_or_known_default()
}

fn create(app: &AppHandle, w: &Wanted, data_root: &Path) -> Option<Webview> {
    let window = output::window(app, &w.output_id)?;
    let url = Url::parse(&w.web.url).ok()?;
    let origin = url.clone();
    let block = w.web.block_navigation;
    let mut builder = WebviewBuilder::new(
        webview_label(&w.output_id, &w.cue_id),
        WebviewUrl::External(url),
    )
    .auto_resize()
    .on_navigation(move |to| {
        let allowed = !block || same_origin(&origin, to) || to.scheme() == "about";
        if !allowed {
            tracing::info!(%to, "blocked navigation away from web cue");
        }
        allowed
    });
    builder = if w.web.persist_session {
        builder.data_directory(data_root.join(&w.cue_id))
    } else {
        builder.incognito(true)
    };
    let size = window.inner_size().ok()?;
    match window.add_child(builder, LogicalPosition::new(0.0, 0.0), size) {
        Ok(view) => {
            let _ = view.set_zoom(w.web.zoom as f64 / 100.0);
            if !w.visible {
                let _ = view.hide();
            }
            Some(view)
        }
        Err(e) => {
            tracing::warn!(error = %e, url = %w.web.url, "could not create web view");
            None
        }
    }
}

/// Synthesizes an arrow key in the page (reveal.js and similar listen on `document`).
fn forward_key(view: &Webview, forward: bool) {
    let (key, code) = if forward {
        ("ArrowRight", 39)
    } else {
        ("ArrowLeft", 37)
    };
    let js = format!(
        "document.dispatchEvent(new KeyboardEvent('keydown', {{key: '{key}', code: '{key}', keyCode: {code}, which: {code}, bubbles: true, cancelable: true}}));"
    );
    let _ = view.eval(js);
}

/// Runs for the lifetime of the app.
pub async fn run(app: AppHandle, state: Arc<AppState>, data_root: PathBuf) {
    let mut events = state.events.subscribe();
    // label -> (info it was created with)
    let mut views: HashMap<String, WebInfo> = HashMap::new();
    let mut last_nav = 0u64;
    loop {
        let (want, nav) = {
            let engine = lock(&state.engine);
            let now = midnightsnack_core::now_ms();
            (wanted(&engine, now), engine.live_state(now).web_nav)
        };
        let app2 = app.clone();
        let data_root2 = data_root.clone();
        let want2 = want.clone();
        let mut views2 = std::mem::take(&mut views);
        let nav2 = nav.clone();
        let last = last_nav;
        // Webview operations belong on the main thread.
        let (tx, rx) = tokio::sync::oneshot::channel();
        let _ = app.run_on_main_thread(move || {
            let keep: HashSet<String> = want2
                .iter()
                .map(|w| webview_label(&w.output_id, &w.cue_id))
                .collect();
            for (label, _) in views2.clone() {
                if !keep.contains(&label) {
                    if let Some(v) = app2.get_webview(&label) {
                        let _ = v.close();
                    }
                    views2.remove(&label);
                }
            }
            for w in &want2 {
                let label = webview_label(&w.output_id, &w.cue_id);
                let existing = app2.get_webview(&label);
                // Settings changed: rebuild the view.
                let stale = views2.get(&label).is_some_and(|info| {
                    info.url != w.web.url
                        || info.persist_session != w.web.persist_session
                        || info.block_navigation != w.web.block_navigation
                });
                let view = match existing {
                    Some(v) if !stale => Some(v),
                    other => {
                        if let Some(v) = other {
                            let _ = v.close();
                        }
                        create(&app2, w, &data_root2)
                    }
                };
                let Some(view) = view else { continue };
                let _ = view.set_zoom(w.web.zoom as f64 / 100.0);
                let _ = if w.visible { view.show() } else { view.hide() };
                views2.insert(label, w.web.clone());
            }
            if let Some(nav) = &nav2 {
                if nav.seq > last {
                    for w in want2
                        .iter()
                        .filter(|w| w.visible && w.cue_id == nav.cue_id && w.web.forward_keys)
                    {
                        if let Some(v) = app2.get_webview(&webview_label(&w.output_id, &w.cue_id)) {
                            forward_key(&v, nav.forward);
                        }
                    }
                }
            }
            let _ = tx.send(views2);
        });
        views = rx.await.unwrap_or_default();
        if let Some(n) = nav {
            last_nav = n.seq;
        }
        // Wait for the next relevant change (or an output window opening/closing).
        loop {
            tokio::select! {
                ev = events.recv() => match ev {
                    Ok(Event::Live | Event::Show) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => break,
                    Ok(_) => continue,
                    Err(_) => return,
                },
                _ = WINDOWS_CHANGED.notified() => break,
            }
        }
    }
}

/// Signalled when output windows open or close, so web views are re-created in them.
pub static WINDOWS_CHANGED: tokio::sync::Notify = tokio::sync::Notify::const_new();
