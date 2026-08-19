//! The embedded prose registry: prompt and rule coverage plus symlink
//! resolution.

use adapter::Source as _;
use adapter::registry::{body, find};
use documentation::Adapter;

#[test]
fn embeds_extract_prompt() {
    assert!(body(Adapter::docs(), "prompts/extract.md").starts_with("# `documentation.extract`"));
}

/// Survey is deleted from the seam (ADR-0008); no survey prose may
/// ride in the component.
#[test]
fn no_survey_prose() {
    assert!(find(Adapter::docs(), "prompts/survey.md").is_none(), "survey prose is deleted");
}

/// The `rules/` overlay pack travels inside the component.
#[test]
fn embeds_rules() {
    let doc = find(Adapter::docs(), "rules/documentation-verbatim-preservation.md")
        .expect("SRC-001 rule overlay is embedded");
    assert!(doc.body.contains("id: SRC-001"), "rule frontmatter carries its id");
}

/// The prose budget over the ported corpus: no embedded document may
/// exceed the 800 non-blank-line hard cap (AGENTS prompt-shape rule).
#[test]
fn prose_caps() {
    for doc in Adapter::docs() {
        let lines = doc.body.lines().filter(|line| !line.trim().is_empty()).count();
        assert!(lines <= 800, "{} carries {lines} non-blank lines (cap 800)", doc.path);
    }
}

/// The `references/emery-runtime` symlink into `codex/references/runtime/`
/// is resolved at build time: documents appear under their symlink-name
/// paths with the shared content inlined.
#[test]
fn symlink_resolved_inline() {
    let doc = find(Adapter::docs(), "references/emery-runtime/reconciliation.md")
        .expect("symlinked runtime reference is embedded");
    assert!(!doc.body.is_empty(), "resolved symlink content is inlined");
}
