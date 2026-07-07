//! The embedded prose registry: prompt and rule coverage plus symlink
//! resolution.

use documentation_core::registry;

#[test]
fn registry_embeds_the_prompts() {
    assert!(registry::body("prompts/survey.md").starts_with("# `documentation.survey`"));
    assert!(registry::body("prompts/extract.md").starts_with("# `documentation.extract`"));
}

/// The `rules/` overlay pack travels inside the component (RFC-66
/// §"Codex ownership becomes real").
#[test]
fn registry_embeds_the_rule_overlay() {
    let doc = registry::doc("rules/documentation-verbatim-preservation.md")
        .expect("SRC-001 rule overlay is embedded");
    assert!(doc.body.contains("id: SRC-001"), "rule frontmatter carries its id");
}

/// The `references/spec-runtime` symlink into `codex/references/runtime/`
/// is resolved at build time: documents appear under their symlink-name
/// paths with the shared content inlined.
#[test]
fn spec_runtime_symlink_is_resolved_inline() {
    let doc = registry::doc("references/spec-runtime/reconciliation.md")
        .expect("symlinked runtime reference is embedded");
    assert!(!doc.body.is_empty(), "resolved symlink content is inlined");
}
