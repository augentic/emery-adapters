//! The embedded prose registry: coverage across all three trees,
//! nested build prompts, ordering, and symlink resolution.

use vectis::registry;

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
        assert!(registry::doc(path).is_some(), "registry embeds `{path}`");
    }
    assert!(registry::body("prompts/build.md").starts_with("# Vectis target — build prompt"));
}

/// The vectis prose references is the largest in the repo: 68 markdown files
/// (~600 KB) across `prompts/` (with its nested per-platform build
/// sub-trees), `references/`, and `rules/` must all embed. The floor
/// guards against a silently truncated walk without pinning the exact
/// prose inventory.
#[test]
fn embed_floor() {
    let docs = registry::docs();
    assert!(docs.len() >= 65, "expected the full prose references, got {} docs", docs.len());
    let total: usize = docs.iter().map(|doc| doc.body.len()).sum();
    assert!(total >= 550 * 1024, "expected >= 550 KiB of embedded prose, got {total} bytes");
}

/// The `references/spec-runtime` and `references/agent-teams.md`
/// symlinks into `codex/references/runtime/` are resolved at build
/// time: documents appear under their symlink-name paths with the
/// shared content inlined.
#[test]
fn symlinks_resolved_inline() {
    let doc = registry::doc("references/spec-runtime/phase-outcome-contract.md")
        .expect("symlinked runtime reference is embedded");
    assert!(!doc.body.is_empty(), "resolved symlink content is inlined");
    assert!(!registry::body("references/agent-teams.md").is_empty());
}
