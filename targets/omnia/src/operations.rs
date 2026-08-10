//! The six target operations over shared [`phase`] scaffolding.
//!
//! Build is generation only (RFC-90 D3): preparation (exemplar
//! checkout) → deterministic scaffold → generation → replay
//! (dispatched only when the build context binds `captures`) →
//! close-out (staged `tasks.md` checkboxes, output declaration,
//! findings synthesis). `verify`, `repair`, and `review` are sibling
//! single-pass operations — order, retries, and budgets are engine
//! policy (RFC-90 D1), so no operation loops or selects a successor.

use std::path::Path;

use adapter::registry::Doc;
use adapter::seam::{
    BuildContext, Context, DiagnosticSource, Error, Finding, Input, MergePhase, PhaseFinding,
    PhaseOutcome, PhaseReport, PhaseSource, RepairOrigin, Report, TargetMetadata, Workspace,
    WritableArtifact,
};
use adapter::{AdapterIdentity, Model, Target, phase};

use crate::registry;

const REFERENCES_POINTER: &str = "Every prompt, reference, and rule document this adapter ships is \
     served by the granted `omnia-references` MCP references (`list_docs` / `read_doc`, adapter-relative \
     paths like `references/guardrails.md`); fetch documents the prompts cite lazily from there.";

/// Rust crates, tests, and guest scaffolding for Omnia deployments.
#[derive(Clone, Copy, Debug)]
pub struct Adapter;

impl Target for Adapter {
    const IDENTITY: AdapterIdentity = AdapterIdentity {
        name: "omnia",
        version: env!("CARGO_PKG_VERSION"),
    };

    fn metadata() -> TargetMetadata {
        TargetMetadata {
            emery_floor: Some("0.38.0".to_string()),
            inputs: Vec::new(),
            platforms: None,
            // The only slice artifact omnia's build writes: tasks.md
            // checkbox close-out, routed onto the lent artifact stage.
            writable_artifacts: vec![WritableArtifact::file("tasks.md")],
        }
    }

