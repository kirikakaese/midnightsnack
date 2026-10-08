// SPDX-License-Identifier: GPL-3.0-or-later
//! Headless host for developing and testing the web remote without the desktop app.
//!
//! ```sh
//! cargo run -p midnightsnack-server --bin midnightsnack-devserver -- --demo
//! ```
//!
//! Prints the operator token, PIN and join URL. Options:
//! `--port N`, `--data DIR` (persist devices/settings), `--demo` (load a generated demo show),
//! `--pin-file FILE` / `--info-file FILE` (write pairing info as JSON, used by E2E tests).

use std::path::PathBuf;

use midnightsnack_core::{Cue, CueContent, MediaRef, Show};
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
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--port" => port = args.next().and_then(|p| p.parse().ok()).unwrap_or(port),
            "--data" => data_dir = args.next().map(PathBuf::from),
            "--demo" => demo = true,
            "--info-file" => info_file = args.next().map(PathBuf::from),
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

    let pairing = handle.state.pairing_info();
    let info = serde_json::json!({
        "port": handle.addr.port(),
        "operator_token": handle.operator_token,
        "pin": pairing.pin,
        "join_url": pairing.join_urls.first(),
    });
    println!("{}", serde_json::to_string_pretty(&info).expect("json"));
    if let Some(f) = info_file {
        std::fs::write(f, info.to_string())?;
    }
    tokio::signal::ctrl_c().await?;
    let _ = std::fs::remove_dir_all(&work);
    Ok(())
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
        },
    );
    let blank = Cue::new(
        "Break",
        CueContent::Blank {
            color: "#000000".into(),
        },
    );
    Ok(Show {
        title: "Demo show".into(),
        cues: vec![deck, talk, blank],
        ..Show::default()
    })
}
