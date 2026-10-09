// SPDX-License-Identifier: GPL-3.0-or-later
//! Headless host for developing and testing the web remote without the desktop app.
//!
//! ```sh
//! cargo run -p midnightsnack-server --bin midnightsnack-devserver -- --demo
//! ```
//!
//! Prints the operator token, PIN and join URL. Options:
//! `--port N`, `--data DIR` (persist devices/settings), `--demo` (load a generated demo show),
//! `--info-file FILE` (write pairing info as JSON, used by E2E tests), `--relay URL` (connect to
//! a relay and wait for it), `--https` (serve HTTPS too).

use std::path::PathBuf;

use midnightsnack_core::model::split_text;
use midnightsnack_core::{Cue, CueContent, MediaRef, Origin, Show};
use midnightsnack_protocol::{
    Action, JoinKind, Overlay, OverlayKind, OverlayPosition, RelayState, TimerCue, TimerMode,
};
use midnightsnack_render::test_support::{build_pdf, TestPage};
use midnightsnack_server::{start, state::lock, ServerConfig};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let mut port = midnightsnack_protocol::DEFAULT_PORT;
    let mut data_dir = None;
    let mut demo = false;
    let mut info_file = None;
    let mut relay = None;
    let mut https = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--port" => port = args.next().and_then(|p| p.parse().ok()).unwrap_or(port),
            "--data" => data_dir = args.next().map(PathBuf::from),
            "--demo" => demo = true,
            "--info-file" => info_file = args.next().map(PathBuf::from),
            "--relay" => relay = args.next(),
            "--https" => https = true,
            other => {
                eprintln!("unknown argument {other}");
                std::process::exit(2);
            }
        }
    }

    let work = std::env::temp_dir().join(format!("midnightsnack-dev-{}", std::process::id()));
    std::fs::create_dir_all(&work)?;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut config = ServerConfig::new(work.join("cache"));
    config.bind.set_port(port);
    config.fallback_to_free_port = false;
    config.data_dir = data_dir;
    config.mdns = false;
    config.restore_autosave = false;
    config.pdfium_dirs = vec![root.join("apps/host/src-tauri/resources/pdfium")];

    let handle = start(config).await?;
    if demo {
        let show = demo_show(&work)?;
        let c = lock(&handle.state.engine).replace_show(show, None);
        handle.state.after_change(c);
    }

    if https {
        let _ = handle
            .state
            .perform(Origin::LOCAL_ADMIN, Action::SetHttps { on: true })
            .await;
        wait_for(|| lock(&handle.state.https_runtime).0.is_some()).await;
    }
    if let Some(url) = relay {
        let action = Action::ConfigureRelay {
            enabled: true,
            url,
            access_token: std::env::var("MIDNIGHTSNACK_RELAY_TOKEN").ok(),
        };
        if let Err(e) = handle.state.perform(Origin::LOCAL_ADMIN, action).await {
            eprintln!("invalid relay URL: {e:?}");
            std::process::exit(2);
        }
        wait_for(|| lock(&handle.state.relay_runtime).state == RelayState::Connected).await;
    }

    let pairing = handle.state.pairing_info();
    let link = |kind: JoinKind| {
        pairing
            .links
            .iter()
            .find(|l| l.kind == kind)
            .map(|l| l.url.clone())
    };
    let info = serde_json::json!({
        "port": handle.addr.port(),
        "operator_token": handle.operator_token,
        "pin": pairing.pin,
        "join_url": link(JoinKind::Lan),
        "https_join_url": link(JoinKind::Https),
        "relay_join_url": link(JoinKind::Relay),
    });
    println!("{}", serde_json::to_string_pretty(&info).expect("json"));
    if let Some(f) = info_file {
        std::fs::write(f, info.to_string())?;
    }
    tokio::signal::ctrl_c().await?;
    let _ = std::fs::remove_dir_all(&work);
    Ok(())
}

/// Waits up to 15 s for a condition (relay connected, HTTPS listening).
async fn wait_for(cond: impl Fn() -> bool) {
    for _ in 0..150 {
        if cond() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    eprintln!("gave up waiting; continuing without it");
}

fn demo_show(dir: &std::path::Path) -> std::io::Result<Show> {
    let pdf = dir.join("demo.pdf");
    let colors = [
        [0.12, 0.2, 0.55],
        [0.55, 0.12, 0.2],
        [0.1, 0.45, 0.25],
        [0.5, 0.4, 0.1],
    ];
    let pages: Vec<TestPage> = colors
        .iter()
        .enumerate()
        .map(|(i, c)| {
            TestPage::landscape(*c).with_note(&format!("Speaker notes for slide {}", i + 1))
        })
        .collect();
    std::fs::write(&pdf, build_pdf(&pages))?;
    let mut deck = Cue::new(
        "Welcome deck",
        CueContent::Pdf {
            file: MediaRef::linked(&pdf),
            page_count: pages.len() as u32,
            source: None,
        },
    );
    deck.slide_notes = (1..=pages.len())
        .map(|i| format!("Speaker notes for slide {i}"))
        .collect();
    deck.color = Some("#4f8cff".into());
    let second = dir.join("second.pdf");
    std::fs::write(
        &second,
        build_pdf(&[
            TestPage::landscape([0.3, 0.3, 0.3]),
            TestPage::landscape([0.6, 0.6, 0.6]),
        ]),
    )?;
    let talk = Cue::new(
        "Second talk",
        CueContent::Pdf {
            file: MediaRef::linked(&second),
            page_count: 2,
            source: None,
        },
    );
    let lyrics = "Oh midnight snack, so sweet and true\nWe gather round to dine with you\n\n\
                  The night is young, the plates are warm\nWe weather every cheese-less storm";
    let song = Cue::new(
        "Anthem",
        CueContent::Text {
            source: lyrics.into(),
            lyrics: true,
            slides: split_text(lyrics, true),
            theme: None,
        },
    );
    let timer = Cue::new(
        "Break timer",
        CueContent::Timer {
            timer: TimerCue {
                mode: TimerMode::Countdown {
                    duration_ms: 10 * 60 * 1000,
                },
                label: "We continue in".into(),
                overtime_color: "#ef4444".into(),
                theme: None,
            },
        },
    );
    let blank = Cue::new(
        "Break",
        CueContent::Blank {
            color: "#000000".into(),
        },
    );
    let overlays = vec![
        Overlay {
            id: "host".into(),
            name: "Host".into(),
            kind: OverlayKind::LowerThird {
                title: "Ada Lovelace".into(),
                subtitle: "Host".into(),
            },
            position: OverlayPosition::BottomLeft,
            color: "#ffd447".into(),
            background: "#0b0d12".into(),
            scale: 100,
        },
        Overlay {
            id: "clock".into(),
            name: "Clock".into(),
            kind: OverlayKind::Clock { seconds: false },
            position: OverlayPosition::TopRight,
            color: "#ffffff".into(),
            background: "#1c2230".into(),
            scale: 100,
        },
    ];
    Ok(Show {
        title: "Demo show".into(),
        cues: vec![deck, talk, song, timer, blank],
        overlays,
        ..Show::default()
    })
}
