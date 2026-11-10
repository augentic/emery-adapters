//! What an image is apart from the URL it was served under: a digest of its
//! content, an SVG's read as markup with its whitespace and quotes evened
//! out, so one image served as a file and inlined as a `data:` URL agree.

use sha2::{Digest as _, Sha256};

use crate::capture::hex;

/// The identity of the image `body`.
pub fn identity(body: &[u8]) -> String {
    let digest = svg_markup(body)
        .map_or_else(|| Sha256::digest(body), |markup| Sha256::digest(evened(markup)));
    hex(&digest[..8])
}

fn svg_markup(body: &[u8]) -> Option<&str> {
    let text = std::str::from_utf8(body).ok()?;
    let text = text.trim_start_matches('\u{feff}').trim_start();
    (text.starts_with('<') && text.contains("<svg")).then_some(text)
}

// The markup with each run of whitespace one space, none between two tags,
// and every double quote single.
fn evened(markup: &str) -> String {
    let mut out = String::with_capacity(markup.len());
    let mut space = false;
    for c in markup.trim().chars() {
        if c.is_whitespace() {
            space = true;
            continue;
        }
        if space && !(c == '<' && out.ends_with('>')) {
            out.push(' ');
        }
        space = false;
        out.push(if c == '"' { '\'' } else { c });
    }
    out
}
