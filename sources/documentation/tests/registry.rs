//! The embedded prose registry: prompt and rule coverage plus symlink
//! resolution.

use documentation::Adapter;
use emery_adapter::Source as _;
use emery_adapter::registry::{body, find};

#[test]
fn embeds_extract_prompt() {
    assert!(body(Adapter::docs(), "prompts/extract.md").starts_with("# `documentation.extract`"));
}

#[test]
fn no_survey_prose() {
    assert!(find(Adapter::docs(), "prompts/survey.md").is_none(), "survey prose is deleted");
}

#[test]
fn embeds_rules() {
    let doc = find(Adapter::docs(), "rules/documentation-verbatim-preservation.md")
        .expect("SRC-001 rule overlay is embedded");
    assert!(doc.body.contains("id: SRC-001"), "rule frontmatter carries its id");
}

// No embedded document may exceed the 800 non-blank-line hard cap.
#[test]
fn prose_caps() {
    for doc in Adapter::docs() {
        let lines = doc.body.lines().filter(|line| !line.trim().is_empty()).count();
        assert!(lines <= 800, "{} carries {lines} non-blank lines (cap 800)", doc.path);
    }
}

// The `references/emery-runtime` symlink resolves at build time with
// the shared content inlined under the symlink-name paths.
#[test]
fn symlink_resolved_inline() {
    let doc = find(Adapter::docs(), "references/emery-runtime/reconciliation.md")
        .expect("symlinked runtime reference is embedded");
    assert!(!doc.body.is_empty(), "resolved symlink content is inlined");
}
