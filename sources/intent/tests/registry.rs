//! The embedded prose registry: intent's own prompts ride inside, and the
//! adapter's bare `references/` tree (only the `spec-runtime` symlink)
//! still resolves.

use adapter::Source as _;
use adapter::registry::{body, find};
use intent::Intent;

#[test]
fn embeds_prompts() {
    assert!(body(Intent::docs(), "prompts/survey.md").starts_with("# intent.survey"));
    assert!(body(Intent::docs(), "prompts/extract.md").starts_with("# intent.extract"));
}

/// Intent ships no references of its own — its `references/` tree holds
/// only the `spec-runtime` symlink, which resolves inline at build time.
#[test]
fn references_spec_runtime_only() {
    assert!(find(Intent::docs(), "references/spec-runtime/reconciliation.md").is_some());
    assert!(
        Intent::docs()
            .iter()
            .filter(|doc| doc.path.starts_with("references/"))
            .all(|doc| doc.path.starts_with("references/spec-runtime/")),
        "every embedded reference is the resolved shared runtime tree"
    );
}
