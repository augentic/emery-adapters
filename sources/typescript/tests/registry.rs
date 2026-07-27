//! The embedded prose registry: typescript's own prompts and its deep
//! references ride inside.

use adapter::Source as _;
use adapter::registry::{body, find};
use typescript::Adapter;

#[test]
fn embeds_prompts() {
    assert!(
        body(Adapter::docs(), "prompts/survey.md")
            .starts_with("# TypeScript / JavaScript source survey")
    );
    assert!(
        body(Adapter::docs(), "prompts/extract.md")
            .starts_with("# TypeScript / JavaScript source extract")
    );
}

/// The extraction references the extract prompt loads on demand is
/// embedded alongside the resolved `emery-runtime` symlink content.
#[test]
fn embeds_references() {
    for path in [
        "references/business-logic.md",
        "references/language-mapping.md",
        "references/scope-filters.md",
        "references/emery-runtime/reconciliation.md",
    ] {
        assert!(find(Adapter::docs(), path).is_some(), "registry embeds `{path}`");
    }
}
