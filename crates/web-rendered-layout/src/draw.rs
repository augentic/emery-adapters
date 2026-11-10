//! Deterministic SVG images of facts, for review: the same facts always draw the same bytes.
//!
//! [`draw`] shows a page as a wireframe at one width: every painted box in
//! document order, then every text line in its colour, split where the
//! browser broke it. [`overlay`] draws the original in grey and marks in red
//! every original item that did not hold, in orange every render item paired
//! with nothing, each with why as its tooltip.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::compare::compare_at;
use crate::facts::Rect;
use crate::items::{parse_float, transparent};
use crate::{Facts, Item, ItemKind, Viewport, Width, items};

/// The widths a review draws a page at: a phone, a tablet, and a desktop.
pub const REVIEW_WIDTHS: [Width; 3] = [Width(360), Width(768), Width(1440)];

// The widest an image is shown, in CSS pixels; its view box keeps page coordinates.
const SHOWN_WIDTH: f64 = 720.0;

// The overlay's legend, beneath the page so it hides no mark.
const LEGEND_HEIGHT: f64 = 64.0;

/// The page of `facts` at `width`, as an SVG wireframe.
#[must_use]
pub fn draw(facts: &Facts, width: Width) -> String {
    let viewport = Viewport::at(width);
    let mut svg = Canvas::new(facts, viewport);
    svg.paints(facts, viewport, Shade::Colour);
    svg.texts(facts, viewport, Shade::Colour);
    svg.finish()
}

