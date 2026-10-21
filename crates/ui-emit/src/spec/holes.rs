//! Finding the decisions the design file did not contain.
//!
//! An extractor writes `TODO(binding)` where it could read the *kind* of thing that belongs
//! somewhere but not its value. The schemas do not catch these (`color: TODO(token)` is a string
//! where a string was expected, so it validates), so they are found here, and the emit refuses a
//! design system that still has any unless holes are allowed. A hole carries which kind of
//! decision is missing, where it goes, and the `TODO(...)` text to replace.

use std::fmt;

use serde_json::Value;

use crate::spec::shared::escape_pointer_token;

/// The kinds of decision a design file cannot contain.
///
/// A design system may have been written by hand or by any extractor, so the marker vocabulary is
/// recognised here without coupling to the tool that happened to write it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    /// Which collection a repeated region iterates.
    Binding,
    /// The name of the event a tap sends.
    Event,
    /// Which archetype an interactive container realises.
    Archetype,
    /// A token referenced by a name the token file does not define.
    Token,
    /// The type of a screen's item, naming the model its rows bind to.
    ItemType,
    /// A screen state nobody drew.
    State,
    /// When a drawn state applies.
    Condition,
    /// The set a choice control picks from.
    Options,
}

impl Kind {
    /// Every kind, for exhaustive checks.
    pub const ALL: [Self; 8] = [
        Self::Binding,
        Self::Event,
        Self::Archetype,
        Self::Token,
        Self::ItemType,
        Self::State,
        Self::Condition,
        Self::Options,
    ];

    /// The marker text as it appears in the artifact.
    #[must_use]
    pub const fn marker(self) -> &'static str {
        match self {
            Self::Binding => "TODO(binding)",
            Self::Event => "TODO(event)",
            Self::Archetype => "TODO(archetype)",
            Self::Token => "TODO(token)",
            Self::ItemType => "TODO(item-type)",
            Self::State => "TODO(state)",
            Self::Condition => "TODO(condition)",
            Self::Options => "TODO(options)",
        }
    }

    /// The kind this text is the marker for, when it is one.
    ///
    /// A check that resolves a name against the design system has to be able to tell an unfilled
    /// decision from a decision that was made, and get a different answer for each.
    #[must_use]
    pub fn of(text: &str) -> Option<Self> {
        let text = text.trim();
        Self::ALL.into_iter().find(|kind| kind.marker() == text)
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.marker())
    }
}

/// One unfilled decision, located.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hole {
    /// What kind of decision is missing.
    pub kind: Kind,

    /// JSON Pointer to the value that has to be replaced.
    pub path: String,

    /// Which file it is in.
    pub file: String,
}

/// Finds every hole in a parsed artifact.
#[must_use]
pub fn find(document: &Value, file: &str) -> Vec<Hole> {
    let mut holes = Vec::new();
    walk(document, "", file, &mut holes);
    holes
}

fn walk(node: &Value, path: &str, file: &str, out: &mut Vec<Hole>) {
    match node {
        Value::String(text) => {
            if let Some(kind) = Kind::of(text) {
                out.push(Hole {
                    kind,
                    path: path.to_owned(),
                    file: file.to_owned(),
                });
            }
        }
        Value::Object(map) => {
            for (key, value) in map {
                walk(value, &format!("{path}/{}", escape_pointer_token(key)), file, out);
            }
        }
        Value::Array(entries) => {
            for (index, entry) in entries.iter().enumerate() {
                walk(entry, &format!("{path}/{index}"), file, out);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_marker_is_found_and_located() {
        let document = serde_json::json!({ "border": { "thin": { "color": "TODO(token)" } } });
        let holes = find(&document, "tokens.json");

        assert_eq!(holes.len(), 1);
        assert_eq!(holes[0].kind, Kind::Token);
        assert_eq!(holes[0].path, "/border/thin/color");
        assert_eq!(holes[0].file, "tokens.json");
    }

    #[test]
    fn markers_inside_arrays_are_located_by_index() {
        let document = serde_json::json!({ "items": [{ "event": "TODO(event)" }] });
        let holes = find(&document, "composition.json");

        assert_eq!(holes.len(), 1);
        assert_eq!(holes[0].path, "/items/0/event");
    }

    /// A real value that merely mentions a marker is not a hole.
    #[test]
    fn prose_containing_the_word_todo_is_not_a_hole() {
        let document = serde_json::json!({ "alt": "TODO: ask design about this" });
        assert_eq!(find(&document, "assets.json"), Vec::new());
    }

    #[test]
    fn a_filled_document_has_no_holes() {
        let document = serde_json::json!({ "border": { "thin": { "color": "outline" } } });
        assert_eq!(find(&document, "tokens.json"), Vec::new());
    }

    #[test]
    fn every_kind_round_trips_through_its_marker() {
        for kind in Kind::ALL {
            assert_eq!(Kind::of(kind.marker()), Some(kind));
        }
    }
}
