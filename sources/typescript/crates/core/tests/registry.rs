//! The embedded prose registry: typescript's own prompts and its deep
//! reference shelf ride inside.

use specify_typescript_core::registry;

#[test]
fn registry_embeds_the_prompts() {
    assert!(
        registry::body("prompts/survey.md").starts_with("# TypeScript / JavaScript source survey")
    );
    assert!(
        registry::body("prompts/extract.md")
            .starts_with("# TypeScript / JavaScript source extract")
    );
}

/// The extraction reference shelf the extract prompt loads on demand is
/// embedded alongside the resolved `spec-runtime` symlink content.
#[test]
fn registry_embeds_the_reference_shelf() {
    for path in [
        "references/business-logic.md",
        "references/language-mapping.md",
        "references/scope-filters.md",
        "references/spec-runtime/reconciliation.md",
    ] {
        assert!(registry::doc(path).is_some(), "registry embeds `{path}`");
    }
}
