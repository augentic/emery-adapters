//! Drawing facts as SVG: byte-stable against the goldens under `tests/golden/`.
//!
//! `UPDATE_GOLDEN=1 cargo nextest run -p web-rendered-layout` rewrites them;
//! review the diff before committing.

mod support;

use std::path::Path;

use support::{BLUE, Card, RED, facts};
use web_rendered_layout::{REVIEW_WIDTHS, Width, draw, overlay, pair};

fn golden(name: &str, drawn: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden").join(name);
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(&path, drawn).expect("the golden is writable");
    }
    let held = std::fs::read_to_string(&path).expect("the golden exists");
    assert_eq!(drawn, held, "{name} differs from its golden");
}

#[test]
fn draw_golden() {
    let page = facts(&[RED, BLUE]);
    for width in REVIEW_WIDTHS {
        golden(&format!("cards-{}.svg", width.0), &draw(&page, width));
    }
}

#[test]
fn overlay_golden() {
    let original = facts(&[RED, BLUE]);
    let moved = facts(&[RED, Card { y: 300.0, ..BLUE }]);
    golden("cards-moved-360.svg", &overlay(&original, &moved, Width(360)));
}

#[test]
fn pair_golden() {
    let before = facts(&[RED, BLUE]);
    let after = facts(&[RED, Card { height: 30.0, ..BLUE }]);
    golden("cards-pair-360.svg", &pair(&before, &after, Width(360)));
}

#[test]
fn draw_escapes_text() {
    let drawn = draw(&facts(&[RED]), Width(360));

    assert!(drawn.contains(">Fresh &lt;eggs&gt; &amp; milk</text>"), "{drawn}");
}
