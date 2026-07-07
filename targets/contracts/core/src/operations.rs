//! The judgment operation template: `guidance`, `build`, and `merge`,
//! over the shared [`phase`] scaffolding.
//!
//! `build` decomposes into the three format sub-flows (json-schema,
//! openapi, asyncapi) as independent legs, then a bounded verify-repair
//! loop over the absorbed contract validators, then one report leg. The
//! validators run again after the report lands (validate-before-visible);
//! residual blocking findings force `status: failure` regardless of the
//! answer.

use std::path::Path;

use adapter::seam::{
    BuildInput, Changeset, Context, Error, Finding, Input, Report, Severity, TargetManifest,
    WorkingTree,
};
use adapter::{Model, phase};

use crate::registry;
use crate::validate::{ContractFinding, validate_baseline};

/// Maximum verify-repair iterations per the build prompt's Phase 4.
const MAX_REPAIR_ITERATIONS: usize = 2;

/// One format sub-flow of the build prompt's Phase 2.
struct SubFlow {
    /// Format name, used in prompts and answer-schema names.
    format: &'static str,
    /// Registry path of the format's sub-prompt.
    prompt: &'static str,
    /// The `contracts/` subdirectory this format owns, used to route
    /// validator findings back to the owning sub-prompt for repair.
    dir: &'static str,
}

/// The three format sub-flows in the build prompt's fixed Phase 2 order:
/// the schema vocabulary stabilises before the bindings reference it.
const SUB_FLOWS: [SubFlow; 3] = [
    SubFlow {
        format: "json-schema",
        prompt: "prompts/build/json-schema.md",
        dir: "schemas",
    },
    SubFlow {
        format: "openapi",
        prompt: "prompts/build/openapi.md",
        dir: "http",
    },
    SubFlow {
        format: "asyncapi",
        prompt: "prompts/build/asyncapi.md",
        dir: "messages",
    },
];

/// Deterministic self-description for the `describe` operation.
///
/// Resolve-time metadata answered from compiled-in constants: no
/// compatibility floor; one optional build input — the slice tree's
/// `contracts/` subtree, carrying partial deltas written by a prior
/// pass.
#[must_use]
pub fn describe() -> TargetManifest {
    TargetManifest {
        specify_floor: None,
        inputs: vec![BuildInput {
            path: "contracts".to_string(),
            required: false,
        }],
        platforms: None,
    }
}

/// Guidance on the expected build artifacts for this target — the
/// embedded guidance prompt, returned deterministically (no judgment leg).
#[must_use]
pub fn guidance() -> &'static str {
    registry::body("prompts/guidance.md")
}

/// Build a slice's contract deltas under `.specify/slices/<slice>/contracts/`.
///
/// One leg per format sub-flow (fixed order), a bounded verify-repair
/// loop over the in-core validators, and one report leg whose answer the
/// validators then re-gate.
///
/// # Errors
///
/// As [`adapter::judgment`].
pub async fn build<P: Model>(
    model: &P, ctx: &Context<'_>, slice: &str, inputs: &[Input], tree: &WorkingTree,
) -> Result<Report, Error> {
    let slice_contracts_rel = format!(".specify/slices/{slice}/contracts");
    let slice_contracts = ctx.tree_root(tree).join(&slice_contracts_rel);
    let inputs_block = phase::render_inputs(inputs);
    let build_prompt = registry::body("prompts/build.md");

    // Phase 2 — author or import, fixed format order. Classification is
    // part of each sub-flow's own judgment: the prompt tells it when to
    // skip, and a skipped leg answers `applicable: false` without writing.
    let mut summaries: Vec<String> = Vec::new();
    for sub_flow in &SUB_FLOWS {
        let format = sub_flow.format;
        let system = format!("{build_prompt}\n\n---\n\n{}", registry::body(sub_flow.prompt));
        let user = format!(
            "Run the `{format}` sub-flow of the contracts build for slice `{slice}` \
             (adapter `{}`).\n\n\
             The project workspace is lent to you. Write only `.yaml` files under \
             `{slice_contracts_rel}/`; the root `contracts/` baseline is read-only \
             context for `$ref` reuse. When the slice has no surface this format owns, \
             write nothing and answer with `applicable: false`.\n\n\
             {inputs_block}",
            ctx.adapter_id,
        );
        let answer = phase::phase(model, ctx, system, user, &format!("{format}-sub-flow")).await?;
        summaries.push(phase::render_outcome(format, &answer));
    }

    // Phase 4 — verify-repair loop over the in-core validators (the
    // Phase 5 tool gate, compiled in). The prompt re-enters the owning
    // sub-prompt per format; the session-less shape folds that into one
    // repair call per iteration carrying every finding, with the owning
    // sub-prompts inlined so repair does not depend on the MCP route.
    for _ in 0..MAX_REPAIR_ITERATIONS {
        let findings = validate_baseline(&slice_contracts);
        if findings.is_empty() {
            break;
        }
        let system = format!("{build_prompt}{}", owning_sub_prompts(&findings, &slice_contracts));
        let user = format!(
            "The contract validators found blocking issues in slice `{slice}`'s delta \
             under `{slice_contracts_rel}/`. Re-enter the owning format sub-prompt(s) \
             per the build prompt's Phase 4 and repair the files in place.\n\n\
             {}\n\n\
             Answer `applicable: true` with a summary of the repairs.",
            render_validator_findings(&findings),
        );
        phase::phase(model, ctx, system, user, "repair").await?;
    }

    // Final leg — the report answer, gated by the derived answer schema.
    let system = build_prompt.to_string();
    let user = format!(
        "Write the build report for slice `{slice}`. Verify the delta under \
         `{slice_contracts_rel}/` per the build prompt's Phase 3, then answer with \
         the report body (`status`, `findings`, `outputs`, `ui-surface`). A \
         `success` report carries only non-blocking findings. Contract artifacts \
         declare no per-platform outputs, so `outputs` is normally empty.\n\n\
         Sub-flow outcomes:\n{}",
        summaries.join("\n"),
    );
    let report = phase::report(model, ctx, system, user).await?;

    // Validate-before-visible: residual blocking findings override the
    // judgment regardless of what the answer claimed.
    Ok(enforce_validators(report, &validate_baseline(&slice_contracts)))
}

