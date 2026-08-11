//! The six target operations over shared [`phase`] scaffolding.
//!
//! Build is generation only (json-schema → openapi → asyncapi onto the
//! lent artifact stage); `verify` runs one check pass (in-guest
//! validator + one verifier leg); `repair` performs one
//! findings-directed pass — iteration and budgets are engine policy
//! (RFC-90 D1). Contracts runs no standards review.

use adapter::registry::Doc;
use adapter::seam::{
    ArtifactStage, BuildContext, BuildInput, Context, DiagnosticSource, Error, Finding,
    FindingArtifact, FindingConfidence, FindingEvidence, FindingKind, Input, MergePhase,
    PhaseFinding, PhaseLocation, PhaseOutcome, PhaseReport, PhaseRoot, PhaseSource, PhaseWrite,
    RepairOrigin, Report, Severity, TargetMetadata, Workspace, WritableArtifact,
};
use adapter::{AdapterIdentity, Model, Target, phase};

use crate::registry;
use crate::validate::{ContractFinding, validate_baseline};

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
            emery_floor: Some("0.38.0".to_string()),
            inputs: vec![BuildInput {
                path: "contracts".to_string(),
                required: false,
            }],
            platforms: None,
            writable_artifacts: vec![
                WritableArtifact::file("tasks.md"),
                WritableArtifact::tree("contracts"),
            ],
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
        workspace: &Workspace,
    ) -> Result<PhaseReport, Error> {
        // Generation only: the staged contract delta goes onto the lent
        // writable artifact stage (mirroring the slice tree); the engine
        // dispatches `verify` / `repair` separately (RFC-90 D3).
        let stage = artifact_stage(workspace)?;
        let stage_contracts = stage_contracts(stage);
        let inputs_block = phase::render_inputs(inputs, workspace);
        let build_prompt = registry::body("prompts/build.md");

        // Each sub-flow judges applicability and self-skips; each answers
        // the full phase-report shape so a failed generation leg can
        // surface blocking findings instead of passing silently.
        let mut findings: Vec<PhaseFinding> = Vec::new();
        let mut written: Vec<PhaseWrite> = Vec::new();
        for sub_flow in &SUB_FLOWS {
            let format = sub_flow.format;
            let system = format!("{build_prompt}\n\n---\n\n{}", registry::body(sub_flow.prompt));
            let user = format!(
                "Run the `{format}` sub-flow of the contracts build for slice `{slice}` \
             (adapter `{}`).\n\n\
             A private workspace is lent to you. Write only `.yaml` files under the \
             slice's staged contract delta at `{stage_contracts}/` (the writable artifact \
             stage mirrors the slice tree; the engine promotes staged writes on terminal \
             success); the `contracts/` baseline in your workspace is read-only context \
             for `$ref` reuse. Answer with the phase report: `written` entries carry \
             root `artifacts` with paths relative to the stage root, e.g. \
             `contracts/http/user-api.yaml`. When the slice has no surface this format \
             owns, write nothing and answer `outcome: not-applicable`; when this format \
             applies but you could not produce its artifacts, report what blocked you \
             as a blocking (`important`) finding.\n\n\
             {inputs_block}",
                ctx.adapter_id,
            );
            let leg = phase::phase_report(model, ctx, system, user, &format!("{format}-sub-flow"))
                .await?;
            findings.extend(leg.findings);
            written.extend(leg.written);
        }

        // Close-out: tick the completed task checkboxes in the staged
        // tasks.md (the `tasks.md` writable grant), like the sibling
        // targets' build close-outs.
        let user = format!(
            "Run the close-out leg of the contracts build for slice `{slice}` (adapter \
         `{}`): the format sub-flows have finished. Mark the completed task \
         checkboxes in the stage copy of the task list at `{}/tasks.md` — never in \
         the authoritative slice tree — ticking only tasks this build's staged \
         contract delta actually completed. Report the written path relative to the \
         stage root (`tasks.md`).",
            ctx.adapter_id, stage.root,
        );
        let closeout =
            phase::phase(model, ctx, build_prompt.to_string(), user, "close-out").await?;
        written.extend(closeout.written.iter().map(|path| stage_write(path)));

        // One merged generation report. Contract artifacts declare no
        // per-platform outputs or UI surface, and every finding here came
        // from a model leg — any other attribution is an unreachable
        // claim, so it is forced back to `model-assisted` for coherence.
        let mut report = PhaseReport::completed(PhaseSource::ModelAssisted);
        report.findings = findings;
        for finding in &mut report.findings {
            finding.source = DiagnosticSource::ModelAssisted;
        }
        report.written = written;
        Ok(report)
    }

    async fn verify<P: Model>(
        model: &P, ctx: &Context<'_>, workspace: &Workspace,
    ) -> Result<PhaseReport, Error> {
        // One check pass over the candidate: the deterministic in-guest
        // validator always runs; one verifier-reference leg runs only
        // when the staged delta carries artifacts. No loop — the engine
        // routes blocking findings to `repair` (RFC-90 D1). A missing
        // stage is a malformed dispatch, exactly as in build and repair
        // — never a vacuous pass.
        let stage = artifact_stage(workspace)?;
        let staged = stage.root_path().join("contracts");
        let mut findings: Vec<PhaseFinding> = validate_baseline(&staged)
            .iter()
            .map(|finding| phase_finding(finding, stage.root_path()))
            .collect();

        if !has_entries(&staged) {
            // Only the in-guest validator ran (vacuously, on an empty delta).
            let mut report = PhaseReport::completed(PhaseSource::Deterministic);
            report.findings = findings;
            return Ok(report);
        }

        let user = format!(
            "Run one verification pass over the contracts candidate (adapter `{}`).\n\n\
         The staged contract delta lives at `{stage_contracts}/` — read-only for this \
         pass, like everything else; the `contracts/` baseline in the lent workspace is \
         read-only cross-reference context. Run the verifier reference of each format \
         that owns staged artifacts and skip the rest, then answer with the phase \
         report (`source: model-assisted`; carry a location path wherever the verifier \
         names a file). Do not repair anything and do not re-run checks.",
            ctx.adapter_id,
            stage_contracts = stage_contracts(stage),
        );
        let mut answer = phase::phase_report(
            model,
            ctx,
            registry::body("prompts/verify.md").to_string(),
            user,
            "verify",
        )
        .await?;
        // `tool` / `human` finding attributions never cross the seam;
        // the leg is a model pass, so sanitize them in code.
        for finding in &mut answer.findings {
            if matches!(finding.source, DiagnosticSource::Tool | DiagnosticSource::Human) {
                finding.source = DiagnosticSource::ModelAssisted;
            }
        }
        findings.extend(answer.findings);

        // Deterministic validator + model leg both contributed: the
        // report is hybrid, and verify never declares outputs, a UI
        // surface, writes, or a continuation change.
        let mut report = PhaseReport::completed(PhaseSource::Hybrid);
        report.findings = findings;
        Ok(report)
    }

    async fn repair<P: Model>(
        model: &P, ctx: &Context<'_>, slice: &str, origin: RepairOrigin, findings: &[PhaseFinding],
        _continuation: Option<&[u8]>, workspace: &Workspace,
    ) -> Result<PhaseReport, Error> {
        // One findings-directed pass; the engine owns iteration, budgets,
        // and the verification that follows (RFC-90 D1/D4).
        let stage = artifact_stage(workspace)?;
        let system =
            format!("{}{}", registry::body("prompts/repair.md"), owning_sub_prompts(findings));
        let user = format!(
            "Perform one findings-directed repair pass for slice `{slice}` (adapter `{}`, \
         origin: `{origin}`).\n\n\
         Repair the staged contract delta in place under `{stage_contracts}/` (the \
         writable artifact stage); the `contracts/` baseline in your workspace is \
         read-only. Fix only what the findings name — do not re-run verification and do \
         not loop; the engine dispatches the next verification itself. Report written \
         paths relative to the stage root and answer `applicable: true` with a summary \
         of the repairs.\n\n\
         Findings from the `{origin}` gate:\n\n{findings_block}",
            ctx.adapter_id,
            origin = origin.as_str(),
            stage_contracts = stage_contracts(stage),
            findings_block = phase::render_findings(findings),
        );
        let answer = phase::phase(model, ctx, system, user, "repair").await?;

        let mut report = PhaseReport::completed(PhaseSource::ModelAssisted);
        if answer.applicable {
            report.written = answer.written.iter().map(|path| stage_write(path)).collect();
        } else {
            report.outcome = PhaseOutcome::NotApplicable;
        }
        Ok(report)
    }

    async fn review<P: Model>(
        _model: &P, _ctx: &Context<'_>, _slice: &str, _continuation: Option<&[u8]>,
        _workspace: &Workspace,
    ) -> Result<PhaseReport, Error> {
        // Contracts carries no standards-review team: a typed
        // non-applicable report, no judgment leg (RFC-90 D7).
        Ok(PhaseReport::not_applicable())
    }

    async fn merge<P: Model>(
        model: &P, ctx: &Context<'_>, slice: &str, phase: MergePhase, workspace: &Workspace,
    ) -> Result<Report, Error> {
        // Both gates validate change-tree and baseline state through the
        // adapter's own `"."` preopen — the lent read-only result view
        // carries product code only.
        if phase == MergePhase::Preflight {
            let staged = ctx.project_root.join(format!(".emery/slices/{slice}/contracts"));
            return Ok(enforce_validators(Report::success(), &validate_baseline(&staged)));
        }

        let baseline = ctx.project_root.join("contracts");
        let merge_prompt = registry::body("prompts/merge.md");

        // Clean baseline → deterministic success; otherwise one repair leg.
        let mut report = Report::success();
        let mut findings = validate_baseline(&baseline);
        if !findings.is_empty() {
            let user = format!(
                "The postflight contract validators found blocking issues in the merged \
             `contracts/` baseline at `{}` (slice `{slice}`, adapter `{}`) — the \
             baseline lives in the project tree outside your read-only workspace \
             view. The engine has already promoted the slice's delta and archived \
             the slice. Repair the baseline files in place, then answer with the \
             corrected report body.\n\n{}",
                workspace.artifact_path("contracts"),
                ctx.adapter_id,
                render_validator_findings(&findings),
            );
            report = phase::report(model, ctx, merge_prompt.to_string(), user).await?;
            findings = validate_baseline(&baseline);
        }

        Ok(enforce_validators(report, &findings))
    }
}

