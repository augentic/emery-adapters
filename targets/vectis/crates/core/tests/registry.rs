//! The embedded prose registry: coverage across all three trees,
//! nested build prompts, ordering, and symlink resolution.

use specify_vectis_core::registry;

#[test]
fn registry_embeds_prompts_references_and_rules() {
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
        "rules/VECTIS-009-lint-suppression-forbidden.md",
    ] {
        assert!(registry::doc(path).is_some(), "registry embeds `{path}`");
    }
    assert!(registry::body("prompts/build.md").starts_with("# Vectis target — build prompt"));
}

/// The vectis prose shelf is the largest in the repo: 68 markdown files
/// (~600 KB) across `prompts/` (with its nested per-platform build
/// sub-trees), `references/`, and `rules/` must all embed. The floor
/// guards against a silently truncated walk without pinning the exact
/// prose inventory.
#[test]
fn registry_embeds_the_full_reference_shelf() {
    let docs = registry::docs();
    assert!(docs.len() >= 65, "expected the full prose shelf, got {} docs", docs.len());
    let total: usize = docs.iter().map(|doc| doc.body.len()).sum();
    assert!(total >= 550 * 1024, "expected >= 550 KiB of embedded prose, got {total} bytes");
}

/// Only markdown embeds: the `rules/vectis.mdc` Cursor rule stays out of
/// the registry (the codegen walks `.md` files only).
#[test]
fn non_markdown_rules_are_not_embedded() {
    assert!(registry::doc("rules/vectis.mdc").is_none());
}

/// The `references/spec-runtime` and `references/agent-teams.md`
/// symlinks into `shared/references/runtime/` are resolved at build
/// time: documents appear under their symlink-name paths with the
/// shared content inlined.
#[test]
fn shared_runtime_symlinks_are_resolved_inline() {
    let doc = registry::doc("references/spec-runtime/phase-outcome-contract.md")
        .expect("symlinked runtime reference is embedded");
    assert!(!doc.body.is_empty(), "resolved symlink content is inlined");
    assert!(!registry::body("references/agent-teams.md").is_empty());
}

/// `registry::doc` binary-searches, so the generated table must be
/// sorted and duplicate-free.
#[test]
fn docs_are_sorted_and_unique_by_path() {
    let docs = registry::docs();
    assert!(!docs.is_empty());
    for pair in docs.windows(2) {
        assert!(pair[0].path < pair[1].path, "`{}` < `{}`", pair[0].path, pair[1].path);
    }
}
