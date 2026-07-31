//! `guidance` / `build` / `merge` over shared [`phase`] scaffolding.
//!
//! Build: json-schema → openapi → asyncapi, then verify-repair, then
//! report; validators re-gate the answer (validate-before-visible).

use std::path::Path;

use adapter::registry::Doc;
use adapter::seam::{
    BuildContext, BuildInput, Context, Error, Finding, Input, MergePhase, Report, Severity,
    TargetMetadata, WorkingTree,
};
use adapter::{AdapterIdentity, Model, Target, phase};

use crate::registry;
use crate::validate::{ContractFinding, validate_baseline};

const MAX_REPAIR_ITERATIONS: usize = 2;

struct SubFlow {
    format: &'static str,
    prompt: &'static str,
    // `contracts/` subdirectory this format owns (routes repair findings).
    dir: &'static str,
}

// Schema vocabulary stabilises before the bindings reference it.
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

/// API contract authoring, import, and validation.
#[derive(Clone, Copy, Debug)]
pub struct Adapter;

impl Target for Adapter {
    const IDENTITY: AdapterIdentity = AdapterIdentity {
        name: "contracts",
        version: env!("CARGO_PKG_VERSION"),
    };

    fn metadata() -> TargetMetadata {
        TargetMetadata {
            emery_floor: Some("0.35.0".to_string()),
            inputs: vec![BuildInput {
                path: "contracts".to_string(),
                required: false,
            }],
            platforms: None,
        }
    }

    fn docs() -> &'static [Doc] {
        registry::docs()
    }

    async fn guidance<P: Model>(_model: &P, _ctx: &Context<'_>) -> Result<String, Error> {
        Ok(registry::body("prompts/guidance.md").to_string())
    }

    async fn build<P: Model>(
        model: &P, ctx: &Context<'_>, slice: &str, inputs: &[Input], _context: &BuildContext,
        tree: &WorkingTree,
    ) -> Result<Report, Error> {
        let slice_contracts_rel = format!(".emery/slices/{slice}/contracts");
        let slice_contracts = ctx.tree_root(tree).join(&slice_contracts_rel);
        let inputs_block = phase::render_inputs(inputs);
        let build_prompt = registry::body("prompts/build.md");

        // Each sub-flow judges applicability and self-skips.
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
            let answer =
                phase::phase(model, ctx, system, user, &format!("{format}-sub-flow")).await?;
            summaries.push(phase::render_outcome(format, &answer));
        }

        // Session-less repair: every finding in one call, owning sub-prompts inlined.
        for _ in 0..MAX_REPAIR_ITERATIONS {
            let findings = validate_baseline(&slice_contracts);
            if findings.is_empty() {
                break;
            }
            let system =
                format!("{build_prompt}{}", owning_sub_prompts(&findings, &slice_contracts));
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

        // Validate-before-visible: residual blocking findings override the answer.
        Ok(enforce_validators(report, &validate_baseline(&slice_contracts)))
    }

    async fn merge<P: Model>(
        model: &P, ctx: &Context<'_>, slice: &str, phase: MergePhase, tree: &WorkingTree,
    ) -> Result<Report, Error> {
        if phase == MergePhase::Preflight {
            let staged = ctx.tree_root(tree).join(format!(".emery/slices/{slice}/contracts"));
            return Ok(enforce_validators(Report::success(), &validate_baseline(&staged)));
        }

        let baseline = ctx.tree_root(tree).join("contracts");
        let merge_prompt = registry::body("prompts/merge.md");

        // Clean baseline → deterministic success; otherwise one repair leg.
        let mut report = Report::success();
        let mut findings = validate_baseline(&baseline);
        if !findings.is_empty() {
            let user = format!(
                "The postflight contract validators found blocking issues in the merged \
             `contracts/` baseline (slice `{slice}`, adapter `{}`). The engine has \
             already promoted the slice's delta and archived the slice. Repair the \
             baseline files in place, then answer with the corrected report body.\n\n{}",
                ctx.adapter_id,
                render_validator_findings(&findings),
            );
            report = phase::report(model, ctx, merge_prompt.to_string(), user).await?;
            findings = validate_baseline(&baseline);
        }

        Ok(enforce_validators(report, &findings))
    }
}

// Findings that route nowhere pull in every sub-prompt.
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

fn validator_finding(finding: &ContractFinding) -> Finding {
    Finding {
        rule_id: Some(finding.rule_id.to_string()),
        severity: Severity::Important,
        detail: format!("{}: {}", finding.path.display(), finding.detail),
    }
}

fn enforce_validators(report: Report, residual: &[ContractFinding]) -> Report {
    phase::enforce(report, residual.iter().map(validator_finding).collect())
}

fn render_validator_findings(findings: &[ContractFinding]) -> String {
    findings
        .iter()
        .map(|finding| {
            format!("- [{}] {}: {}", finding.rule_id, finding.path.display(), finding.detail)
        })
        .collect::<Vec<_>>()
        .join("\n")
}