    fn docs() -> &'static [Doc] {
        registry::docs()
    }

    async fn guidance<P: Model>(_model: &P, _ctx: &Context<'_>) -> Result<String, Error> {
        Ok(registry::body("prompts/guidance.md").to_string())
    }

    async fn build<P: Model>(
        model: &P, ctx: &Context<'_>, slice: &str, inputs: &[Input], context: &BuildContext,
        workspace: &Workspace,
    ) -> Result<PhaseReport, Error> {
        let workspace_root = workspace.root_path();
        let inputs_block = phase::render_inputs(inputs, workspace);

        // The agent prepares the read-only exemplar checkout the rest of
        // the build reads: scaffold templates, worked code, and the Omnia
        // compatibility contract all come from it.
        let system = assemble(&["prompts/build.md", "prompts/build/prepare.md"]);
        let user = format!(
            "Run the preparation leg of the omnia build for slice `{slice}` \
         (adapter `{}`): prepare the read-only exemplar checkout at \
         `target/omnia-exemplar/` in the lent workspace per the preparation prompt — \
         clone or refresh unpinned `main`, keep an existing checkout when only the \
         refresh fails (note the staleness in your summary), and surface the stop \
         hint per the build prompt's `## § Stop hint contract` in your summary when \
         no checkout can be obtained. {REFERENCES_POINTER}",
            ctx.adapter_id,
        );
        let preparation = phase::phase(model, ctx, system, user, "preparation").await?;

        // Deterministic base-repo prelude: strictly validate the prepared
        // checkout's template contract, then fill any missing standard
        // tooling file from it. A missing or malformed checkout or an I/O
        // failure aborts the build here — the agent must never recreate
        // deterministic files from prose.
        let scaffold_block = scaffold_prelude(workspace_root)?;

        // One writer leg holds crate / test / guest together. `guidance.md`
        // stays on the `guidance` operation only — its idioms were folded
        // into the artifacts at refine. The guest writer ships only in
        // create mode: keyed on the workspace-root `src/lib.rs` the
        // prelude's tree walk already sees.
        let create_mode = !workspace_root.join("src").join("lib.rs").is_file();
        let mut generation_prompts =
            vec!["prompts/build.md", "prompts/build/crate.md", "prompts/build/test.md"];
        if create_mode {
            generation_prompts.push("prompts/build/guest.md");
        }
        let system = assemble(&generation_prompts);
        let user = format!(
            "Run the generation leg of the omnia build for slice `{slice}` \
         (adapter `{}`).\n\n\
         The project workspace is lent to you. The read-only exemplar checkout at \
         `target/omnia-exemplar/` is already prepared and validated — read it per \
         `references/exemplar.md`. Detect create vs update mode per the \
         build prompt's `## Mode detection`, then follow the crate-writer, \
         test-writer, and (create mode only) guest-writer prompts. Write code only: \
         do not run the check suite or fix-and-recheck — verification, repair, and \
         standards review are separate engine-dispatched operations. Guidance idioms \
         were already folded into the slice artifacts at refine; re-read `design.md` \
         and the specs, and fetch `references/guardrails.md` via MCP if needed. \
         {REFERENCES_POINTER}\n\n{scaffold_block}\n\n{inputs_block}",
            ctx.adapter_id,
        );
        let generation = phase::phase(model, ctx, system, user, "generation").await?;

        // Whether the slice binds `captures` is deterministic — the
        // engine forwards the bound source names on the build context —
        // so the leg is dispatched only when bound; no spawn exists to
        // answer `applicable: false`. It runs before close-out so the
        // close-out's findings synthesis can fold replay failures.
        let replay = if context.sources.iter().any(|source| source == "captures") {
            let system = assemble(&["prompts/build.md", "prompts/build/replay.md"]);
            let user = format!(
                "Run the capture-replay leg of the omnia build for slice `{slice}` — \
             the slice binds the `captures` source. Follow the replay prompt and \
             classify results in your summary; the close-out leg folds unresolved \
             replay failures into the build report's findings. {REFERENCES_POINTER}",
            );
            Some(phase::phase(model, ctx, system, user, "replay").await?)
        } else {
            None
        };

        // Close-out answers the phase report: staged tasks.md checkbox
        // marking, output declaration, and the generation pass's findings
        // synthesis. Verification and standards review are NOT run here —
        // the engine dispatches them as separate operations.
        let mut outcomes = vec![
            phase::render_outcome("preparation", &preparation),
            phase::render_outcome("generation", &generation),
        ];
        outcomes.push(replay.as_ref().map_or_else(
            || "- replay: skipped in-guest — the slice binds no `captures` source".to_string(),
            |answer| phase::render_outcome("replay", answer),
        ));
        let stage_sentence = workspace.artifact_stage.as_ref().map_or_else(
            || {
                "No artifact stage was lent: skip the tasks.md checkbox close-out and note \
                 the skip in your summary."
                    .to_string()
            },
            |stage| {
                format!(
                    "Mark the completed `tasks.md` checkboxes in the stage copy at \
                     `{}/tasks.md` — never in the authoritative slice tree.",
                    stage.root,
                )
            },
        );
        let system = assemble(&["prompts/build.md", "prompts/build/report.md"]);
        let user = format!(
            "Run the close-out leg of the omnia build for slice `{slice}` per the \
         report prompt. {stage_sentence} Declare the slice's crate tree (and the \
         guest scaffolding, when this build wrote it) as `platform: core` outputs \
         with paths relative to the workspace root — only paths the workspace \
         actually contains. Synthesise the generation pass's findings — a stale or \
         missing exemplar checkout, gaps the writers could not close, unresolved \
         capture-replay failures — into `findings[]`; do not run the check suite or \
         review standards here, those passes are separate engine-dispatched \
         operations. Answer with the phase report: `outcome: completed`, `source: \
         model-assisted`, and `written` entries for what this build touched (root \
         `artifacts` for the staged tasks.md, root `workspace` for product code). \
         {REFERENCES_POINTER}\n\nPhase outcomes:\n{}",
            outcomes.join("\n"),
        );
        phase::phase_report(model, ctx, system, user, "report").await
    }

    async fn verify<P: Model>(
        model: &P, ctx: &Context<'_>, _workspace: &Workspace,
    ) -> Result<PhaseReport, Error> {
        let system = registry::body("prompts/verify.md").to_string();
        let user = format!(
            "Run one omnia verification pass over the lent candidate workspace \
         (adapter `{}`): execute the verify prompt's check commands from the \
         workspace root — the cargo / clippy / test commands run in the lent \
         workspace; this adapter cannot spawn them. One pass only: report every \
         failure as a finding with a file location where the output names one, and \
         fix nothing — repair is a separate engine-dispatched operation. Answer \
         with the phase report (`source: model-assisted`, empty `outputs`, no \
         `ui-surface`). {REFERENCES_POINTER}",
            ctx.adapter_id,
        );
        Ok(check_pass(phase::phase_report(model, ctx, system, user, "verify").await?))
    }

    async fn repair<P: Model>(
        model: &P, ctx: &Context<'_>, slice: &str, origin: RepairOrigin, findings: &[PhaseFinding],
        _continuation: Option<&[u8]>, _workspace: &Workspace,
    ) -> Result<PhaseReport, Error> {
        let system = registry::body("prompts/repair.md").to_string();
        let user = format!(
            "Run one omnia repair pass for slice `{slice}` (adapter `{}`) with \
         `repair-origin: {}`: follow the repair prompt's branch for that origin and \
         apply the minimum change that clears each finding below. One pass only — \
         do not re-run the full check suite, re-review, or loop; the engine \
         re-verifies after this pass. Answer with the phase report (`source: \
         model-assisted`, empty `outputs`, no `ui-surface`). \
         {REFERENCES_POINTER}\n\nFindings to repair:\n\n{}",
            ctx.adapter_id,
            origin.as_str(),
            phase::render_findings(findings),
        );
        Ok(check_pass(phase::phase_report(model, ctx, system, user, "repair").await?))
    }

    async fn review<P: Model>(
        model: &P, ctx: &Context<'_>, slice: &str, _continuation: Option<&[u8]>,
        _workspace: &Workspace,
    ) -> Result<PhaseReport, Error> {
        let system = registry::body("prompts/review.md").to_string();
        let user = format!(
            "Run one omnia standards-review pass for slice `{slice}` (adapter \
         `{}`): spawn the review team per the review prompt, synthesise \
         `REVIEW.md`, and report the findings. One pass only — no remediation \
         cycle and no auto-fix; the engine routes blocking findings through a \
         separate repair operation under its own budget. Answer with the phase \
         report (`source: model-assisted`, empty `outputs`, no `ui-surface`). \
         {REFERENCES_POINTER}",
            ctx.adapter_id,
        );
        Ok(check_pass(phase::phase_report(model, ctx, system, user, "review").await?))
    }

    async fn merge<P: Model>(
        model: &P, ctx: &Context<'_>, slice: &str, phase: MergePhase, workspace: &Workspace,
    ) -> Result<Report, Error> {
        // No merged-baseline validator — postflight is deterministic success.
        if phase == MergePhase::Postflight {
            return Ok(Report::success());
        }

        let merge_prompt = registry::body("prompts/merge.md");

        let user = format!(
            "Run the preflight merge gate for slice `{slice}` (adapter `{}`). A view of \
         the built result snapshot is lent to you as your workspace; nothing you write \
         there is captured, and the engine folds the slice's spec deltas only after \
         this gate passes. Run the merge prompt's `## § Omnia pre-merge gate` yourself \
         — the cargo / clippy / test / wasm32-wasip2 commands run in the lent \
         workspace; this adapter cannot spawn them. Any gate failure means \
         `status: failure`. Answer with the report body. {REFERENCES_POINTER}",
            ctx.adapter_id,
        );
        let report = phase::report(model, ctx, merge_prompt.to_string(), user).await?;

        gate_report(model, ctx, merge_prompt, report, workspace.root_path(), "merge-preflight")
            .await
    }
}

