//! The page as regions, containers, and leaves, read off the document nesting alone.
//!
//! Every element shown at one of the [`REVIEW_WIDTHS`] at least is placed by
//! its own shape, never by grouping its neighbours: a landmark (`header`,
//! `nav`, `main`, `section`, `article`, `aside`, `footer`, `form`, or a
//! landmark `role`) is a region; any other element nesting two elements or
//! more is a container; an element nesting none is a leaf. A wrapper nesting
//! one element is not placed, and `html`, `body`, and `br` never are.

use std::fmt::Write as _;

use serde_json::Value;

use crate::facts::{Rect, js_round};
use crate::{Facts, NodeId, REVIEW_WIDTHS, Viewport, Width};

const LANDMARK_TAGS: [&str; 8] =
    ["header", "nav", "main", "section", "article", "aside", "footer", "form"];
const LANDMARK_ROLES: [&str; 8] =
    ["banner", "navigation", "main", "region", "complementary", "contentinfo", "form", "search"];
const UNPLACED_TAGS: [&str; 3] = ["html", "body", "br"];

// The longest label, in characters, before it is cut.
const LABEL_CHARS: usize = 60;

/// What a placed element is to the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// A landmark.
    Region,
    /// An element nesting two elements or more.
    Container,
    /// An element nesting none.
    Leaf,
}

/// One placed element.
#[derive(Debug, Clone, PartialEq)]
pub struct Placement {
    /// Region, container, or leaf.
    pub role: Role,
    /// The element.
    pub id: NodeId,
    /// The nearest placed element it nests in.
    pub parent: Option<NodeId>,
    /// The facts file holding the element, relative to the facts folder.
    pub file: String,
    /// The element's tag.
    pub tag: String,
    /// Its accessible name or the start of its text; empty where it has neither.
    pub label: String,
    /// Its box at each review width it is shown at.
    pub boxes: Vec<(Width, Rect)>,
}

impl Placement {
    /// The placement in one line: tag, id, parent, label, and boxes.
    #[must_use]
    pub fn synopsis(&self) -> String {
        let mut line = format!("`{}` {}", self.tag, self.id);
        if let Some(parent) = &self.parent {
            let _ = write!(line, " in {parent}");
        }
        if !self.label.is_empty() {
            let _ = write!(line, ": \"{}\"", self.label);
        }
        for (width, [x, y, w, h]) in &self.boxes {
            let _ = write!(
                line,
                "; at {width} {},{} {}x{}",
                js_round(*x),
                js_round(*y),
                js_round(*w),
                js_round(*h)
            );
        }
        line
    }
}

/// Every placed element of `facts`, in document order.
#[must_use]
pub fn outline(facts: &Facts) -> Vec<Placement> {
    let mut placed: Vec<Placement> = Vec::new();
    for id in facts.order() {
        let node = facts.node(id);
        let tag = node["tag"].as_str().unwrap_or_default();
        if node["kind"].as_str() == Some("text") || UNPLACED_TAGS.contains(&tag) {
            continue;
        }
        let boxes: Vec<(Width, Rect)> = REVIEW_WIDTHS
            .into_iter()
            .filter_map(|width| Facts::box_at(node, Viewport::at(width)).map(|rect| (width, rect)))
            .collect();
        if boxes.is_empty() {
            continue;
        }
        let nested = children(facts, node)
            .filter(|child| {
                child["kind"].as_str() != Some("text") && child["tag"].as_str() != Some("br")
            })
            .count();
        let role = if landmark(node, tag) {
            Role::Region
        } else if nested >= 2 {
            Role::Container
        } else if nested == 0 {
            Role::Leaf
        } else {
            continue;
        };
        placed.push(Placement {
            role,
            id: id.clone(),
            parent: placed_ancestor(facts, node, &placed),
            file: facts.file_of(id).to_owned(),
            tag: tag.to_owned(),
            label: label(facts, node),
            boxes,
        });
    }
    placed
}

fn landmark(node: &Value, tag: &str) -> bool {
    LANDMARK_TAGS.contains(&tag)
        || node["attrs"]["role"].as_str().is_some_and(|role| LANDMARK_ROLES.contains(&role))
}

fn children<'a>(facts: &'a Facts, node: &'a Value) -> impl Iterator<Item = &'a Value> {
    node["children"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter_map(|child| facts.node_by_key(child))
}

fn placed_ancestor(facts: &Facts, node: &Value, placed: &[Placement]) -> Option<NodeId> {
    let mut parent = node["parent"].as_str();
    while let Some(id) = parent {
        if let Some(found) = placed.iter().rev().find(|placement| placement.id.as_str() == id) {
            return Some(found.id.clone());
        }
        parent = facts.node_by_key(id).and_then(|node| node["parent"].as_str());
    }
    None
}

fn label(facts: &Facts, node: &Value) -> String {
    let named = ["aria-label", "alt", "title"]
        .into_iter()
        .find_map(|name| node["attrs"][name].as_str().filter(|value| !value.trim().is_empty()));
    let text = named.map_or_else(
        || {
            let mut text = String::new();
            gather_text(facts, node, &mut text);
            text
        },
        str::to_owned,
    );
    let words = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if words.chars().count() <= LABEL_CHARS {
        return words;
    }
    words.chars().take(LABEL_CHARS - 1).chain(['…']).collect()
}

fn gather_text(facts: &Facts, node: &Value, text: &mut String) {
    if let Some(own) = node["text"].as_str() {
        text.push_str(own);
        text.push(' ');
    }
    for child in children(facts, node) {
        gather_text(facts, child, text);
    }
}
