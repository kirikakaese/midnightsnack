// SPDX-License-Identifier: GPL-3.0-or-later
//! Minimal PDF writer used by tests, the development server and E2E fixtures.

use std::fmt::Write as _;

/// One page: size in points, RGB fill (0–1) covering the page, optional sticky-note text.
pub struct TestPage {
    pub width: f32,
    pub height: f32,
    pub rgb: [f32; 3],
    pub note: Option<String>,
}

impl TestPage {
    pub fn landscape(rgb: [f32; 3]) -> Self {
        TestPage {
            width: 800.0,
            height: 450.0,
            rgb,
            note: None,
        }
    }

    pub fn with_note(mut self, note: &str) -> Self {
        self.note = Some(note.to_owned());
        self
    }
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
}

/// Builds a valid PDF with a solid-colored rectangle and a page number on every page.
pub fn build_pdf(pages: &[TestPage]) -> Vec<u8> {
    let mut objects: Vec<String> = Vec::new();
    // 1: catalog, 2: pages, 3: font, then per page: page, content, [annotation]
    let mut kids = Vec::new();
    let mut next = 4;
    let mut page_objs = Vec::new();
    for (i, p) in pages.iter().enumerate() {
        let page_id = next;
        let content_id = next + 1;
        let annot_id = p.note.as_ref().map(|_| next + 2);
        next += if annot_id.is_some() { 3 } else { 2 };
        kids.push(format!("{page_id} 0 R"));
        let stream = format!(
            "{} {} {} rg 0 0 {} {} re f 0 0 0 rg BT /F1 48 Tf 40 40 Td ({}) Tj ET",
            p.rgb[0],
            p.rgb[1],
            p.rgb[2],
            p.width,
            p.height,
            i + 1
        );
        let annots = annot_id
            .map(|a| format!(" /Annots [{a} 0 R]"))
            .unwrap_or_default();
        page_objs.push((
            page_id,
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Contents {content_id} 0 R \
                 /Resources << /Font << /F1 3 0 R >> >>{annots} >>",
                p.width, p.height
            ),
        ));
        page_objs.push((
            content_id,
            format!(
                "<< /Length {} >>\nstream\n{stream}\nendstream",
                stream.len()
            ),
        ));
        if let (Some(a), Some(note)) = (annot_id, &p.note) {
            page_objs.push((
                a,
                format!(
                    "<< /Type /Annot /Subtype /Text /Rect [10 10 30 30] /Contents ({}) >>",
                    escape(note)
                ),
            ));
        }
    }
    objects.push("<< /Type /Catalog /Pages 2 0 R >>".into());
    objects.push(format!(
        "<< /Type /Pages /Kids [{}] /Count {} >>",
        kids.join(" "),
        pages.len()
    ));
    objects.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into());
    page_objs.sort_by_key(|(id, _)| *id);
    objects.extend(page_objs.into_iter().map(|(_, o)| o));

    let mut out = String::from("%PDF-1.4\n");
    let mut offsets = Vec::new();
    for (i, o) in objects.iter().enumerate() {
        offsets.push(out.len());
        let _ = write!(out, "{} 0 obj\n{o}\nendobj\n", i + 1);
    }
    let xref = out.len();
    let _ = write!(out, "xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1);
    for off in offsets {
        let _ = writeln!(out, "{off:010} 00000 n ");
    }
    let _ = write!(
        out,
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
        objects.len() + 1
    );
    out.into_bytes()
}
