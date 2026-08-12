//! `guidance` / `build` / `verify` / `repair` / `review` / `merge`
//! over shared [`phase`] scaffolding.
//!
//! RFC-90 split: `build` is generation only (composition → core →
//! per-shell writers, bracketed by the deterministic prepare prelude
//! and validator gate); `verify`, `repair`, and `review` are each one
//! engine-dispatched pass returning one typed phase report. Operation
//! order, repair routing, and budgets are engine policy — nothing here
//! loops or selects the next operation.

mod finding;
mod gate;
mod prelude;
mod shells;

use std::path::{Path, PathBuf};

use adapter::registry::Doc;
use adapter::seam::{
    BuildContext, BuildInput, Context, DiagnosticSource, Error, Finding, FindingArtifact, Input,
    MergePhase, PhaseFinding, PhaseOutcome, PhaseReport, PhaseSource, Platform,
    PlatformsCapability, RepairOrigin, Report, Status, TargetMetadata, Workspace, WritableArtifact,
};
use adapter::{AdapterIdentity, Model, Target, phase};

use crate::{VectisError, prepare, registry};

const REFERENCES_POINTER: &str = "Every prompt, reference, and rule document this adapter ships is \
     served by the granted `vectis-references` MCP references (`list_docs` / `read_doc`, \
     adapter-relative paths like `references/hard-rules-core.md` or \
     `prompts/build/ios/write.md`); fetch documents the prompts cite lazily from there.";

/// Host-FS bootstrap contract for greenfield trees (typescript-style).
///
/// The target guest only mounts the project root, so a sibling
/// `../vectis-exemplar` is invisible in-guest. The build agent performs
/// the allowlisted copy by hand on the host filesystem, following
/// `references/template-materialize.md`.
const BINDING_NOTE: &str = "Resolve `$TEMPLATE_DIR` before any greenfield write: default \
                            `../vectis-exemplar` relative to the consumer project root, or the \
                            absolute path in `VECTIS_EXEMPLAR_DIR`. Clone \
                            https://github.com/augentic/vectis-exemplar.git if missing — fail \
                            closed; do not invent a scaffold or version pins. This is **template \
                            materialize** (the allowlisted copy procedure in \
                            `references/template-materialize.md`), not asset materialize (the \
                            in-guest prepare prelude). Strip grammar: `$TEMPLATE_DIR/AGENTS.md` \
                            (not copied). Late-cap adoption: \
                            `references/template-capabilities.md` plus that AGENTS.md.";

/// Crux shared cores plus `SwiftUI` / Jetpack Compose shells.
#[derive(Clone, Copy, Debug)]
pub struct Adapter;

impl Target for Adapter {
    const IDENTITY: AdapterIdentity = AdapterIdentity {
        name: "vectis",
        version: env!("CARGO_PKG_VERSION"),
    };

