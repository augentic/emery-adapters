//! The embedded corpus of every shipped adapter: the extraction prompt is
//! present and headed, the survey prose stays deleted, no document breaks
//! the 800 non-blank-line cap, and the shared runtime references reached the
//! build through the `references/emery-runtime` symlink. Each adapter then
//! asserts its own registry facts.
//!
//! `foreach_adapter!` is the orphan guard for the adapter set: a new
//! `sources/<name>` component fails to compile here until a test of that
//! name exists.

#![cfg(not(target_arch = "wasm32"))]

use emery_prose::registry::{Doc, body, find};
use emery_sdk::SourceAdapter as _;

// Every `sources/*` component `crates/test-programs` builds must have a
// matching test here; a new adapter without one fails to compile.
test_programs::foreach_adapter!();

/// What every adapter's corpus satisfies: `prompts/extract.md` opens with
/// `heading`, `prompts/survey.md` is absent, every document stays under the
/// cap, and the shared runtime tree resolved through the symlink.
fn corpus(docs: &[Doc], heading: &str) {
    assert!(
        body(docs, "prompts/extract.md").is_some_and(|prompt| prompt.starts_with(heading)),
        "`prompts/extract.md` opens with `{heading}`"
    );
    assert!(find(docs, "prompts/survey.md").is_none(), "survey prose is deleted");

    for doc in docs {
        let lines = doc.body.lines().filter(|line| !line.trim().is_empty()).count();
        assert!(lines <= 800, "{} carries {lines} non-blank lines (cap 800)", doc.path);
    }

    let runtime = find(docs, "references/emery-runtime/reconciliation.md")
        .expect("the symlinked runtime reference is embedded");
    assert!(!runtime.body.is_empty(), "resolved symlink content is inlined");
}

// Intent's `references/` tree holds only the `emery-runtime` symlink.
#[test]
fn intent() {
    let docs = intent::Adapter::docs();
    corpus(docs, "# intent.extract");
    assert!(
        docs.iter()
            .filter(|doc| doc.path.starts_with("references/"))
            .all(|doc| doc.path.starts_with("references/emery-runtime/")),
        "every embedded reference is the resolved shared runtime tree"
    );
}

// Documentation embeds its SRC-001 rule overlay beside the prompt.
#[test]
fn documentation() {
    let docs = documentation::Adapter::docs();
    corpus(docs, "# `documentation.extract`");
    let rule = find(docs, "rules/documentation-verbatim-preservation.md")
        .expect("SRC-001 rule overlay is embedded");
    assert!(rule.body.contains("id: SRC-001"), "rule frontmatter carries its id");
}

// TypeScript's deep references ride inside beside the shared tree.
#[test]
fn typescript() {
    let docs = typescript::Adapter::docs();
    corpus(docs, "# TypeScript / JavaScript source extract");
    for path in ["references/business-logic.md", "references/language-mapping.md"] {
        assert!(find(docs, path).is_some(), "registry embeds `{path}`");
    }
}
