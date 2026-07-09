//! The embedded prose registry: coverage across all three trees,
//! ordering, and symlink resolution.

use omnia::registry;

#[test]
fn embeds_all_trees() {
    for path in [
        "prompts/guidance.md",
        "prompts/build.md",
        "prompts/merge.md",
        "prompts/build/crate.md",
        "prompts/build/test.md",
        "prompts/build/guest.md",
        "prompts/build/review.md",
        "prompts/build/replay.md",
        "references/guardrails.md",
        "references/wasm-constraints.md",
        "references/team-protocol-crate.md",
        "references/providers/state-store.md",
        "references/examples/README.md",
        "rules/provider-only-host-access.md",
        "rules/wasm-guest-runtime-constraints.md",
        "rules/classified-errors-no-panics.md",
        "rules/host-managed-secrets-identity.md",
        "rules/universal/hardcoded-secrets.md",
        "rules/universal/unvalidated-input.md",
    ] {
        assert!(registry::doc(path).is_some(), "registry embeds `{path}`");
    }
    assert!(registry::body("prompts/build.md").starts_with("# Omnia target — build prompt"));
}

/// The references is the point of this adapter: ~65 markdown files
/// (~700 KB) across `references/` plus the prompts and rules must all
/// embed. The floor guards against a silently truncated walk without
/// pinning the exact prose inventory.
#[test]
fn embed_floor() {
    let docs = registry::docs();
    assert!(docs.len() >= 90, "expected the full prose references, got {} docs", docs.len());
    let total: usize = docs.iter().map(|doc| doc.body.len()).sum();
    assert!(total >= 700 * 1024, "expected >= 700 KiB of embedded prose, got {total} bytes");
}

/// Only markdown embeds: the `rules/omnia.mdc` Cursor rule stays out of
/// the registry (the codegen walks `.md` files only).
#[test]
fn markdown_only() {
    assert!(registry::doc("rules/omnia.mdc").is_none());
}

/// The `references/spec-runtime` symlink into `codex/references/runtime/`
/// is resolved at build time: documents appear under their symlink-name
/// paths with the shared content inlined.
#[test]
fn symlinks_resolved_inline() {
    let doc = registry::doc("references/spec-runtime/phase-outcome-contract.md")
        .expect("symlinked runtime reference is embedded");
    assert!(!doc.body.is_empty(), "resolved symlink content is inlined");
}

/// `registry::doc` binary-searches, so the generated table must be
/// sorted and duplicate-free.
#[test]
fn sorted_unique() {
    let docs = registry::docs();
    assert!(!docs.is_empty());
    for pair in docs.windows(2) {
        assert!(pair[0].path < pair[1].path, "`{}` < `{}`", pair[0].path, pair[1].path);
    }
}