// Build-loop operations receive a lent stage; its absence is a
// malformed dispatch, not a skippable input.
fn artifact_stage(workspace: &Workspace) -> Result<&ArtifactStage, Error> {
    workspace.artifact_stage.as_ref().ok_or_else(|| {
        Error::InvalidRequest("contracts build-loop operations require an artifact stage".into())
    })
}

// The staged contract delta's deployment-local (agent-visible) path.
fn stage_contracts(stage: &ArtifactStage) -> String {
    format!("{}/contracts", stage.root)
}

fn has_entries(dir: &std::path::Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|mut entries| entries.next().is_some())
}

fn stage_write(path: &str) -> PhaseWrite {
    PhaseWrite {
        root: PhaseRoot::Artifacts,
        path: path.to_string(),
    }
}

// Findings that route nowhere pull in every sub-prompt.
fn owning_sub_prompts(findings: &[PhaseFinding]) -> String {
    let owns = |sub_flow: &SubFlow, finding: &PhaseFinding| {
        finding
            .location
            .as_ref()
            .is_some_and(|location| location.path.contains(&format!("contracts/{}/", sub_flow.dir)))
    };
    let unrouted =
        findings.iter().any(|finding| !SUB_FLOWS.iter().any(|sub_flow| owns(sub_flow, finding)));
    let mut inlined = String::new();
    for sub_flow in &SUB_FLOWS {
        if unrouted || findings.iter().any(|finding| owns(sub_flow, finding)) {
            inlined.push_str("\n\n---\n\n");
            inlined.push_str(registry::body(sub_flow.prompt));
        }
    }
    inlined
}

// One in-guest validator finding as a deterministic phase finding; the
// location is the finding's stage-relative path (the slice-relative
// form of the staged file).
fn phase_finding(finding: &ContractFinding, stage_root: &std::path::Path) -> PhaseFinding {
    let path = finding.path.strip_prefix(stage_root).unwrap_or(&finding.path);
    PhaseFinding {
        id: String::new(),
        rule_id: Some(finding.rule_id.to_string()),
        related_rule_ids: Vec::new(),
        title: format!("contract validator: {}", finding.rule_id),
        severity: Severity::Important,
        source: DiagnosticSource::Deterministic,
        kind: FindingKind::Violation,
        artifact: FindingArtifact::Contracts,
        location: Some(PhaseLocation {
            path: path.display().to_string(),
            ..PhaseLocation::default()
        }),
        evidence: FindingEvidence::Snippet {
            value: finding.detail.clone(),
        },
        impact: finding.detail.clone(),
        remediation: "Repair the contract document so the in-guest validator passes; the \
                      identity and version rules live in `references/contract-identity.md`."
            .to_string(),
        confidence: Some(FindingConfidence::High),
        fingerprint: String::new(),
    }
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
