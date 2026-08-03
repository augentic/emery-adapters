//! `guidance` / `build` / `merge` over shared [`phase`] scaffolding.
//!
//! Judgment legs sit between a deterministic prepare prelude and a
//! validate / report-coherence postlude. Build order: composition →
//! core → per-shell → review → final-core-verify → report. Host verify
//! stays agent-side.

mod gate;
mod prelude;
mod shells;

use std::path::Path;

use adapter::registry::Doc;
use adapter::seam::{
    BuildContext, BuildInput, Context, Error, Finding, Input, MergePhase, Platform,
    PlatformsCapability, Report, Status, TargetMetadata, WorkingTree,
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
            emery_floor: Some("0.37.0".to_string()),
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
        let tree_root = ctx.tree_root(tree);
        let slice_dir_rel = format!(".emery/slices/{slice}");
        let slice_dir = tree_root.join(&slice_dir_rel);
        let slice_composition = slice_dir.join("composition.yaml");
        let inputs_block = phase::render_inputs(inputs);

        // The materialize scope derives from the same declared-platform
        // read as the shell legs, so a core-only project materializes
        // nothing for shells it will not build.
        let shell_platforms: Vec<String> = shells::declared_shell_legs(&tree_root)
            .iter()
            .map(|leg| leg.name.to_string())
            .collect();
        let prepared = prepare::materialize_step(&slice_dir, &tree_root, &shell_platforms)
            .map_err(error_from_vectis)?;
        let prelude_block = prelude::render_prelude(&prepared);

        // Bootstrap gate (§L): the launcher app-icon must be satisfiable for
        // every declared UI platform before any write leg.
        let bootstrap = gate::bootstrap_findings(&tree_root);
        if !bootstrap.is_empty() {
            return Ok(failure_report(bootstrap));
        }

        let composition = composition_leg(
            model,
            ctx,
            slice,
            &slice_dir_rel,
            &tree_root,
            &prelude_block,
            &inputs_block,
        )
        .await?;

        // The per-shell write prompts require the composition gate passed
        // before any platform phase: an exhausted repair budget parks the
        // slice with a deterministic failure report.
        let residual =
            gate::composition_gate(model, ctx, slice, &slice_dir_rel, &slice_composition).await?;
        if !residual.is_empty() {
            return Ok(failure_report(residual));
        }

        if let Err(err) = crate::projections::test_ids::write_generated(&tree_root, Some(slice)) {
            return Ok(failure_report(vec![format!("- [test-id-projection] {err}")]));
        }

        let scaffold_block = prelude::scaffold_missing_trees(&tree_root);
        let core = core_leg(model, ctx, slice, &scaffold_block, &inputs_block).await?;
        let shell_outcomes =
            shells::run_write_legs(model, ctx, slice, &tree_root, &scaffold_block).await?;
        let review = review_leg(model, ctx, slice, &tree_root).await?;
        let final_core = final_core_leg(model, ctx, slice).await?;

        let mut outcomes = vec![("composition", &composition), ("core", &core)];
        outcomes.extend(shell_outcomes.iter().map(|(name, answer)| (*name, answer)));
        outcomes.push(("review", &review));
        outcomes.push(("final-core-verify", &final_core));

        let (report, report_prompt) = report_leg(model, ctx, slice, &tree_root, &outcomes).await?;
        let mut report = gate::gate_report(
            model,
            ctx,
            &report_prompt,
            report,
            &tree_root,
            &slice_composition,
            "build",
            true,
            Some(slice),
        )
        .await?;

        // Suggestion findings only — they ride the report but never fail
        // it or trigger the repair leg.
        let coherence = gate::ui_surface_coherence(&report, &slice_composition);
        report.findings.extend(coherence);
        Ok(report)
    }

    async fn merge<P: Model>(
        model: &P, ctx: &Context<'_>, slice: &str, phase: MergePhase, tree: &WorkingTree,
    ) -> Result<Report, Error> {
        let tree_root = ctx.tree_root(tree);
        let merge_prompt = registry::body("prompts/merge.md");

        if phase == MergePhase::Preflight {
            // Deterministic gate: an invalid staged slice composition blocks
            // the merge before the engine folds it, per the merge prompt.
            let staged = tree_root.join(format!(".emery/slices/{slice}/composition.yaml"));
            let staged_findings = gate::validation_findings(&staged);
            if staged_findings.is_empty() {
                return Ok(Report::success());
            }
            return Ok(failure_report(staged_findings));
        }

        let baseline_composition = tree_root.join(".emery/specs/composition.yaml");
        let user = format!(
            "Run the postflight merge gate for slice `{slice}` (adapter `{}`). The engine \
         has already folded the slice's deltas — including its `composition.yaml` and \
         any operator-curated `tokens.yaml` / `assets.yaml` updates — into the \
         baseline and archived the slice. Run the merge prompt's `## Postflight — \
         host cap-matrix re-verification` yourself: the cargo / make / gradlew \
         commands run in the lent workspace; this adapter cannot spawn them. The \
         composition validator re-runs deterministically in-guest after your answer. \
         Any gate failure means `status: failure`. Answer with the report body. \
         {REFERENCES_POINTER}",
            ctx.adapter_id,
        );
        let report = phase::report(model, ctx, merge_prompt.to_string(), user).await?;

        gate::gate_report(
            model,
            ctx,
            merge_prompt,
            report,
            &tree_root,
            &baseline_composition,
            "merge-postflight",
            false,
            None,
        )
        .await
    }
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
    model: &P, ctx: &Context<'_>, slice: &str, slice_dir_rel: &str, tree_root: &Path,
    prelude_block: &str, inputs_block: &str,
) -> Result<phase::PhaseAnswer, Error> {
    let infer_block = prelude::render_infer_report(tree_root);
    let system = assemble(&["prompts/build.md", "prompts/build/composition.md"]);
    let user = format!(
        "Run component inference (Step 0.5) and composition regeneration (Phase 1) of \
         the vectis build for slice `{slice}` (adapter `{}`).\n\n\
         The project workspace is lent to you. The adapter already ran the \
         deterministic component-identity clustering in-guest — the name-free cluster \
         report is below; do not attempt to re-run it. Decide what each unbound \
         cluster is and what to call it per the composition prompt's Step 0.5, write your \
         `{{ fingerprint -> slug }}` decisions to \
         `{slice_dir_rel}/build/component-bindings.yaml` (echo populated `bound-slug` \
         names verbatim — operator parts carry naming authority), then regenerate \
         `{slice_dir_rel}/composition.yaml` from the slice artifacts per the \
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

// The core verify-repair loop crosses the write and test prompts
// (a cargo failure re-enters the writer), so one agent leg holds
// them together.
async fn core_leg<P: Model>(
    model: &P, ctx: &Context<'_>, slice: &str, scaffold_block: &str, inputs_block: &str,
) -> Result<phase::PhaseAnswer, Error> {
    let system =
        assemble(&["prompts/build.md", "prompts/build/core/write.md", "prompts/build/test.md"]);
    let user = format!(
        "Run the Crux core phases (2-3) of the vectis build for slice `{slice}`: \
         generate or update the shared core per the core write prompt, write the \
         Crux tests, then run the test prompt's mid-build core verify-repair loop \
         yourself — the cargo check / clippy / test commands run in the lent \
         workspace; this adapter cannot spawn them. Do not write \
         `shared/.vectis/verify.ok` here; the final-core-verify leg after review owns \
         that stamp. Detect create vs update mode from the tree. When the \
         template-materialize prelude below lists absent trees, materialize from \
         `$TEMPLATE_DIR` first (host FS) before writing feature code.\n\n\
         {BINDING_NOTE}\n\n{scaffold_block}\n\n{REFERENCES_POINTER}\n\n{inputs_block}",
    );
    phase::phase(model, ctx, system, user, "core").await
}

async fn review_leg<P: Model>(
    model: &P, ctx: &Context<'_>, slice: &str, tree_root: &Path,
) -> Result<phase::PhaseAnswer, Error> {
    let mut review_prompts = vec!["prompts/build.md", "prompts/build/core/review.md"];
    review_prompts
        .extend(shells::declared_shell_legs(tree_root).iter().map(|shell| shell.review_prompt));
    let system = assemble(&review_prompts);
    let user = format!(
        "Run the review phases (6-7) of the vectis build for slice `{slice}`: spawn \
         the core reviewer team and, for each in-scope shell, its platform reviewer \
         team per the review prompts (reviewers run in parallel), then run the core \
         review prompt's `## § Consolidate review findings` and drive any remediation \
         in the lent workspace. {REFERENCES_POINTER}",
    );
    phase::phase(model, ctx, system, user, "review").await
}

// Final core verify-repair after review may have edited `shared/`.
// Mid-build verify in the core leg does not write the durable stamp;
// only this leg refreshes `shared/.vectis/verify.ok`.
async fn final_core_leg<P: Model>(
    model: &P, ctx: &Context<'_>, slice: &str,
) -> Result<phase::PhaseAnswer, Error> {
    let system = assemble(&["prompts/build.md", "prompts/build/test.md"]);
    let user = format!(
        "Run the final core verify-repair pass for slice `{slice}` after review and \
         before the build report. Re-run only Step 6 of the test prompt (fmt / check / \
         clippy / test) against the current tree — no feature writing. When `shared/` \
         exists, always run the four commands unconditionally (not only if review \
         touched core). On success, write `shared/.vectis/verify.ok` containing the \
         digest of `shared/src/**/*.rs` per the test prompt's stamp contract; the \
         mid-build core verify-repair loop must NOT write this stamp. On exhausted \
         repair budget, answer `applicable: true` with a failure summary so the report \
         cannot claim success. {REFERENCES_POINTER}",
    );
    phase::phase(model, ctx, system, user, "final-core-verify").await
}

// The deterministic shell verify gate runs in-guest and feeds the
// report leg, gated by the derived answer schema. The report contract
// (shell verify gate, phase outcome, report shape) lives in the report
// phase prompt, not the shared preamble, so only this leg and its gate
// pay those bytes.
async fn report_leg<P: Model>(
    model: &P, ctx: &Context<'_>, slice: &str, tree_root: &Path,
    outcomes: &[(&str, &phase::PhaseAnswer)],
) -> Result<(Report, String), Error> {
    let verify_block = prelude::render_verify_gate(tree_root, Some(slice));
    let report_prompt = assemble(&["prompts/build.md", "prompts/build/report.md"]);
    let user = format!(
        "Write the build report for slice `{slice}` per the report prompt's `## Build \
         report`. The adapter already ran the deterministic shell verify gate in-guest \
         — its findings are below and re-run after your answer; a missing or empty \
         tree for a supported declared platform forces `status: failure`, so repair \
         the tree first when the gate reports errors. A missing or stale \
         `shared/.vectis/verify.ok` digest stamp (when the core tree is present) also \
         forces `status: failure`. Then mark the completed `tasks.md` checkboxes in \
         the slice directory per the prompt before answering. A `success` report \
         carries only non-blocking findings; an exhausted verify-repair budget, a \
         failed composition gate, or unresolved blocking review findings mean \
         `status: failure`. Declare `outputs[]` per supported platform with paths \
         relative to the project root, and set `ui-surface.screens` from the slice's \
         own screen count.\n\n\
         {verify_block}\n\n\
         Phase outcomes:\n{}",
        outcomes
            .iter()
            .map(|(name, answer)| phase::render_outcome(name, answer))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    let report = phase::report(model, ctx, report_prompt.clone(), user).await?;
    Ok((report, report_prompt))
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