fn assemble(prompts: &[&str]) -> String {
    let bodies: Vec<&str> = prompts.iter().map(|prompt| registry::body(prompt)).collect();
    phase::assemble_system(&bodies)
}

/// Postlude shared by the check passes (`verify` / `repair` /
/// `review`): enforce the engine's non-build coherence gates in code
/// rather than trusting prompt prose — empty `outputs`, no UI
/// surface, no continuation change, `model-assisted` attribution (the
/// pass is one model leg; `tool` / `human` finding sources are
/// unreachable claims here), and no declared writes on a
/// `not-applicable` outcome.
fn check_pass(mut report: PhaseReport) -> PhaseReport {
    report.outputs = Vec::new();
    report.ui_surface = None;
    report.next_continuation = None;
    report.source = PhaseSource::ModelAssisted;
    for finding in &mut report.findings {
        if matches!(finding.source, DiagnosticSource::Tool | DiagnosticSource::Human) {
            finding.source = DiagnosticSource::ModelAssisted;
        }
    }
    if report.outcome == PhaseOutcome::NotApplicable && !report.findings.is_empty() {
        report.outcome = PhaseOutcome::Completed;
    }
    if report.outcome == PhaseOutcome::NotApplicable {
        report.written = Vec::new();
    }
    report
}

