//! The embedded prose registry: intent's own briefs ride inside, and the
//! adapter's bare `references/` tree (only the `spec-runtime` symlink)
//! still resolves.

use specify_intent_core::registry;

#[test]
fn registry_embeds_the_briefs() {
    assert!(registry::body("briefs/survey.md").starts_with("# intent.survey"));
    assert!(registry::body("briefs/extract.md").starts_with("# intent.extract"));
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
