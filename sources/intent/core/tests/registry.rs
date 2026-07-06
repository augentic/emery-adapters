//! The embedded prose registry: intent's own prompts ride inside, and the
//! adapter's bare `references/` tree (only the `spec-runtime` symlink)
//! still resolves.

use intent_core::registry;

#[test]
fn registry_embeds_the_prompts() {
    assert!(registry::body("prompts/survey.md").starts_with("# intent.survey"));
    assert!(registry::body("prompts/extract.md").starts_with("# intent.extract"));
}

/// Intent ships no references of its own — its `references/` tree holds
/// only the `spec-runtime` symlink, which resolves inline at build time.
#[test]
fn references_hold_only_the_resolved_spec_runtime_tree() {
    assert!(registry::doc("references/spec-runtime/reconciliation.md").is_some());
    assert!(
        registry::docs()
            .iter()
            .filter(|doc| doc.path.starts_with("references/"))
            .all(|doc| doc.path.starts_with("references/spec-runtime/")),
        "every embedded reference is the resolved shared runtime tree"
    );
}
