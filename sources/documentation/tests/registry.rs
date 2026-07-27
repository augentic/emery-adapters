//! The embedded prose registry: prompt and rule coverage plus symlink
//! resolution.

use adapter::Source as _;
use adapter::registry::{body, find};
use documentation::Adapter;

#[test]
fn embeds_prompts() {
    assert!(body(Adapter::docs(), "prompts/survey.md").starts_with("# `documentation.survey`"));
    assert!(body(Adapter::docs(), "prompts/extract.md").starts_with("# `documentation.extract`"));
}

/// The `rules/` overlay pack travels inside the component.
#[test]
fn embeds_rules() {
    let doc = find(Adapter::docs(), "rules/documentation-verbatim-preservation.md")
        .expect("SRC-001 rule overlay is embedded");
    assert!(doc.body.contains("id: SRC-001"), "rule frontmatter carries its id");
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