/// `original` at `width` in grey, with what [`compare_at`] found against `render` marked.
#[must_use]
pub fn overlay(original: &Facts, render: &Facts, width: Width) -> String {
    let viewport = Viewport::at(width);
    let found = compare_at(original, render, viewport);
    let mut svg = Canvas::new(original, viewport);
    svg.paints(original, viewport, Shade::Grey);
    svg.texts(original, viewport, Shade::Grey);
    for (item, why) in &found.unpaired {
        svg.mark(item, why, "#e67e00", "4 3");
    }
    for (item, why) in &found.failed {
        svg.mark(item, why, "#d40000", "");
    }
    let top = svg.height;
    svg.height += LEGEND_HEIGHT;
    let _ = write!(
        svg.body,
        r##"<g font-size="13"><rect x="0" y="{}" width="{}" height="{LEGEND_HEIGHT}" fill="#fff" stroke="#333"/><text x="8" y="{}">{width}: text {} paint {}</text><text x="8" y="{}" fill="#d40000">red: {} original items not held</text><text x="8" y="{}" fill="#e67e00">orange: {} render items unpaired</text></g>"##,
        number(top),
        number(svg.width),
        number(top + 18.0),
        found.point.text,
        found.point.paint,
        number(top + 36.0),
        found.failed.len(),
        number(top + 54.0),
        found.unpaired.len(),
    );
    svg.finish()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Shade {
    Colour,
    Grey,
}

struct Canvas {
    width: f64,
    height: f64,
    body: String,
}

impl Canvas {
    fn new(facts: &Facts, viewport: Viewport) -> Self {
        let width = f64::from(viewport.width.0);
        let page = Facts::box_at(facts.node(facts.root()), viewport).map_or(0.0, |rect| rect[3]);
        let height = page.max(facts.height()).max(1.0);
        let mut body = String::new();
        let _ = write!(
            body,
            r##"<rect x="0" y="0" width="{}" height="{}" fill="#fff"/>"##,
            number(width),
            number(height)
        );
        Self { width, height, body }
    }

    fn paints(&mut self, facts: &Facts, viewport: Viewport, shade: Shade) {
        let position: BTreeMap<&str, usize> =
            facts.order().iter().enumerate().map(|(k, id)| (id.as_str(), k)).collect();
        let mut paints: Vec<Item> = items(facts, viewport)
            .into_iter()
            .filter(|item| item.kind == ItemKind::Paint)
            .collect();
        let place = |item: &Item| {
            let (element, pseudo) =
                item.id.split_once("::").map_or((item.id.as_str(), ""), |(e, p)| (e, p));
            (
                position.get(element).copied().unwrap_or(usize::MAX),
                pseudo != "before" && !pseudo.is_empty(),
            )
        };
        paints.sort_by_key(place);
        for item in &paints {
            self.paint(item, shade);
        }
    }

    fn paint(&mut self, item: &Item, shade: Shade) {
        let [x, y, w, h] = item.bounds;
        let fields: Vec<&str> = item.signature.split('|').collect();
        let (fill, stroke, stroke_width, radius) = match fields[0] {
            "box" => {
                let fill = if !transparent(fields[1]) {
                    fields[1].to_owned()
                } else if fields[2] != "none" {
                    // a mid grey: text over a background image is often light
                    "#7d7d7d".to_owned()
                } else {
                    "none".to_owned()
                };
                let widths: Vec<f64> = fields[3].split(',').filter_map(parse_float).collect();
                let border = widths.iter().copied().fold(0.0, f64::max);
                let colour =
                    fields[4].split(',').find(|c| !c.is_empty()).unwrap_or("none").to_owned();
                let shadow =
                    if border == 0.0 && fields[6] != "none" { "#ccc".to_owned() } else { colour };
                let radius = parse_float(fields[5]).unwrap_or(0.0);
                (fill, shadow, border.max(if fields[6] == "none" { 0.0 } else { 1.0 }), radius)
            }
            "svg" => ("#e8e8f0".to_owned(), "#99a".to_owned(), 1.0, 0.0),
            _ => ("#ececec".to_owned(), "#999".to_owned(), 1.0, 0.0),
        };
        let (fill, stroke) = match shade {
            Shade::Colour => (fill, stroke),
            Shade::Grey => (
                if fill == "none" { fill } else { "#f2f2f2".to_owned() },
                if stroke_width > 0.0 { "#c8c8c8".to_owned() } else { "none".to_owned() },
            ),
        };
        let _ = write!(
            self.body,
            r#"<rect x="{}" y="{}" width="{}" height="{}" rx="{}" fill="{}" stroke="{}" stroke-width="{}"/>"#,
            number(x),
            number(y),
            number(w),
            number(h),
            number(radius.min(w / 2.0).min(h / 2.0)),
            escape(&fill),
            escape(&stroke),
            number(stroke_width),
        );
        if fields[0].starts_with("img") {
            let _ = write!(
                self.body,
                r##"<path d="M{} {}L{} {}M{} {}L{} {}" stroke="#bbb"/>"##,
                number(x),
                number(y),
                number(x + w),
                number(y + h),
                number(x + w),
                number(y),
                number(x),
                number(y + h),
            );
        }
    }

    fn texts(&mut self, facts: &Facts, viewport: Viewport, shade: Shade) {
        for id in facts.order() {
            let node = facts.node(id);
            if node["tag"] != "#text" {
                continue;
            }
            let Some(parent) = node["parent"].as_str().and_then(|p| facts.node_by_key(p)) else {
                continue;
            };
            if Facts::box_at(parent, viewport).is_none()
                || facts.style_at(parent, "visibility", viewport) == "hidden"
            {
                continue;
            }
            let rects = Facts::line_rects(node, viewport);
            if rects.is_empty() {
                continue;
            }
            let text = node["text"].as_str().unwrap_or("");
            let lines = split_lines(text, &Facts::line_breaks(node, viewport), rects.len());
            let colour = match shade {
                Shade::Colour => facts.style_at(parent, "color", viewport),
                Shade::Grey => "#9a9a9a".to_owned(),
            };
            for (rect, line) in rects.iter().zip(&lines) {
                self.text_line(*rect, line, &colour);
            }
        }
    }

    fn text_line(&mut self, [x, y, w, h]: Rect, line: &str, colour: &str) {
        let words = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if words.is_empty() || w <= 0.0 || h <= 0.0 {
            return;
        }
        let _ = write!(
            self.body,
            r#"<text x="{}" y="{}" font-size="{}" fill="{}" textLength="{}" lengthAdjust="spacingAndGlyphs">{}</text>"#,
            number(x),
            number(0.78f64.mul_add(h, y)),
            number(h * 0.8),
            escape(colour),
            number(w),
            escape(&words),
        );
    }

    fn mark(&mut self, item: &Item, why: &str, colour: &str, dash: &str) {
        for [x, y, w, h] in &item.boxes {
            let _ = write!(
                self.body,
                r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{colour}" fill-opacity="0.18" stroke="{colour}" stroke-width="2" stroke-dasharray="{dash}"><title>{} {}: {}</title></rect>"#,
                number(*x),
                number(*y),
                number(*w),
                number(*h),
                item.kind.name(),
                escape(&item.label),
                escape(why),
            );
        }
    }

    fn finish(self) -> String {
        let shown = self.width.min(SHOWN_WIDTH);
        let scale = shown / self.width;
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}" font-family="sans-serif">{}</svg>"#,
            number(shown),
            number(self.height * scale),
            number(self.width),
            number(self.height),
            self.body
        ) + "\n"
    }
}

// The `count` lines of `text` split at `breaks`, offsets in UTF-16 units.
fn split_lines(text: &str, breaks: &[usize], count: usize) -> Vec<String> {
    let mut lines = vec![String::new(); count.max(1)];
    let mut line = 0;
    let mut offset = 0;
    for c in text.chars() {
        while line < breaks.len() && offset >= breaks[line] {
            line += 1;
        }
        if let Some(held) = lines.get_mut(line.min(count.saturating_sub(1))) {
            held.push(c);
        }
        offset += c.len_utf16();
    }
    lines
}

// A coordinate to one decimal place, without a trailing `.0`.
fn number(value: f64) -> String {
    let text = format!("{:.1}", (value * 10.0).round() / 10.0);
    let text = text.strip_suffix(".0").unwrap_or(&text).to_owned();
    if text == "-0" { "0".to_owned() } else { text }
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}
