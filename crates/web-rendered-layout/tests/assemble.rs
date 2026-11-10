//! Building facts from a raw browser capture: a two-element page sampled at three widths.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::json;
use web_rendered_layout::{Facts, ItemKind, Viewport, Width, assemble, items};

// A capture where the `div` is half the viewport wide at 320, 880, and 1440,
// unless `kink` moves it at 880, and holds one line of text that never moves.
fn capture(name: &str, kink: f64) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    fs::create_dir_all(dir.join("w")).expect("a scratch dir");
    fs::create_dir_all(dir.join("assets")).expect("a scratch dir");
    let meta = json!({
        "elements": [
            { "p": "html", "tag": "html", "kind": "box", "parent": -1, "seq": [1] },
            { "p": "html > div", "tag": "div", "kind": "box", "parent": 0, "seq": ["t0"] },
        ],
        "texts": [{ "p": "html > div > #text", "text": "Hello", "parent": 1 }],
        "range": [320, 1440],
        "height": 800,
        "heights": { "range": [400, 1440], "sweeps": {} },
        "ends": [320, 880, 1440],
        "fonts": {},
        "assets": {},
        "checks": [{ "kind": "line", "ok": true }],
    });
    fs::write(dir.join("meta.json"), meta.to_string()).expect("a scratch file");
    for width in [320.0_f64, 880.0, 1440.0] {
        let y = if (width - 880.0).abs() < 1.0 { 20.0 + kink } else { 20.0 };
        let mut sample = json!({
            "b": [0, 0, width, 200, 1, 10, y, width / 2.0, 40, 1],
            "l": [[1, 18, 28, 40, 20]],
            "s": {}, "m": {}, "a": {}, "ps": {}, "ax": "ax/0.json",
        });
        if width < 321.0 {
            sample["s"] = json!({
                "0": { "background-color": "rgba(0, 0, 0, 0)", "opacity": "1", "box-shadow": "none", "background-image": "none" },
                "1": { "background-color": "rgb(255, 0, 0)", "opacity": "1", "box-shadow": "none", "background-image": "none" },
            });
        }
        fs::write(dir.join("w").join(format!("{width}.json")), sample.to_string())
            .expect("a scratch file");
    }
    fs::write(dir.join("assets/a.woff2"), [0_u8, 1, 2]).expect("a scratch file");
    dir
}

#[test]
fn assemble_fits_one_line() {
    let folder = assemble(&capture("straight", 0.0)).expect("the capture assembles");
    let nodes = folder.json("parts/n0.json").expect("one part");

    assert_eq!(
        nodes["n1"]["box"],
        json!([[320.0, 1440.0, 0.0, 0.0, 0.5, 0.0, 10.0, 20.0, 0.0, 40.0, 1]])
    );
    assert_eq!(nodes["n1"]["children"], json!(["t0"]));
    assert_eq!(nodes["t0"]["lines"][0]["lines"], 1);
    assert_eq!(folder.get("assets/a.woff2"), Some(&[0, 1, 2][..]));
    let index = folder.json("index.json").expect("an index");
    assert_eq!(index["version"], 2);
    assert_eq!(index["search"]["checks"], json!({ "line": { "total": 1, "failed": 0 } }));
    assert_eq!(
        index["counts"],
        json!({ "elements": 2, "elements_hidden_at_widest": 0, "texts": 1 })
    );

    let facts = Facts::read(&folder).expect("assembled facts read");
    let shown = items(&facts, Viewport::at(Width(600)));
    let paint = shown.iter().find(|item| item.kind == ItemKind::Paint).expect("the div paints");
    assert_eq!(paint.bounds, [10.0, 20.0, 300.0, 40.0]);
    assert!(shown.iter().any(|item| item.kind == ItemKind::Text && item.label == "Hello"));
}

#[test]
fn assemble_splits_at_a_kink() {
    let folder = assemble(&capture("kinked", 30.0)).expect("the capture assembles");
    let nodes = folder.json("parts/n0.json").expect("one part");
    let pieces: Vec<(f64, f64)> = nodes["n1"]["box"]
        .as_array()
        .expect("box pieces")
        .iter()
        .map(|piece| (piece[0].as_f64().unwrap_or(0.0), piece[1].as_f64().unwrap_or(0.0)))
        .collect();

    assert_eq!(pieces, [(320.0, 880.0), (880.0, 1440.0)]);
}

#[test]
fn assemble_refuses_a_missing_sample() {
    let dir = capture("missing", 0.0);
    fs::remove_file(dir.join("w/880.json")).expect("the sample exists");
    let refused = assemble(&dir).expect_err("a missing sample is refused");

    assert!(refused.to_string().contains("880.json"), "{refused}");
}
