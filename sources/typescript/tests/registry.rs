//! The embedded prose registry: typescript's own prompts and its deep
//! references ride inside.

use adapter::Source as _;
use adapter::registry::{body, find};
use typescript::Typescript;

#[test]
fn embeds_prompts() {
    assert!(
        body(Typescript::docs(), "prompts/survey.md")
            .starts_with("# TypeScript / JavaScript source survey")
    );
    assert!(
        body(Typescript::docs(), "prompts/extract.md")
            .starts_with("# TypeScript / JavaScript source extract")
    );
}

/// The extraction references the extract prompt loads on demand is
/// embedded alongside the resolved `spec-runtime` symlink content.
#[test]
fn embeds_references() {
    for path in [
        "references/business-logic.md",
        "references/language-mapping.md",
        "references/scope-filters.md",
        "references/spec-runtime/reconciliation.md",
    ] {
        assert!(find(Typescript::docs(), path).is_some(), "registry embeds `{path}`");
    }
}
