//! An image pairs by what it shows, whatever URL it was served under: the
//! greenfield-farm hero and map, served as files in the original and
//! inlined by Vite as `data:` URLs in a render (the hero's markup evened
//! out, the map's kept whole as base64, each captured under `.svgxml`).

mod support;

use serde_json::{Value, json};
use web_rendered_layout::{Facts, ItemKind, Viewport, Width, compare, compare_at};

const HERO: &[u8] = include_bytes!("fixtures/images/hero.svg");
const HERO_INLINED: &[u8] = include_bytes!("fixtures/images/hero-inlined.svgxml");
const MAP: &[u8] = include_bytes!("fixtures/images/map.svg");
const MAP_INLINED: &[u8] = include_bytes!("fixtures/images/map-inlined.svgxml");

// A page of one `img` per `(file, body)`, stacked down the page.
fn images(shown: &[(&str, &[u8])]) -> Facts {
    let mut folder = support::page(&[]);
    let mut nodes = folder.json("parts/n0.json").expect("hand-built facts read");
    let mut children = Vec::new();
    for (k, (file, body)) in shown.iter().enumerate() {
        let id = format!("n{}", k + 1);
        #[expect(clippy::cast_precision_loss, reason = "a few images")]
        let top = 100.0f64.mul_add(k as f64, 16.0);
        nodes[&id] = json!({
            "tag": "img",
            "kind": "box",
            "path": format!("html > img:nth-of-type({})", k + 1),
            "parent": "n0",
            "children": [],
            "box": [[320, 1440, 0, 0, 0, 0, 16, top, 200, 90, 1]],
            "extra": { "src": format!("/{file}"), "file": file },
        });
        children.push(Value::String(id));
        folder.insert(file, body.to_vec()).expect("a safe path");
    }
    nodes["n0"]["children"] = Value::Array(children);
    folder.insert("parts/n0.json", nodes.to_string().into_bytes()).expect("a safe path");
    Facts::read(&folder).expect("hand-built facts read")
}

#[test]
fn inlined_images_pair_with_their_files() {
    let original =
        images(&[("assets/95b086ec6acbbe0e.svg", HERO), ("assets/602b14ca306e0e2d.svg", MAP)]);
    let render = images(&[
        ("assets/60a7022323c58859.svgxml", HERO_INLINED),
        ("assets/602b14ca306e0e2d.svgxml", MAP_INLINED),
    ]);
    let comparison = compare(&original, &render).expect("the ranges overlap");

    assert!(comparison.pass, "{:?}", comparison.worst_original_items);
    assert_eq!(comparison.unpaired_render_items, []);
}

#[test]
fn another_image_does_not_pair() {
    let original = images(&[("assets/95b086ec6acbbe0e.svg", HERO)]);
    let render = images(&[("assets/602b14ca306e0e2d.svg", MAP)]);
    let found = compare_at(&original, &render, Viewport::at(Width(800)));

    let (item, _) = &found.failed[0];
    assert_eq!(item.kind, ItemKind::Paint);
    assert_eq!(found.unpaired.len(), 1);
}
