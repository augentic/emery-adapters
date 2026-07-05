//! The embedded prose registry: brief coverage and symlink resolution.

use specify_documentation_core::registry;

#[test]
fn registry_embeds_the_briefs() {
    assert!(registry::body("briefs/survey.md").starts_with("# `documentation.survey`"));
    assert!(registry::body("briefs/extract.md").starts_with("# `documentation.extract`"));
}

/// The `references/spec-runtime` symlink into `shared/references/runtime/`
/// is resolved at build time: documents appear under their symlink-name
/// paths with the shared content inlined.
#[test]
fn spec_runtime_symlink_is_resolved_inline() {
    let doc = registry::doc("references/spec-runtime/reconciliation.md")
        .expect("symlinked runtime reference is embedded");
    assert!(!doc.body.is_empty(), "resolved symlink content is inlined");
}