// A scaffold failure fails the build before model generation: the
// templates are deterministic, so asking the agent to recreate them
// from prose would only produce weaker copies. A missing or malformed
// checkout is equally fatal — the preparation leg must be repaired, not
// papered over.
fn scaffold_prelude(workspace_root: &Path) -> Result<String, Error> {
    use std::fmt::Write as _;

    let report = crate::scaffold::ensure_missing(workspace_root)
        .map_err(|err| Error::Io(format!("base-repo scaffold prelude failed: {err}")))?;

    let mut block = if report.written.is_empty() {
        "### scaffold prelude (already run in-guest)\n\nEvery standard tooling file \
         was already present; nothing was written. Do not re-author them."
            .to_string()
    } else {
        format!(
            "### scaffold prelude (already run in-guest)\n\nThe adapter wrote the missing \
             standard tooling files from the exemplar checkout's template contract:\n{}\n\n\
             Do not re-author or overwrite them.",
            report.written.iter().map(|path| format!("- `{path}`")).collect::<Vec<_>>().join("\n"),
        )
    };
    if !report.unfilled_tokens.is_empty() {
        let tokens = report
            .unfilled_tokens
            .iter()
            .map(|token| format!("`{token}`"))
            .collect::<Vec<_>>()
            .join(" / ");
        let _ = write!(
            block,
            "\n\nUnfilled placeholders still present in `{}`: {tokens}. Fill them before \
             considering the guest scaffolding complete.",
            crate::scaffold::PUBLISH_WORKFLOW,
        );
    }
    if report.written.iter().any(|path| path == crate::scaffold::VET_CONFIG) {
        block.push_str(
            "\n\nRun `cargo vet regenerate {imports,exemptions,unpublished}` once \
             `Cargo.lock` exists.",
        );
    }
    if let Some(mismatch) = &report.pin_mismatch {
        let consumer_version = mismatch.consumer_version.as_deref().unwrap_or("(unparsed)");
        let consumer_rev = mismatch.consumer_rev.as_deref().unwrap_or("(unparsed)");
        let _ = write!(
            block,
            "\n\nSoft warning — Omnia pin mismatch (update mode): consumer \
             `omnia` version `{consumer_version}` / rev `{consumer_rev}` vs exemplar \
             `{}` / `{}`. Preserve the consumer pin; prefer consumer-evidenced idioms \
             over exemplar idioms wherever they conflict.",
            mismatch.exemplar_version, mismatch.exemplar_rev,
        );
    }
    Ok(block)
}

async fn gate_report<P: Model>(
    model: &P, ctx: &Context<'_>, prompt: &str, mut report: Report, workspace_root: &Path,
    operation: &str,
) -> Result<Report, Error> {
    let mut missing = phase::missing_outputs(&report, workspace_root);
    if !missing.is_empty() {
        let user = format!(
            "The deterministic report gate rejected the {operation} report: it claims \
             `status: success` but declares outputs the workspace does not \
             contain.\n\n{}\n\n\
             Repair the workspace or correct the report, then answer with the \
             corrected report body.",
            missing.join("\n"),
        );
        report = phase::report(model, ctx, prompt.to_string(), user).await?;
        missing = phase::missing_outputs(&report, workspace_root);
    }
    Ok(phase::enforce(report, missing.into_iter().map(Finding::blocking).collect()))
}