    fn metadata() -> TargetMetadata {
        let optional = |path: &str| BuildInput {
            path: path.to_string(),
            required: false,
        };
        TargetMetadata {
            emery_floor: Some("0.38.0".to_string()),
            inputs: vec![
                optional("tokens.yaml"),
                optional("assets.yaml"),
                optional("components.yaml"),
            ],
            platforms: Some(PlatformsCapability {
                required: true,
                allowed: vec![
                    Platform::Core,
                    Platform::Ios,
                    Platform::Android,
                    Platform::Web,
                    Platform::Desktop,
                ],
                default: vec![Platform::Core, Platform::Ios, Platform::Android],
            }),
            // RFC-90 D5: the task list, the regenerated slice
            // composition, and the build bookkeeping subtree
            // (`build/component-bindings.yaml`).
            writable_artifacts: vec![
                WritableArtifact::file("tasks.md"),
                WritableArtifact::file("composition.yaml"),
                WritableArtifact::tree("build"),
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
        // RFC-87/90 split: non-slice change-tree context (`.emery/*`)
        // reads through the adapter's own `"."` preopen; product code
        // lives in the prepared private workspace; every target-authored
        // slice-artifact read and write routes through the writable
        // artifact stage.
        let change_root = ctx.project_root;
        let code_root = workspace.root_path();
        let stage = slice_stage(workspace, ctx, slice);
        let slice_composition = stage.fs.join("composition.yaml");
        let inputs_block = phase::render_inputs(inputs, workspace);

        // Bootstrap gate (§L): the launcher app-icon must be satisfiable for
        // every declared UI platform before any filesystem effect —
        // including the materialize prelude — so a blocked build is
        // side-effect-free. Purely deterministic, no model leg has run.
        let bootstrap_ran = change_root.join(".emery/project.yaml").exists();
        let bootstrap = gate::bootstrap_findings(change_root, code_root);
        if !bootstrap.is_empty() {
            return Ok(deterministic_blocked(bootstrap));
        }

        // The materialize scope derives from the same declared-platform
        // read as the shell legs, so a core-only project materializes
        // nothing for shells it will not build. Exports land beside the
        // workspace's design-system baseline, so capture records them.
        let shell_platforms: Vec<String> = shells::declared_shell_legs(change_root)
            .iter()
            .map(|leg| leg.name.to_string())
            .collect();
        let prepared = prepare::materialize_step(&stage.fs, code_root, &shell_platforms)
            .map_err(error_from_vectis)?;
        let prelude_block = prelude::render_prelude(&prepared);

        let composition = composition_leg(
            model,
            ctx,
            slice,
            &stage.agent,
            change_root,
            &prelude_block,
            &inputs_block,
        )
        .await?;

        // The per-shell write prompts require a valid staged composition
        // before any platform phase: blocking validator findings park
        // generation here and ride the build report — repair routing is
        // engine policy, never an in-build loop.
        let (validator_ran, composition_findings) =
            finding::composition_findings(&slice_composition);
        if !composition_findings.is_empty() {
            let mut report = PhaseReport::completed(PhaseSource::Hybrid);
            report.findings = composition_findings;
            return Ok(report);
        }

        let scaffold_block = prelude::scaffold_missing_trees(change_root, code_root);
        let core = core_leg(model, ctx, slice, &scaffold_block, &inputs_block).await?;

        if let Err(err) = crate::projections::test_id_registry::write_generated(
            change_root,
            code_root,
            Some(&slice_composition),
        ) {
            return Ok(deterministic_blocked(vec![format!("[test-id-projection] {err}")]));
        }

        let shell_outcomes =
            shells::run_write_legs(model, ctx, slice, change_root, &scaffold_block).await?;

        let mut outcomes = vec![("composition", &composition), ("core", &core)];
        outcomes.extend(shell_outcomes.iter().map(|(name, answer)| (*name, answer)));

        let mut report = report_leg(model, ctx, slice, &stage.agent, &outcomes).await?;

        // Re-run the validator once after the writers — the core leg may
        // patch `# GAP` comments in the staged composition in place.
        let (revalidated, residual) = finding::composition_findings(&slice_composition);
        report.findings.extend(residual);
        // Suggestion findings only — they ride the report but never
        // block it.
        report
            .findings
            .extend(finding::ui_surface_coherence(report.ui_surface, &slice_composition));
        let deterministic_ran = bootstrap_ran || validator_ran || revalidated;
        report.source =
            if deterministic_ran { PhaseSource::Hybrid } else { PhaseSource::ModelAssisted };
        report.next_continuation = None;
        if report.outcome == PhaseOutcome::NotApplicable && !report.findings.is_empty() {
            report.outcome = PhaseOutcome::Completed;
        }
        Ok(report)
    }

    async fn verify<P: Model>(
        model: &P, ctx: &Context<'_>, workspace: &Workspace,
    ) -> Result<PhaseReport, Error> {
        // One check pass, workspace-self-contained: no slice identity is
        // supplied — the candidate slice tree, when one exists, is the
        // lent artifact stage.
        let change_root = ctx.project_root;
        let code_root = workspace.root_path();
        let staged_composition: Option<PathBuf> = workspace
            .artifact_stage
            .as_ref()
            .map(|stage| stage.root_path().join("composition.yaml"));

        let stage_note = workspace.artifact_stage.as_ref().map_or_else(String::new, |stage| {
            format!(
                " The candidate slice-artifact stage is lent at `{}`; read candidate slice \
                 artifacts (e.g. `composition.yaml`) there.",
                stage.root
            )
        });
        let user = format!(
            "Run one vectis verification pass over the lent workspace (adapter `{}`). Work \
             only from the workspace and project context — no slice identity is supplied. \
             Read the declared platform set from `{}` and run the verify prompt's \
             per-platform checks once each yourself — the cargo / make commands run in the \
             lent workspace; this adapter cannot spawn them. One pass only: apply no fixes \
             and no retry loop; report every remaining failure as a structured finding and \
             answer with the phase report. The deterministic in-guest vectis checks re-run \
             after your answer and their findings ride the same report.{stage_note}\n\n\
             {REFERENCES_POINTER}",
            ctx.adapter_id,
            workspace.artifact_path(".emery/project.yaml"),
        );
        let report = phase::phase_report(
            model,
            ctx,
            registry::body("prompts/verify.md").to_string(),
            user,
            "verify",
        )
        .await?;

        let (shell_ran, shell_findings) =
            finding::shell_verify_findings(change_root, code_root, staged_composition.as_deref());
        let (composition_ran, composition_findings) = staged_composition
            .as_deref()
            .map_or((false, Vec::new()), finding::composition_findings);
        let mut deterministic = shell_findings;
        deterministic.extend(composition_findings);
        Ok(check_pass(report, deterministic, shell_ran || composition_ran))
    }

    async fn repair<P: Model>(
        model: &P, ctx: &Context<'_>, slice: &str, origin: RepairOrigin, findings: &[PhaseFinding],
        _continuation: Option<&[u8]>, workspace: &Workspace,
    ) -> Result<PhaseReport, Error> {
        // Vectis carries no writer-session state across passes, so the
        // incoming continuation is ignored and `None` (preserve) is
        // returned.
        let stage = slice_stage(workspace, ctx, slice);
        let brief = phase::render_findings(findings);
        let user = format!(
            "Run one findings-directed repair pass for slice `{slice}` (adapter `{}`). The \
             findings below came from the engine's {origin} gate; repair the lent workspace \
             so they clear — one pass, minimum change, no verify-repair loop (the engine \
             re-verifies after this pass; never run standards review here). Product-code \
             fixes go under the workspace root; candidate slice-artifact fixes (e.g. \
             `composition.yaml`, `build/component-bindings.yaml`) go to the writable \
             artifact stage at `{stage_agent}`. Answer with the phase report describing \
             what was repaired; answer `outcome: not-applicable` only when none of the \
             findings are repairable by this target.\n\n\
             Findings ({origin}):\n\n{brief}\n\n{REFERENCES_POINTER}",
            ctx.adapter_id,
            origin = origin.as_str(),
            stage_agent = stage.agent,
        );
        let report = phase::phase_report(
            model,
            ctx,
            registry::body("prompts/repair.md").to_string(),
            user,
            "repair",
        )
        .await?;
        Ok(check_pass(report, Vec::new(), false))
    }

    async fn review<P: Model>(
        model: &P, ctx: &Context<'_>, slice: &str, _continuation: Option<&[u8]>,
        workspace: &Workspace,
    ) -> Result<PhaseReport, Error> {
        let change_root = ctx.project_root;
        let stage = slice_stage(workspace, ctx, slice);
        let mut review_prompts = vec!["prompts/review.md", "prompts/build/core/review.md"];
        review_prompts.extend(
            shells::declared_shell_legs(change_root).iter().map(|shell| shell.review_prompt),
        );
        let system = assemble(&review_prompts);
        let user = format!(
            "Run one engineering-standards review pass for slice `{slice}` (adapter `{}`): \
             spawn the core reviewer team and, for each in-scope shell, its platform \
             reviewer team per the review prompts (reviewers run in parallel), then run the \
             core review prompt's `## § Consolidate review findings` and answer with the \
             phase report carrying the consolidated structured findings. One pass only: \
             review reports — it never remediates, auto-fixes, or loops; blocking findings \
             route through the engine's repair dispatch. Candidate slice artifacts (e.g. \
             `composition.yaml`) read from the stage at `{stage_agent}`. \
             {REFERENCES_POINTER}",
            ctx.adapter_id,
            stage_agent = stage.agent,
        );
        let report = phase::phase_report(model, ctx, system, user, "review").await?;
        Ok(check_pass(report, Vec::new(), false))
    }

    async fn merge<P: Model>(
        model: &P, ctx: &Context<'_>, slice: &str, phase: MergePhase, _workspace: &Workspace,
    ) -> Result<Report, Error> {
        // Change-tree state (staged and baseline compositions) reads
        // through the `"."` preopen; the lent workspace is a read-only
        // view of the built result code.
        let change_root = ctx.project_root;
        let merge_prompt = registry::body("prompts/merge.md");

        if phase == MergePhase::Preflight {
            // Deterministic gate: an invalid staged slice composition blocks
            // the merge before the engine folds it, per the merge prompt.
            let staged = change_root.join(format!(".emery/slices/{slice}/composition.yaml"));
            let staged_findings = gate::validation_findings(&staged);
            if staged_findings.is_empty() {
                return Ok(Report::success());
            }
            return Ok(failure_report(staged_findings));
        }

        let baseline_composition = change_root.join(".emery/specs/composition.yaml");
        let user = format!(
            "Run the postflight merge gate for slice `{slice}` (adapter `{}`). The engine \
         has already folded the slice's deltas — including its `composition.yaml` and \
         any operator-curated `tokens.yaml` / `assets.yaml` updates — into the \
         baseline and archived the slice. A read-only view of the accepted result \
         snapshot is lent to you as your workspace; nothing you write there is \
         captured. Run the merge prompt's `## Postflight — host cap-matrix \
         re-verification` yourself: the cargo / make / gradlew commands run in the \
         lent workspace; this adapter cannot spawn them. The composition validator \
         re-runs deterministically in-guest after your answer. Any gate failure means \
         `status: failure`. Answer with the report body. {REFERENCES_POINTER}",
            ctx.adapter_id,
        );
        let report = phase::report(model, ctx, merge_prompt.to_string(), user).await?;

        gate::merge_gate(model, ctx, merge_prompt, report, &baseline_composition).await
    }
}

/// The slice roots one build-loop operation works against: the
/// in-guest filesystem path (`fs`) plus the agent-visible path
/// (`agent`) rendered into prompts. The writable artifact stage when
/// the engine lent one (RFC-90 D5); the authoritative slice tree only
/// in the degenerate stage-less shape.
struct SliceRoots {
    fs: PathBuf,
    agent: String,
}

fn slice_stage(workspace: &Workspace, ctx: &Context<'_>, slice: &str) -> SliceRoots {
    workspace.artifact_stage.as_ref().map_or_else(
        || {
            let relative = format!(".emery/slices/{slice}");
            SliceRoots {
                fs: ctx.project_root.join(&relative),
                agent: workspace.artifact_path(&relative),
            }
        },
        |stage| SliceRoots {
            fs: stage.root_path().to_path_buf(),
            agent: stage.root.clone(),
        },
    )
}

/// A completed, purely deterministic build report blocked before any
/// model leg ran.
fn deterministic_blocked(details: Vec<String>) -> PhaseReport {
    let mut report = PhaseReport::completed(PhaseSource::Deterministic);
    report.findings = details
        .into_iter()
        .map(|detail| {
            finding::violation(
                detail,
                FindingArtifact::Code,
                "fix the reported precondition, then re-run `emery plan execute`",
            )
        })
        .collect();
    report
}

/// Postlude shared by the check passes (`verify` / `repair` /
/// `review`): fold the deterministic findings in, stamp the
/// report-level assurance source (`hybrid` when an in-guest check
/// contributed alongside the model leg), and enforce the non-build
/// coherence rules — empty outputs, no UI surface, no continuation
/// change, no `tool`/`human` finding attributions, no brief-derived
/// `deterministic` finding attributions on a model-only pass, and no
/// declared writes on a `not-applicable` outcome.
fn check_pass(
    mut report: PhaseReport, deterministic: Vec<PhaseFinding>, deterministic_ran: bool,
) -> PhaseReport {
    report.findings.extend(deterministic);
    report.outputs = Vec::new();
    report.ui_surface = None;
    report.next_continuation = None;
    report.source =
        if deterministic_ran { PhaseSource::Hybrid } else { PhaseSource::ModelAssisted };
    for finding in &mut report.findings {
        let mislabeled = matches!(finding.source, DiagnosticSource::Tool | DiagnosticSource::Human)
            || (!deterministic_ran && finding.source == DiagnosticSource::Deterministic);
        if mislabeled {
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

fn failure_report(findings: Vec<String>) -> Report {
    Report {
        status: Status::Failure,
        findings: findings.into_iter().map(Finding::blocking).collect(),
        outputs: Vec::new(),
        ui_surface: None,
    }
}

// Component *identity* is deterministic and runs in-guest (the
// name-free cluster report); *naming* is the leg's judgment,
// recorded as a bindings file the workflow's deterministic bind
// bookkeeping projects into the catalog. `guidance.md` stays on the
// `guidance` operation only — its idioms were folded into the
// artifacts at refine.
async fn composition_leg<P: Model>(
    model: &P, ctx: &Context<'_>, slice: &str, stage_agent: &str, change_root: &Path,
    prelude_block: &str, inputs_block: &str,
) -> Result<phase::PhaseAnswer, Error> {
    let infer_block = prelude::render_infer_report(change_root);
    let system = assemble(&["prompts/build.md", "prompts/build/composition.md"]);
    let user = format!(
        "Run component inference (Step 0.5) and composition regeneration (Phase 1) of \
         the vectis build for slice `{slice}` (adapter `{}`).\n\n\
         A private workspace is lent to you; the slice's candidate artifacts live in \
         the writable artifact stage at `{stage_agent}/` — every slice-artifact read \
         and write goes there, never to the authoritative slice tree. The adapter \
         already ran the deterministic component-identity clustering in-guest — the \
         name-free cluster report is below; do not attempt to re-run it. Decide what \
         each unbound cluster is and what to call it per the composition prompt's \
         Step 0.5, write your `{{ fingerprint -> slug }}` decisions to \
         `{stage_agent}/build/component-bindings.yaml` (echo populated `bound-slug` \
         names verbatim — operator parts carry naming authority), then regenerate \
         `{stage_agent}/composition.yaml` from the slice artifacts per the \
         composition prompt, treating your fresh bindings plus the existing catalog \
         as the effective component set. Guidance idioms were already folded into the \
         slice artifacts at refine; re-read `design.md` and the specs. For a slice \
         with no UI surface, write no composition and answer with \
         `applicable: false`.\n\n\
         {infer_block}\n\n\
         {prelude_block}\n\n{REFERENCES_POINTER}\n\n{inputs_block}",
        ctx.adapter_id,
    );
    phase::phase(model, ctx, system, user, "composition").await
}

// The core leg writes the shared core and its Crux tests; running the
// fmt / clippy / test commands is the engine-dispatched verify
// operation's pass, not part of generation.
async fn core_leg<P: Model>(
    model: &P, ctx: &Context<'_>, slice: &str, scaffold_block: &str, inputs_block: &str,
) -> Result<phase::PhaseAnswer, Error> {
    let system =
        assemble(&["prompts/build.md", "prompts/build/core/write.md", "prompts/build/test.md"]);
    let user = format!(
        "Run the Crux core write phases (2-3) of the vectis build for slice `{slice}`: \
         generate or update the shared core per the core write prompt, then write the \
         Crux tests per the test prompt. This is a generation-only pass: you may run \
         `cargo check` as a smoke gate while writing, but do not run a verify-repair \
         loop and do not write any `.vectis/verify.ok` stamp — the engine dispatches \
         a separate verify operation. Detect create vs update mode from the tree. \
         When the template-materialize prelude below lists absent trees, materialize \
         from `$TEMPLATE_DIR` first (host FS) before writing feature code.\n\n\
         {BINDING_NOTE}\n\n{scaffold_block}\n\n{REFERENCES_POINTER}\n\n{inputs_block}",
    );
    phase::phase(model, ctx, system, user, "core").await
}

// The build phase report is the generation phase's own typed answer;
// the engine — not this leg — assembles the terminal build report.
async fn report_leg<P: Model>(
    model: &P, ctx: &Context<'_>, slice: &str, stage_agent: &str,
    outcomes: &[(&str, &phase::PhaseAnswer)],
) -> Result<PhaseReport, Error> {
    let system = assemble(&["prompts/build.md", "prompts/build/report.md"]);
    let user = format!(
        "Write the build phase report for slice `{slice}` per the report prompt. This \
         is the generation phase's own typed report — the engine assembles the \
         terminal build report; never write `build/report.yaml` or any report file. \
         First mark the completed `tasks.md` checkboxes at `{stage_agent}/tasks.md` \
         (the writable artifact stage) per the prompt. Declare `outputs[]` as the \
         per-platform tree paths this build produced or maintained, relative to the \
         workspace root (e.g. `shared/`, `iOS/`, `Android/`), and set \
         `ui-surface.screens` from the slice's own screen count. Report \
         `outcome: completed` and carry a structured blocking finding for any write \
         leg that failed or was left incomplete. Verification, repair, and standards \
         review are separate engine-dispatched operations — do not run them or \
         anticipate their results here.\n\n\
         Phase outcomes:\n{}",
        outcomes
            .iter()
            .map(|(name, answer)| phase::render_outcome(name, answer))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    phase::phase_report(model, ctx, system, user, "build-report").await
}

fn assemble(prompts: &[&str]) -> String {
    let bodies: Vec<&str> = prompts.iter().map(|prompt| registry::body(prompt)).collect();
    phase::assemble_system(&bodies)
}

fn error_from_vectis(err: VectisError) -> Error {
    match err {
        VectisError::Io(io) => Error::Io(io.to_string()),
        VectisError::InvalidProject { message } => Error::InvalidRequest(message),
        VectisError::Internal { message } => Error::Internal(message),
    }
}
