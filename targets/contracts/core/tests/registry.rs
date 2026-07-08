//! The embedded prose registry: coverage across all three trees,
//! ordering, and symlink resolution.

use contracts_core as core;
use core::registry;

#[test]
fn registry_embeds_prompts_references_and_rules() {
    for path in [
        "prompts/guidance.md",
        "prompts/build.md",
        "prompts/merge.md",
        "prompts/build/json-schema.md",
        "prompts/build/openapi.md",
        "prompts/build/asyncapi.md",
        "references/report-shape.md",
        "references/openapi/verifier.md",
        "rules/asyncapi-consumer-compatibility.md",
        "rules/consumer-impact-classification.md",
        "rules/json-schema-evolution-safety.md",
        "rules/openapi-consumer-compatibility.md",
        "rules/semver-contract-versioning.md",
    ] {
        assert!(registry::doc(path).is_some(), "registry embeds `{path}`");
    }
    assert!(registry::body("prompts/build.md").starts_with("# contracts.build"));
}

/// The `references/spec-runtime` symlink into `codex/references/runtime/`
/// is resolved at build time: documents appear under their symlink-name
/// paths with the shared content inlined.
#[test]
fn spec_runtime_symlink_is_resolved_inline() {
    let doc = registry::doc("references/spec-runtime/phase-outcome-contract.md")
        .expect("symlinked runtime reference is embedded");
    assert!(!doc.body.is_empty(), "resolved symlink content is inlined");
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
