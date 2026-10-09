// SPDX-License-Identifier: GPL-3.0-or-later
//! OpenSlides stores motion and topic texts as HTML from its editor. midnightsnack never puts
//! that HTML into a page: it is reduced to plain text blocks (paragraphs, headings, list items).

use midnightsnack_protocol::{OsBlock, OsBlockKind};

/// Longest text kept per block, and blocks per document (protects the slides and the wire).
const MAX_BLOCK_CHARS: usize = 4000;
const MAX_BLOCKS: usize = 400;

/// Converts HTML into text blocks. Unknown tags are dropped, their text kept; `<script>` and
/// `<style>` contents are dropped; entities are decoded; whitespace is collapsed.
pub fn blocks(html: &str) -> Vec<OsBlock> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut kind = OsBlockKind::Paragraph;
    let mut skip_depth = 0usize;
    let mut list_depth = 0usize;
    let mut rest = html;

    let flush = |out: &mut Vec<OsBlock>, current: &mut String, kind: OsBlockKind| {
        let text = collapse(current);
        current.clear();
        if !text.is_empty() && out.len() < MAX_BLOCKS {
            out.push(OsBlock {
                kind,
                text: text.chars().take(MAX_BLOCK_CHARS).collect(),
            });
        }
    };

    while !rest.is_empty() {
        let Some(lt) = rest.find('<') else {
            if skip_depth == 0 {
                current.push_str(&decode_entities(rest));
            }
            break;
        };
        if skip_depth == 0 {
            current.push_str(&decode_entities(&rest[..lt]));
        }
        rest = &rest[lt..];
        // Comments.
        if let Some(after) = rest.strip_prefix("<!--") {
            rest = after.find("-->").map_or("", |i| &after[i + 3..]);
            continue;
        }
        let Some(gt) = rest.find('>') else { break };
        let tag = &rest[1..gt];
        rest = &rest[gt + 1..];
        let closing = tag.starts_with('/');
        let name: String = tag
            .trim_start_matches('/')
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_lowercase();
        match name.as_str() {
            "script" | "style" | "template" => {
                if closing {
                    skip_depth = skip_depth.saturating_sub(1);
                } else if !tag.ends_with('/') {
                    skip_depth += 1;
                }
            }
            "br" => current.push(LINE_BREAK),
            "p" | "div" | "blockquote" | "pre" | "table" | "tr" => {
                flush(&mut out, &mut current, kind);
                kind = if list_depth > 0 {
                    OsBlockKind::ListItem
                } else {
                    OsBlockKind::Paragraph
                };
            }
            "td" | "th" if closing => current.push(' '),
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                flush(&mut out, &mut current, kind);
                kind = if closing {
                    OsBlockKind::Paragraph
                } else {
                    OsBlockKind::Heading
                };
            }
            "ul" | "ol" => {
                flush(&mut out, &mut current, kind);
                if closing {
                    list_depth = list_depth.saturating_sub(1);
                } else {
                    list_depth += 1;
                }
                kind = OsBlockKind::Paragraph;
            }
            "li" => {
                flush(&mut out, &mut current, kind);
                kind = if closing {
                    OsBlockKind::Paragraph
                } else {
                    OsBlockKind::ListItem
                };
            }
            _ => {}
        }
    }
    flush(&mut out, &mut current, kind);
    out
}

/// Marks `<br>` while collecting text (source newlines are just whitespace).
const LINE_BREAK: char = '\u{2028}';

/// Collapses runs of whitespace (keeping explicit line breaks) and trims.
fn collapse(s: &str) -> String {
    let lines: Vec<String> = s
        .split(LINE_BREAK)
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect();
    lines.join("\n").trim_matches(['\n', ' ']).to_owned()
}

fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_owned();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        let end = rest.find(';').filter(|&i| i <= 10);
        let decoded = end.and_then(|i| entity(&rest[1..i]));
        match (end, decoded) {
            (Some(i), Some(c)) => {
                out.push(c);
                rest = &rest[i + 1..];
            }
            _ => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn entity(name: &str) -> Option<char> {
    if let Some(num) = name.strip_prefix('#') {
        let code = match num.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, 16).ok()?,
            None => num.parse().ok()?,
        };
        return char::from_u32(code).filter(|c| !c.is_control() || *c == '\n');
    }
    Some(match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => ' ',
        "shy" => '\u{ad}',
        "ndash" => '–',
        "mdash" => '—',
        "hellip" => '…',
        "laquo" => '«',
        "raquo" => '»',
        "bdquo" => '„',
        "ldquo" => '“',
        "rdquo" => '”',
        "lsquo" => '‘',
        "rsquo" => '’',
        "sbquo" => '‚',
        "euro" => '€',
        "auml" => 'ä',
        "ouml" => 'ö',
        "uuml" => 'ü',
        "Auml" => 'Ä',
        "Ouml" => 'Ö',
        "Uuml" => 'Ü',
        "szlig" => 'ß',
        "eacute" => 'é',
        "egrave" => 'è',
        "agrave" => 'à',
        "copy" => '©',
        "sect" => '§',
        "deg" => '°',
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds_and_text(b: &[OsBlock]) -> Vec<(OsBlockKind, &str)> {
        b.iter().map(|b| (b.kind, b.text.as_str())).collect()
    }

    #[test]
    fn editor_html_becomes_blocks() {
        let html =
            "<h2>Background</h2><p>The <strong>assembly</strong> &amp; its\n   committees</p>\
                    <ul><li>first&nbsp;point</li><li><p>second</p></li></ul><p>line<br>break</p>";
        assert_eq!(
            kinds_and_text(&blocks(html)),
            vec![
                (OsBlockKind::Heading, "Background"),
                (OsBlockKind::Paragraph, "The assembly & its committees"),
                (OsBlockKind::ListItem, "first point"),
                (OsBlockKind::ListItem, "second"),
                (OsBlockKind::Paragraph, "line\nbreak"),
            ]
        );
    }

    #[test]
    fn scripts_styles_and_comments_are_dropped() {
        let html = "<p>a<script>alert('x')</script>b</p><style>p{}</style><!-- hidden -->\
                    <img src=x onerror=alert(1)>c";
        let text: Vec<String> = blocks(html).into_iter().map(|b| b.text).collect();
        assert_eq!(text, vec!["ab", "c"]);
    }

    #[test]
    fn entities_and_plain_text() {
        assert_eq!(
            blocks("Fu&szlig;ball &#8364; &#x41; &bogus; &")[0].text,
            "Fußball € A &bogus; &"
        );
        assert!(blocks("").is_empty());
        assert!(blocks("<p> </p><p>\n</p>").is_empty());
        assert_eq!(blocks("<p>unclosed <b").len(), 1);
    }
}
