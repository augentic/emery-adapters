//! The embedded prose registry: intent's own prompts ride inside, and the
//! adapter's bare `references/` tree (only the `emery-runtime` symlink)
//! still resolves.

use emery_adapter::Source as _;
use emery_adapter::registry::{body, find};
use intent::Adapter;

#[test]
fn embeds_extract_prompt() {
    assert!(body(Adapter::docs(), "prompts/extract.md").starts_with("# intent.extract"));
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

// Intent's `references/` tree holds only the `emery-runtime` symlink.
#[test]
fn runtime_references_only() {
    assert!(find(Adapter::docs(), "references/emery-runtime/reconciliation.md").is_some());
    assert!(
        Adapter::docs()
            .iter()
            .filter(|doc| doc.path.starts_with("references/"))
            .all(|doc| doc.path.starts_with("references/emery-runtime/")),
        "every embedded reference is the resolved shared runtime tree"
    );
}
