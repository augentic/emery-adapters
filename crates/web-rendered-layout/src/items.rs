//! What a page shows at one viewport, as the comparison pairs it.
//!
//! Text is one item per run of consecutive text nodes in one element, so two
//! pages that split text into nodes differently agree; generated content
//! (`::before` and `::after` text, list markers) is text the same way. Every
//! shown element or pseudo-element that paints (background colour or image,
//! border, box shadow, an `img`, an inline `svg`) is a paint item. Wrappers
//! that paint nothing are not items, so two pages may nest differently.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use serde_json::Value;

use crate::facts::{Rect, js_number, js_round, num};
use crate::{Facts, NodeId, Viewport};

/// The two kinds of item a comparison scores apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ItemKind {
    /// A run of text, its line boxes the item's boxes.
    Text,
    /// A painted box.
    Paint,
}

impl ItemKind {
    pub(crate) const ALL: [Self; 2] = [Self::Text, Self::Paint];

    /// The kind as a score report names it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Paint => "paint",
        }
    }
}

/// One thing a page shows at a viewport.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    /// Text or paint.
    pub kind: ItemKind,
    /// The element, pseudo-element (`n4::before`), or first text node of the run.
    pub id: String,
    /// What a partner must equal: the text and its drawn font and colours, or the paint values.
    pub signature: String,
    /// The line boxes of a text item; the one box of a paint item.
    pub boxes: Vec<Rect>,
    /// The union of [`boxes`](Self::boxes).
    pub bounds: Rect,
    /// The summed area of [`boxes`](Self::boxes).
    pub area: f64,
    /// A short human label: the text's start, or the element's tag and path.
    pub label: String,
}

pub const TEXT_SEPARATOR: char = '\u{1f}';
pub const TEXT_FIELDS: [&str; 8] = [
    "text",
    "font",
    "color",
    "font-size",
    "font-weight",
    "font-style",
    "text-transform",
    "text-decoration-line",
];
pub const PAINT_FIELDS: [&str; 7] = [
    "paint",
    "background-color",
    "background-image",
    "border-width",
    "border-color",
    "border-top-left-radius",
    "box-shadow",
];
const SIDES: [&str; 4] = ["top", "right", "bottom", "left"];

/// Every item `facts` shows at `viewport`, in visual order: top, then left.
#[must_use]
pub fn items(facts: &Facts, viewport: Viewport) -> Vec<Item> {
    let mut out = Vec::new();
    for id in facts.order() {
        let node = facts.node(id);
        if node["tag"] == "#text" {
            continue;
        }
        let Some(rect) = visible(facts, node, viewport) else {
            continue;
        };
        if let Some(signature) = paint_signature(facts, node, viewport) {
            out.push(paint_item(id.as_str().to_owned(), signature, rect, label(node, id, "")));
        }

        for (kind, pseudo) in node["pseudo"].as_object().into_iter().flatten() {
            let Some(pseudo_rect) = visible(facts, pseudo, viewport) else {
                continue;
            };
            let pseudo_id = format!("{id}::{kind}");
            if let Some(signature) = paint_signature(facts, pseudo, viewport) {
                out.push(paint_item(
                    pseudo_id.clone(),
                    signature,
                    pseudo_rect,
                    label(node, id, &format!("::{kind}")),
                ));
            }
            let text = if kind == "marker" {
                let style = facts.style_at(node, "list-style-type", viewport);
                marker(&style)
            } else {
                literal(&facts.style_at(pseudo, "content", viewport))
            };
            out.extend(text_item(facts, pseudo, pseudo_id, &text, &[pseudo_rect], viewport));
        }

        // runs of consecutive text nodes
        let mut run: Vec<&Value> = Vec::new();
        let mut run_id = String::new();
        let flush = |run: &mut Vec<&Value>, run_id: &str, out: &mut Vec<Item>| {
            if run.is_empty() {
                return;
            }
            let text: String = run.iter().filter_map(|t| t["text"].as_str()).collect();
            let rects: Vec<Rect> =
                run.iter().flat_map(|t| Facts::line_rects(t, viewport)).collect();
            out.extend(text_item(facts, node, run_id.to_owned(), &text, &rects, viewport));
            run.clear();
        };
        for child in node["children"].as_array().into_iter().flatten().filter_map(Value::as_str) {
            if child.trim().is_empty() {
                continue;
            }
            match facts.node_by_key(child) {
                Some(text) if text["tag"] == "#text" => {
                    if run.is_empty() {
                        child.clone_into(&mut run_id);
                    }
                    run.push(text);
                }
                _ => flush(&mut run, &run_id, &mut out),
            }
        }
        flush(&mut run, &run_id, &mut out);
    }
    out.sort_by(|a, b| {
        js_round(a.bounds[1])
            .total_cmp(&js_round(b.bounds[1]))
            .then(js_round(a.bounds[0]).total_cmp(&js_round(b.bounds[0])))
    });
    out
}

