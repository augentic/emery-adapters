//! Small hand-built facts: a page of coloured cards, each holding one line of text.

#![allow(dead_code, reason = "each suite uses its own share")]

use serde_json::{Map, Value, json};
use web_rendered_layout::{Facts, FactsFolder};

/// One card: its left and top, its width as a share of the viewport, its height, its colour, its text.
#[derive(Clone, Copy)]
pub struct Card {
    pub x: f64,
    pub y: f64,
    pub share: f64,
    pub height: f64,
    pub colour: &'static str,
    pub text: &'static str,
}

pub const RED: Card = Card {
    x: 16.0,
    y: 16.0,
    share: 0.5,
    height: 60.0,
    colour: "rgb(200, 0, 0)",
    text: "Fresh <eggs> & milk",
};
pub const BLUE: Card = Card {
    x: 16.0,
    y: 100.0,
    share: 0.8,
    height: 60.0,
    colour: "rgb(0, 0, 200)",
    text: "Open daily",
};

/// The facts of `cards` over widths 320 to 1440 at a height of 800.
pub fn page(cards: &[Card]) -> FactsFolder {
    page_over([320, 1440], cards)
}

/// The facts of `cards` over `range`.
pub fn page_over(range: [u32; 2], cards: &[Card]) -> FactsFolder {
    let mut nodes = Map::new();
    let mut children = Vec::new();
    for (k, card) in cards.iter().enumerate() {
        let (element, text) = (format!("n{}", k + 1), format!("t{k}"));
        children.push(json!(element));
        nodes.insert(
            element.clone(),
            json!({
                "tag": "div",
                "kind": "box",
                "path": format!("html > div:nth-of-type({})", k + 1),
                "parent": "n0",
                "children": [text],
                "box": [[range[0], range[1], 0, 0, card.share, 0, card.x, card.y, 0, card.height, 1]],
                "style": { "background-color": card.colour, "color": "rgb(255, 255, 255)" },
            }),
        );
        #[expect(clippy::cast_precision_loss, reason = "short texts")]
        let width = 8.0 * card.text.len() as f64;
        nodes.insert(
            text,
            json!({
                "tag": "#text",
                "kind": "text",
                "parent": element,
                "children": [],
                "text": card.text,
                "lines": [{
                    "lo": range[0], "hi": range[1], "lines": 1, "breaks": [],
                    "rects": [[[0, card.x + 8.0], [0, card.y + 8.0], [0, width], [0, 20]]],
                }],
            }),
        );
    }
    nodes.insert(
        "n0".to_owned(),
        json!({
            "tag": "html",
            "kind": "box",
            "path": "html",
            "parent": null,
            "children": children,
            "box": [[range[0], range[1], 0, 0, 1, 0, 0, 0, 0, 200, 1]],
        }),
    );
    let index = json!({
        "version": 2,
        "range": range,
        "height": 800,
        "height_range": [400, 1440],
        "root": "n0",
        "defaults": DEFAULTS.iter().map(|(k, v)| ((*k).to_owned(), json!(v))).collect::<Map<_, _>>(),
        "fonts": {},
        "faces": [],
        "assets": {},
        "containers": {},
        "parts": [{ "file": "parts/n0.json" }],
    });
    let mut folder = FactsFolder::default();
    folder.insert("index.json", line(&index)).expect("a safe path");
    folder.insert("parts/n0.json", line(&Value::Object(nodes))).expect("a safe path");
    folder
}

/// The facts of `cards`, read.
pub fn facts(cards: &[Card]) -> Facts {
    Facts::read(&page(cards)).expect("hand-built facts read")
}

const DEFAULTS: [(&str, &str); 21] = [
    ("background-color", "rgba(0, 0, 0, 0)"),
    ("background-image", "none"),
    ("box-shadow", "none"),
    ("border-top-style", "none"),
    ("border-right-style", "none"),
    ("border-bottom-style", "none"),
    ("border-left-style", "none"),
    ("border-top-width", "0px"),
    ("border-right-width", "0px"),
    ("border-bottom-width", "0px"),
    ("border-left-width", "0px"),
    ("border-top-left-radius", "0px"),
    ("visibility", "visible"),
    ("opacity", "1"),
    ("color", "rgb(0, 0, 0)"),
    ("font-family", "Arial"),
    ("font-size", "16px"),
    ("font-weight", "400"),
    ("font-style", "normal"),
    ("text-decoration-line", "none"),
    ("text-transform", "none"),
];

fn line(value: &Value) -> Vec<u8> {
    let mut bytes = value.to_string().into_bytes();
    bytes.push(b'\n');
    bytes
}
