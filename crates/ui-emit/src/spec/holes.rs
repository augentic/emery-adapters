//! Finding the decisions the design file did not contain.
//!
//! `vx extract` writes `TODO(binding)` where it could read the *kind* of thing that belongs
//! somewhere but not its value. The Vectis schemas do not catch these — `color: TODO(token)` is a
//! string where a string was expected, so it validates — and in Emery they never had to, because
//! the adapter filled them before anything validated.
//!
//! Standalone they have to be found, because they are `vx repair`'s entire work list. A hole
//! carries three things the stage needs: which kind of decision is missing, where it goes, and the
//! `TODO(...)` text to replace.

use std::fmt;

use serde_json::Value;

use crate::spec::shared::escape_pointer_token;

/// The kinds of decision a design file cannot contain, mirroring `figma_compile::Hole`.
///
/// Duplicated rather than depended upon: `vx-spec` validates authored specs, which may have been
/// written by hand or by a different extractor, so it must recognise the marker vocabulary without
/// coupling to the tool that happened to write it. `crates/vx/tests/markers.rs` pins the two
/// vocabularies equal, from the one crate that depends on both.
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

    /// What has to be decided, phrased as an instruction to whoever fills it.
    #[must_use]
    pub const fn wants(self) -> &'static str {
        match self {
            Self::Binding => {
                "the field this reads, as a name on the screen's view model — the collection a \
                 repeated region iterates, or the value a control shows and changes"
            }
            Self::Event => "the name of the event this tap sends, in the app's event vocabulary",
            Self::Archetype => "which archetype this container realises, from the closed set",
            Self::Token => "the name of an entry that exists in tokens.yaml",
            Self::ItemType => "the model these rows are bound to",
            Self::State => "the screen state, and what the user sees in it",
            Self::Condition => "what puts the screen into this state",
            Self::Options => "the values this control picks from",
        }
    }

    /// The kind this text is the marker for, when it is one.
    ///
    /// Public because the marker vocabulary is not only `vx repair`'s work list: a check that
    /// resolves a name against the design system has to be able to tell an unfilled decision from
    /// a decision that was made, and get a different answer for each. See
    /// vx-spec's `behaviour::vocabulary::Pending`.
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
        let holes = find(&document, "tokens.yaml");

        assert_eq!(holes.len(), 1);
        assert_eq!(holes[0].kind, Kind::Token);
        assert_eq!(holes[0].path, "/border/thin/color");
        assert_eq!(holes[0].file, "tokens.yaml");
    }

    #[test]
    fn markers_inside_arrays_are_located_by_index() {
        let document = serde_json::json!({ "items": [{ "event": "TODO(event)" }] });
        let holes = find(&document, "composition.yaml");

        assert_eq!(holes.len(), 1);
        assert_eq!(holes[0].path, "/items/0/event");
    }

    /// A real value that merely mentions a marker is not a hole.
    #[test]
    fn prose_containing_the_word_todo_is_not_a_hole() {
        let document = serde_json::json!({ "alt": "TODO: ask design about this" });
        assert!(find(&document, "assets.yaml").is_empty());
    }

    #[test]
    fn a_filled_document_has_no_holes() {
        let document = serde_json::json!({ "border": { "thin": { "color": "outline" } } });
        assert!(find(&document, "tokens.yaml").is_empty());
    }

    #[test]
    fn every_kind_round_trips_through_its_marker() {
        for kind in Kind::ALL {
            assert_eq!(Kind::of(kind.marker()), Some(kind));
        }
    }
}
