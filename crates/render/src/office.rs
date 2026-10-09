// SPDX-License-Identifier: GPL-3.0-or-later
//! Office presentations: conversion to PDF and speaker notes.
//!
//! - PPTX/PPT/ODP are converted with headless LibreOffice (`soffice`), found on `PATH`, in the
//!   usual install locations, or via `MIDNIGHTSNACK_SOFFICE`.
//! - Keynote files are exported by Keynote itself through AppleScript (macOS only).
//! - Speaker notes are read from the PPTX (`ppt/notesSlides`) or ODP (`presentation:notes`).

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use quick_xml::events::Event;
use quick_xml::Reader;

pub const SOFFICE_ENV: &str = "MIDNIGHTSNACK_SOFFICE";
const CONVERT_TIMEOUT: Duration = Duration::from_secs(180);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfficeKind {
    /// PowerPoint (`pptx`, `ppt`, `pps`, `ppsx`).
    PowerPoint,
    /// OpenDocument presentation (`odp`).
    OpenDocument,
    Keynote,
}

impl OfficeKind {
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        match ext.as_str() {
            "pptx" | "ppt" | "pps" | "ppsx" => Some(OfficeKind::PowerPoint),
            "odp" => Some(OfficeKind::OpenDocument),
            "key" => Some(OfficeKind::Keynote),
            _ => None,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum OfficeError {
    /// No converter is installed for this kind of file.
    #[error("no converter available")]
    ConverterMissing,
    #[error("conversion failed: {0}")]
    Failed(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Location of LibreOffice's `soffice`, if installed.
pub fn find_soffice() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os(SOFFICE_ENV)
        .map(PathBuf::from)
        .filter(|p| p.is_file())
    {
        return Some(p);
    }
    let names: &[&str] = if cfg!(windows) {
        &["soffice.exe"]
    } else {
        &["soffice", "libreoffice"]
    };
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            for n in names {
                let p = dir.join(n);
                if p.is_file() {
                    return Some(p);
                }
            }
        }
    }
    let known: &[&str] = if cfg!(target_os = "macos") {
        &["/Applications/LibreOffice.app/Contents/MacOS/soffice"]
    } else if cfg!(windows) {
        &[
            r"C:\Program Files\LibreOffice\program\soffice.exe",
            r"C:\Program Files (x86)\LibreOffice\program\soffice.exe",
        ]
    } else {
        &[
            "/usr/bin/soffice",
            "/usr/lib/libreoffice/program/soffice",
            "/opt/libreoffice/program/soffice",
            "/snap/bin/libreoffice",
        ]
    };
    known.iter().map(PathBuf::from).find(|p| p.is_file())
}

/// Whether a converter exists for this file on this machine.
pub fn can_convert(kind: OfficeKind) -> bool {
    match kind {
        OfficeKind::Keynote => cfg!(target_os = "macos"),
        _ => find_soffice().is_some(),
    }
}

/// Converts `src` to `dest` (a `.pdf` path). Blocking; run it off the async runtime.
pub fn convert_to_pdf(src: &Path, dest: &Path) -> Result<(), OfficeError> {
    let kind = OfficeKind::from_path(src)
        .ok_or_else(|| OfficeError::Failed("not an office file".into()))?;
    let dir = dest
        .parent()
        .ok_or_else(|| OfficeError::Failed("bad destination".into()))?;
    std::fs::create_dir_all(dir)?;
    match kind {
        OfficeKind::Keynote => convert_keynote(src, dest),
        _ => convert_soffice(src, dest),
    }
}

fn run_with_timeout(mut cmd: Command) -> Result<(), OfficeError> {
    let mut child = cmd
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            if status.success() {
                return Ok(());
            }
            let mut err = String::new();
            if let Some(mut e) = child.stderr.take() {
                let _ = e.read_to_string(&mut err);
            }
            return Err(OfficeError::Failed(format!("{status}: {}", err.trim())));
        }
        if started.elapsed() > CONVERT_TIMEOUT {
            let _ = child.kill();
            return Err(OfficeError::Failed("timed out".into()));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn convert_soffice(src: &Path, dest: &Path) -> Result<(), OfficeError> {
    let soffice = find_soffice().ok_or(OfficeError::ConverterMissing)?;
    // A private profile so a running LibreOffice does not block the conversion.
    let work = tempfile::tempdir()?;
    let profile = work.path().join("profile");
    let profile_url = format!(
        "file:///{}",
        profile
            .to_string_lossy()
            .replace('\\', "/")
            .trim_start_matches('/')
    );
    let out = work.path().join("out");
    let mut cmd = Command::new(soffice);
    cmd.arg(format!("-env:UserInstallation={profile_url}"))
        .args([
            "--headless",
            "--norestore",
            "--nolockcheck",
            "--convert-to",
            "pdf",
            "--outdir",
        ])
        .arg(&out)
        .arg(src);
    run_with_timeout(cmd)?;
    let stem = src.file_stem().unwrap_or_default();
    let produced = out.join(stem).with_extension("pdf");
    if !produced.is_file() {
        return Err(OfficeError::Failed("LibreOffice produced no PDF".into()));
    }
    std::fs::copy(&produced, dest)?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn convert_keynote(src: &Path, dest: &Path) -> Result<(), OfficeError> {
    fn quote(p: &Path) -> String {
        p.to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
    }
    let script = format!(
        r#"tell application "Keynote"
    set doc to open POSIX file "{src}"
    export doc to POSIX file "{dest}" as PDF
    close doc saving no
end tell"#,
        src = quote(src),
        dest = quote(dest)
    );
    let mut cmd = Command::new("osascript");
    cmd.args(["-e", &script]);
    run_with_timeout(cmd)?;
    if dest.is_file() {
        Ok(())
    } else {
        Err(OfficeError::Failed("Keynote produced no PDF".into()))
    }
}

#[cfg(not(target_os = "macos"))]
fn convert_keynote(_src: &Path, _dest: &Path) -> Result<(), OfficeError> {
    Err(OfficeError::ConverterMissing)
}

/// Speaker notes per slide, if the format carries them.
pub fn read_notes(path: &Path) -> Vec<String> {
    let result = match OfficeKind::from_path(path) {
        Some(OfficeKind::PowerPoint) => pptx_notes(path),
        Some(OfficeKind::OpenDocument) => odp_notes(path),
        _ => return Vec::new(),
    };
    match result {
        Ok(mut notes) => {
            while notes.last().is_some_and(String::is_empty) {
                notes.pop();
            }
            notes
        }
        Err(e) => {
            tracing::debug!(path = %path.display(), error = %e, "no notes read");
            Vec::new()
        }
    }
}

type ZipFile = zip::ZipArchive<std::io::BufReader<std::fs::File>>;

fn read_entry(zip: &mut ZipFile, name: &str) -> Option<String> {
    let mut entry = zip.by_name(name).ok()?;
    let mut s = String::new();
    entry
        .by_ref()
        .take(32 * 1024 * 1024)
        .read_to_string(&mut s)
        .ok()?;
    Some(s)
}

/// `Id -> Target` from a `.rels` file.
fn relationships(xml: &str) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    let mut reader = Reader::from_str(xml);
    loop {
        match reader.read_event() {
            Ok(Event::Empty(e) | Event::Start(e)) if e.local_name().as_ref() == "Relationship" => {
                let attr = |name: &str| {
                    e.attributes()
                        .flatten()
                        .find(|a| a.key.local_name().as_ref() == name)
                        .map(|a| a.value.to_string())
                        .unwrap_or_default()
                };
                out.push((attr("Id"), attr("Target"), attr("Type")));
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    out
}

/// Resolves a relationship target relative to the directory of the part that owns it.
fn resolve_part(base_dir: &str, target: &str) -> String {
    let mut parts: Vec<&str> = if target.starts_with('/') {
        Vec::new()
    } else {
        base_dir.split('/').filter(|p| !p.is_empty()).collect()
    };
    for seg in target.trim_start_matches('/').split('/') {
        match seg {
            ".." => {
                parts.pop();
            }
            "." | "" => {}
            s => parts.push(s),
        }
    }
    parts.join("/")
}

fn pptx_notes(path: &Path) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut zip = zip::ZipArchive::new(std::io::BufReader::new(std::fs::File::open(path)?))?;
    let presentation = read_entry(&mut zip, "ppt/presentation.xml").ok_or("no presentation.xml")?;
    let rels =
        relationships(&read_entry(&mut zip, "ppt/_rels/presentation.xml.rels").unwrap_or_default());

    // Slide order: <p:sldIdLst><p:sldId r:id="rId2"/>...
    let mut order = Vec::new();
    let mut reader = Reader::from_str(&presentation);
    loop {
        match reader.read_event() {
            Ok(Event::Empty(e) | Event::Start(e)) if e.local_name().as_ref() == "sldId" => {
                if let Some(id) = e
                    .attributes()
                    .flatten()
                    .find(|a| a.key.local_name().as_ref() == "id" && a.key.as_ref() != "id")
                {
                    order.push(id.value.to_string());
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }

    let mut notes = Vec::new();
    for rid in order {
        let Some((_, target, _)) = rels.iter().find(|(id, _, _)| *id == rid) else {
            notes.push(String::new());
            continue;
        };
        let slide = resolve_part("ppt", target);
        let (dir, file) = slide.rsplit_once('/').unwrap_or(("", &slide));
        let slide_rels =
            read_entry(&mut zip, &format!("{dir}/_rels/{file}.rels")).unwrap_or_default();
        let notes_part = relationships(&slide_rels)
            .into_iter()
            .find(|(_, _, ty)| ty.ends_with("/notesSlide"))
            .map(|(_, t, _)| resolve_part(dir, &t));
        let text = notes_part
            .and_then(|p| read_entry(&mut zip, &p))
            .map(|xml| pptx_notes_text(&xml))
            .unwrap_or_default();
        notes.push(text);
    }
    Ok(notes)
}

/// Text of a notes slide, skipping placeholders that are not the notes body (slide image,
/// slide number, header, footer, date).
fn pptx_notes_text(xml: &str) -> String {
    let mut reader = Reader::from_str(xml);
    let mut paragraphs: Vec<String> = Vec::new();
    let mut in_shape = false;
    let mut skip_shape = false;
    let mut shape_paras: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_text = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => match e.local_name().as_ref() {
                "sp" => {
                    in_shape = true;
                    skip_shape = false;
                    shape_paras.clear();
                }
                "t" if in_shape => in_text = true,
                _ => {}
            },
            Ok(Event::Empty(e)) if e.local_name().as_ref() == "ph" && in_shape => {
                let ty = e
                    .attributes()
                    .flatten()
                    .find(|a| a.key.local_name().as_ref() == "type")
                    .map(|a| a.value.to_string());
                if matches!(
                    ty.as_deref(),
                    Some("sldImg" | "sldNum" | "hdr" | "ftr" | "dt")
                ) {
                    skip_shape = true;
                }
            }
            Ok(Event::Text(t)) if in_text => {
                current.push_str(&t.xml10_content());
            }
            Ok(Event::GeneralRef(r)) if in_text => {
                let name: &str = r.as_ref();
                current.push_str(match name {
                    "amp" => "&",
                    "lt" => "<",
                    "gt" => ">",
                    "quot" => "\"",
                    "apos" => "'",
                    _ => "",
                });
            }
            Ok(Event::End(e)) => match e.local_name().as_ref() {
                "t" => in_text = false,
                "p" if in_shape => shape_paras.push(std::mem::take(&mut current)),
                "sp" => {
                    in_shape = false;
                    if !skip_shape {
                        paragraphs.append(&mut shape_paras);
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    paragraphs.join("\n").trim().to_owned()
}

fn odp_notes(path: &Path) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut zip = zip::ZipArchive::new(std::io::BufReader::new(std::fs::File::open(path)?))?;
    let content = read_entry(&mut zip, "content.xml").ok_or("no content.xml")?;
    let mut reader = Reader::from_str(&content);
    let mut notes = Vec::new();
    let (mut in_page, mut in_notes, mut in_para) = (false, false, false);
    let mut page_notes: Vec<String> = Vec::new();
    let mut current = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => match e.name().as_ref() {
                "draw:page" => {
                    in_page = true;
                    page_notes.clear();
                }
                "presentation:notes" if in_page => in_notes = true,
                "text:p" if in_notes => in_para = true,
                _ => {}
            },
            Ok(Event::Text(t)) if in_para => {
                current.push_str(&t.xml10_content());
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                "text:p" if in_para => {
                    in_para = false;
                    page_notes.push(std::mem::take(&mut current));
                }
                "presentation:notes" => in_notes = false,
                "draw:page" => {
                    in_page = false;
                    notes.push(page_notes.join("\n").trim().to_owned());
                }
                _ => {}
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    Ok(notes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    fn write_zip(path: &Path, files: &[(&str, &str)]) {
        let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for (name, body) in files {
            zip.start_file(*name, SimpleFileOptions::default()).unwrap();
            zip.write_all(body.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }

    const P: &str = r#"xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships""#;

    fn notes_xml(text: &str) -> String {
        format!(
            r#"<p:notes {P}><p:cSld><p:spTree>
<p:sp><p:nvSpPr><p:nvPr><p:ph type="sldImg"/></p:nvPr></p:nvSpPr></p:sp>
<p:sp><p:nvSpPr><p:nvPr><p:ph type="body" idx="1"/></p:nvPr></p:nvSpPr><p:txBody>{text}</p:txBody></p:sp>
<p:sp><p:nvSpPr><p:nvPr><p:ph type="sldNum"/></p:nvPr></p:nvSpPr><p:txBody><a:p><a:r><a:t>7</a:t></a:r></a:p></p:txBody></p:sp>
</p:spTree></p:cSld></p:notes>"#
        )
    }

    #[test]
    fn reads_pptx_notes_in_presentation_order() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("talk.pptx");
        let rel = |id: &str, target: &str, ty: &str| {
            format!(
                r#"<Relationship Id="{id}" Target="{target}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/{ty}"/>"#
            )
        };
        let pres_rels = format!(
            "<Relationships>{}{}</Relationships>",
            rel("rId2", "slides/slide2.xml", "slide"),
            rel("rId3", "slides/slide1.xml", "slide")
        );
        let s1_rels = format!(
            "<Relationships>{}</Relationships>",
            rel("rId1", "../notesSlides/notesSlide1.xml", "notesSlide")
        );
        let first = notes_xml("<a:p><a:r><a:t>Welcome &amp; </a:t></a:r><a:r><a:t>thanks</a:t></a:r></a:p><a:p><a:r><a:t>Second line</a:t></a:r></a:p>");
        // Slide order is rId3 (slide1, with notes) then rId2 (slide2, without).
        let presentation = format!(
            r#"<p:presentation {P}><p:sldIdLst><p:sldId id="256" r:id="rId3"/><p:sldId id="257" r:id="rId2"/></p:sldIdLst></p:presentation>"#
        );
        write_zip(
            &file,
            &[
                ("ppt/presentation.xml", &presentation),
                ("ppt/_rels/presentation.xml.rels", &pres_rels),
                ("ppt/slides/_rels/slide1.xml.rels", &s1_rels),
                ("ppt/notesSlides/notesSlide1.xml", &first),
            ],
        );
        assert_eq!(
            read_notes(&file),
            vec!["Welcome & thanks\nSecond line".to_owned()]
        );
    }

    #[test]
    fn reads_odp_notes() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("talk.odp");
        let content = r#"<office:document-content xmlns:office="o" xmlns:draw="d" xmlns:presentation="p" xmlns:text="t"><office:body><office:presentation>
<draw:page draw:name="1"><draw:frame><draw:text-box><text:p>Slide text, not a note</text:p></draw:text-box></draw:frame><presentation:notes><draw:frame><draw:text-box><text:p>Note one</text:p><text:p>line two</text:p></draw:text-box></draw:frame></presentation:notes></draw:page>
<draw:page draw:name="2"></draw:page>
<draw:page draw:name="3"><presentation:notes><draw:frame><draw:text-box><text:p>Third</text:p></draw:text-box></draw:frame></presentation:notes></draw:page>
</office:presentation></office:body></office:document-content>"#;
        write_zip(&file, &[("content.xml", content)]);
        assert_eq!(read_notes(&file), vec!["Note one\nline two", "", "Third"]);
    }

    #[test]
    fn kinds_and_paths() {
        assert_eq!(
            OfficeKind::from_path(Path::new("a.PPTX")),
            Some(OfficeKind::PowerPoint)
        );
        assert_eq!(
            OfficeKind::from_path(Path::new("a.odp")),
            Some(OfficeKind::OpenDocument)
        );
        assert_eq!(
            OfficeKind::from_path(Path::new("a.key")),
            Some(OfficeKind::Keynote)
        );
        assert_eq!(OfficeKind::from_path(Path::new("a.pdf")), None);
        assert_eq!(
            resolve_part("ppt/slides", "../notesSlides/n1.xml"),
            "ppt/notesSlides/n1.xml"
        );
        assert_eq!(resolve_part("ppt", "/ppt/slides/s.xml"), "ppt/slides/s.xml");
    }

    /// Needs LibreOffice and `MIDNIGHTSNACK_TEST_PPTX` pointing at a 3-slide deck with notes
    /// on slides 1 and 3 (run with `--ignored`).
    #[test]
    #[ignore]
    fn converts_a_real_pptx() {
        let src = PathBuf::from(std::env::var("MIDNIGHTSNACK_TEST_PPTX").expect("fixture"));
        let notes = read_notes(&src);
        assert_eq!(notes.len(), 3);
        assert!(notes[0].contains("sponsors & greet"), "{notes:?}");
        assert_eq!(notes[1], "");
        assert!(notes[2].contains("Remind about dinner"));
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("talk.pdf");
        let started = Instant::now();
        convert_to_pdf(&src, &dest).unwrap();
        eprintln!("converted in {:.1} s", started.elapsed().as_secs_f64());
        assert!(std::fs::read(&dest).unwrap().starts_with(b"%PDF"));
    }

    #[test]
    fn broken_files_have_no_notes() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("broken.pptx");
        std::fs::write(&file, b"not a zip").unwrap();
        assert!(read_notes(&file).is_empty());
    }
}
