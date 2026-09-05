//! The embedded prose registry: typescript's own prompt and its deep
//! references ride inside.

use emery_adapter::SourceAdapter as _;
use emery_prose::registry::{body, find};
use typescript::Adapter;

#[test]
fn embeds_extract_prompt() {
    assert!(
        body(Adapter::docs(), "prompts/extract.md")
            .starts_with("# TypeScript / JavaScript source extract")
    );
}

#[test]
fn no_survey_prose() {
    assert!(find(Adapter::docs(), "prompts/survey.md").is_none(), "survey prose is deleted");
}

// No embedded document may exceed the 800 non-blank-line hard cap.
#[test]
fn prose_caps() {
    for doc in Adapter::docs() {
        let lines = doc.body.lines().filter(|line| !line.trim().is_empty()).count();
        assert!(lines <= 800, "{} carries {lines} non-blank lines (cap 800)", doc.path);
    }
}

#[test]
fn embeds_references() {
    for path in [
        "references/business-logic.md",
        "references/language-mapping.md",
        "references/emery-runtime/reconciliation.md",
    ] {
        assert!(find(Adapter::docs(), path).is_some(), "registry embeds `{path}`");
    }
}