/// Merge a built slice's delta into the baseline `contracts/` tree.
///
/// One judgment leg folds the delta and answers with the report, then
/// the post-merge validator gate runs in core with one bounded repair
/// leg. Merge deliberately gets one repair leg where build gets two: the
/// delta was already validated at build time, so post-merge findings are
/// collision-shaped (`id-unique` against the baseline) and either one
/// pass clears them or the slice needs human review.
///
/// # Errors
///
/// As [`adapter::judgment`].
pub async fn merge<P: Model>(
    model: &P, ctx: &Context<'_>, slice: &str, delta: &Changeset, tree: &WorkingTree,
) -> Result<Report, Error> {
    let baseline = ctx.tree_root(tree).join("contracts");
    let merge_prompt = registry::body("prompts/merge.md");
    let delta_block = phase::render_delta(delta);

    let user = format!(
        "Merge slice `{slice}`'s built contract delta into the baseline `contracts/` \
         tree (adapter `{}`). The project workspace is lent to you; the delta below \
         applies against base `{}` (a 3-way merge: the baseline is ours, the delta is \
         theirs). Fold the changes in place, resolving conflicts per the merge prompt, \
         then answer with the report body.\n\n{delta_block}",
        ctx.adapter_id, delta.base,
    );
    let mut report = phase::report(model, ctx, merge_prompt.to_string(), user).await?;

    // Post-merge validator gate with one bounded repair leg.
    let mut findings = validate_baseline(&baseline);
    if !findings.is_empty() {
        let user = format!(
            "The post-merge contract validators found blocking issues in the merged \
             `contracts/` baseline. Repair the files in place, then answer with the \
             corrected report body.\n\n{}",
            render_validator_findings(&findings),
        );
        report = phase::report(model, ctx, merge_prompt.to_string(), user).await?;
        findings = validate_baseline(&baseline);
    }

    Ok(enforce_validators(report, &findings))
}

/// Inline the sub-prompts owning the findings' files into a repair
/// prompt, routed by the `contracts/` subdirectory each format owns.
/// Findings that route nowhere pull in every sub-prompt, so repair never
/// runs without the specialist material the build prompt's Phase 4
/// re-enters.
fn owning_sub_prompts(findings: &[ContractFinding], contracts_dir: &Path) -> String {
    let unrouted = findings.iter().any(|finding| {
        !SUB_FLOWS.iter().any(|sub_flow| finding.path.starts_with(contracts_dir.join(sub_flow.dir)))
    });
    let mut inlined = String::new();
    for sub_flow in &SUB_FLOWS {
        let owned_dir = contracts_dir.join(sub_flow.dir);
        if unrouted || findings.iter().any(|finding| finding.path.starts_with(&owned_dir)) {
            inlined.push_str("\n\n---\n\n");
            inlined.push_str(registry::body(sub_flow.prompt));
        }
    }
    inlined
}

/// Map one deterministic validator finding into the seam shape.
/// Contract rules gate the build, so validator findings are blocking
/// (`important`).
fn validator_finding(finding: &ContractFinding) -> Finding {
    Finding {
        rule_id: Some(finding.rule_id.to_string()),
        severity: Severity::Important,
        detail: format!("{}: {}", finding.path.display(), finding.detail),
    }
}

/// [`phase::enforce`] over the deterministic validator residue.
fn enforce_validators(report: Report, residual: &[ContractFinding]) -> Report {
    phase::enforce(report, residual.iter().map(validator_finding).collect())
}

/// Render validator findings for a repair prompt.
fn render_validator_findings(findings: &[ContractFinding]) -> String {
    findings
        .iter()
        .map(|finding| {
            format!("- [{}] {}: {}", finding.rule_id, finding.path.display(), finding.detail)
        })
        .collect::<Vec<_>>()
        .join("\n")
}
