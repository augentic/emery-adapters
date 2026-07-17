//! Embedded prose registry coverage and symlink resolution.

use adapter::Target as _;
use adapter::registry::{body, find};
use vectis::Adapter;

#[test]
fn embeds_all_trees() {
    for path in [
        "prompts/guidance.md",
        "prompts/build.md",
        "prompts/merge.md",
        "prompts/build/composition.md",
        "prompts/build/test.md",
        "prompts/build/core/write.md",
        "prompts/build/core/review.md",
        "prompts/build/ios/write.md",
        "prompts/build/ios/review.md",
        "prompts/build/android/write.md",
        "prompts/build/android/review.md",
        "references/hard-rules-core.md",
        "references/hard-rules-ios.md",
        "references/hard-rules-android.md",
        "references/agent-teams.md",
        "rules/VECTIS-006-asset-render-by-kind.md",
        "rules/VECTIS-007-ios-scaffold-immutability.md",
        "rules/VECTIS-008-prompts-forbid-named-simulator.md",
        "rules/VECTIS-009-lint-suppression-forbidden.md",
        "rules/universal/hardcoded-secrets.md",
        "rules/universal/unvalidated-input.md",
    ] {
        assert!(find(Adapter::docs(), path).is_some(), "registry embeds `{path}`");
    }
    assert!(
        body(Adapter::docs(), "prompts/build.md").starts_with("# Vectis target — build prompt")
    );
}

#[test]
fn embed_floor() {
    let docs = Adapter::docs();
    assert!(docs.len() >= 65, "expected the full prose references, got {} docs", docs.len());
    let total: usize = docs.iter().map(|doc| doc.body.len()).sum();
    assert!(total >= 550 * 1024, "expected >= 550 KiB of embedded prose, got {total} bytes");
}

#[test]
fn symlinks_resolved_inline() {
    let doc = find(Adapter::docs(), "references/spec-runtime/phase-outcome-contract.md")
        .expect("symlinked runtime reference is embedded");
    assert!(!doc.body.is_empty(), "resolved symlink content is inlined");
    assert!(!body(Adapter::docs(), "references/agent-teams.md").is_empty());
}