fn label(node: &Value, id: &NodeId, suffix: &str) -> String {
    let tag = node["tag"].as_str().unwrap_or("");
    let path = node["path"].as_str().unwrap_or(id.as_str());
    format!("{tag}{suffix} {path}").chars().take(90).collect()
}

fn paint_item(id: String, signature: String, rect: Rect, label: String) -> Item {
    Item {
        kind: ItemKind::Paint,
        id,
        signature,
        boxes: vec![rect],
        bounds: rect,
        area: rect[2] * rect[3],
        label,
    }
}

fn visible(facts: &Facts, node: &Value, viewport: Viewport) -> Option<Rect> {
    let rect = Facts::box_at(node, viewport)?;
    let hidden = facts.style_at(node, "visibility", viewport) == "hidden"
        || js_to_number(&facts.style_at(node, "opacity", viewport)) == 0.0;
    (!hidden).then_some(rect)
}

pub fn paint_signature(facts: &Facts, node: &Value, viewport: Viewport) -> Option<String> {
    match node["tag"].as_str() {
        Some("img") => {
            let file = node["extra"]["file"].as_str().map_or("", basename);
            return Some(format!("img {file}"));
        }
        Some("svg") => return Some("svg".to_owned()),
        _ => {}
    }
    let style = |property: &str| facts.style_at(node, property, viewport);
    let background = style("background-color");
    let image = urls(facts, &style("background-image"));
    let shadow = style("box-shadow");
    let widths: Vec<f64> = SIDES
        .iter()
        .map(|side| {
            if style(&format!("border-{side}-style")) == "none" {
                0.0
            } else {
                parse_float(&style(&format!("border-{side}-width"))).unwrap_or(0.0)
            }
        })
        .collect();
    if transparent(&background)
        && image == "none"
        && !widths.iter().any(|&w| w > 0.0)
        && shadow == "none"
    {
        return None;
    }
    let colours: Vec<String> = SIDES
        .iter()
        .zip(&widths)
        .map(
            |(side, &width)| {
                if width > 0.0 { style(&format!("border-{side}-color")) } else { String::new() }
            },
        )
        .collect();
    let widths: Vec<String> = widths.iter().map(|&w| js_number(w)).collect();
    Some(
        [
            "box".to_owned(),
            background,
            image,
            widths.join(","),
            colours.join(","),
            style("border-top-left-radius"),
            shadow,
        ]
        .join("|"),
    )
}

// The font the browser drew `node`'s text with, from its font token's
// platform fonts: the web fonts used, else the system font of most glyphs;
// the first declared family where there is no token.
fn drawn_font(facts: &Facts, node: &Value, viewport: Viewport) -> String {
    let width = f64::from(viewport.width.0);
    let token = match &node["font"] {
        Value::String(token) => Some(token.as_str()),
        Value::Array(runs) => runs
            .iter()
            .filter_map(Value::as_array)
            .find(|run| num(&run[0]) <= width && width <= num(&run[1]))
            .and_then(|run| run[2].as_str()),
        _ => None,
    };
    let platform = token.map_or(&[][..], |token| facts.platform_fonts(token));
    if platform.is_empty() {
        return first_family(&facts.style_at(node, "font-family", viewport));
    }
    let web: BTreeSet<&str> = platform
        .iter()
        .filter(|font| font["isCustomFont"] == true)
        .filter_map(|font| font["familyName"].as_str())
        .collect();
    if !web.is_empty() {
        return web.into_iter().collect::<Vec<_>>().join(" + ");
    }
    let most = platform.iter().fold(&platform[0], |most, font| {
        if num(&font["glyphCount"]) > num(&most["glyphCount"]) { font } else { most }
    });
    format!("{} (system)", most["familyName"].as_str().unwrap_or(""))
}

fn text_signature(facts: &Facts, styled: &Value, text: &str, viewport: Viewport) -> String {
    let mut fields = vec![text.to_owned(), drawn_font(facts, styled, viewport)];
    fields
        .extend(TEXT_FIELDS[2..].iter().map(|property| facts.style_at(styled, property, viewport)));
    fields.join(&TEXT_SEPARATOR.to_string())
}

fn text_item(
    facts: &Facts, styled: &Value, id: String, raw: &str, rects: &[Rect], viewport: Viewport,
) -> Option<Item> {
    let text = collapse_whitespace(raw);
    let boxes = line_boxes(rects);
    if text.is_empty() || boxes.is_empty() {
        return None;
    }
    let bounds = union(&boxes);
    let area = boxes.iter().map(|b| b[2] * b[3]).sum();
    Some(Item {
        kind: ItemKind::Text,
        id,
        signature: text_signature(facts, styled, &text, viewport),
        label: text.chars().take(60).collect(),
        boxes,
        bounds,
        area,
    })
}

// Boxes on one line, tops within 1px, become one line box.
fn line_boxes(rects: &[Rect]) -> Vec<Rect> {
    let mut shown: Vec<Rect> = rects.iter().copied().filter(|r| r[2] > 0.0 && r[3] > 0.0).collect();
    shown.sort_by(|p, q| (p[1] - q[1]).total_cmp(&0.0).then((p[0] - q[0]).total_cmp(&0.0)));
    let mut lines: Vec<Vec<Rect>> = Vec::new();
    for rect in shown {
        match lines.iter_mut().find(|line| (line[0][1] - rect[1]).abs() < 1.0) {
            Some(line) => line.push(rect),
            None => lines.push(vec![rect]),
        }
    }
    lines.iter().map(|line| union(line)).collect()
}

pub fn union(rects: &[Rect]) -> Rect {
    let x0 = rects.iter().map(|r| r[0]).fold(f64::INFINITY, f64::min);
    let y0 = rects.iter().map(|r| r[1]).fold(f64::INFINITY, f64::min);
    let x1 = rects.iter().map(|r| r[0] + r[2]).fold(f64::NEG_INFINITY, f64::max);
    let y1 = rects.iter().map(|r| r[1] + r[3]).fold(f64::NEG_INFINITY, f64::max);
    [x0, y0, x1 - x0, y1 - y0]
}

fn marker(list_style: &str) -> String {
    match list_style {
        "disc" => "•".to_owned(),
        "circle" => "◦".to_owned(),
        "square" => "▪".to_owned(),
        other => format!("marker:{other}"),
    }
}

// The string a `content` value holds, `"…"` with its escapes undone; empty for any other value.
fn literal(content: &str) -> String {
    let Some(inner) = content.strip_prefix('"').and_then(|rest| rest.strip_suffix('"')) else {
        return String::new();
    };
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            } else {
                out.push(c);
            }
        } else {
            out.push(c);
        }
    }
    out
}

// Every `url(…)` naming a captured asset, rewritten to the asset's file name,
// so two pages serving one image under two URLs agree.
fn urls(facts: &Facts, value: &str) -> String {
    let mut out = String::new();
    let mut rest = value;
    while let Some(at) = rest.find("url(") {
        out.push_str(&rest[..at]);
        let after = &rest[at + 4..];
        let quoted = after.starts_with('"');
        let body = if quoted { &after[1..] } else { after };
        let end = body.find(['"', ')']).unwrap_or(body.len());
        let url = &body[..end];
        let tail = &body[end..];
        let closing = if tail.starts_with("\")") {
            Some(2)
        } else if tail.starts_with(')') {
            Some(1)
        } else {
            None
        };
        match closing {
            Some(close) if !url.is_empty() => {
                let whole = 4 + usize::from(quoted) + end + close;
                match facts.asset(url) {
                    Some(file) => {
                        let _ = write!(out, "url({})", basename(file));
                    }
                    None => out.push_str(&rest[at..at + whole]),
                }
                rest = &rest[at + whole..];
            }
            _ => {
                out.push_str("url(");
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

pub fn transparent(colour: &str) -> bool {
    matches!(colour, "" | "transparent" | "rgba(0, 0, 0, 0)")
}

fn first_family(families: &str) -> String {
    let first = families.split(',').next().unwrap_or("").trim();
    let first = first.strip_prefix(['"', '\'']).unwrap_or(first);
    let first = first.strip_suffix(['"', '\'']).unwrap_or(first);
    first.to_lowercase()
}

fn basename(file: &str) -> &str {
    file.rsplit('/').next().unwrap_or(file)
}

const fn is_js_space(c: char) -> bool {
    c.is_whitespace() || c == '\u{feff}'
}

fn collapse_whitespace(raw: &str) -> String {
    raw.split(is_js_space).filter(|word| !word.is_empty()).collect::<Vec<_>>().join(" ")
}

// `parseFloat`: the longest leading decimal number, after leading spaces.
pub fn parse_float(text: &str) -> Option<f64> {
    let text = text.trim_start_matches(is_js_space);
    let mut end = 0;
    let mut seen_digit = false;
    let mut seen_dot = false;
    let mut seen_exponent = false;
    let bytes = text.as_bytes();
    while end < bytes.len() {
        let b = bytes[end];
        match b {
            b'0'..=b'9' => seen_digit = true,
            b'+' | b'-' if end == 0 => {}
            b'+' | b'-' if seen_exponent && matches!(bytes[end - 1], b'e' | b'E') => {}
            b'.' if !seen_dot && !seen_exponent => seen_dot = true,
            b'e' | b'E' if seen_digit && !seen_exponent => seen_exponent = true,
            _ => break,
        }
        end += 1;
    }
    let mut candidate = &text[..end];
    while !candidate.is_empty() {
        if let Ok(value) = candidate.parse::<f64>() {
            return Some(value);
        }
        candidate = &candidate[..candidate.len() - 1];
    }
    None
}

// `Number(text)`: the whole trimmed text as a number, empty as zero, else NaN.
fn js_to_number(text: &str) -> f64 {
    let trimmed = text.trim_matches(is_js_space);
    if trimmed.is_empty() {
        return 0.0;
    }
    trimmed.parse().unwrap_or(f64::NAN)
}
