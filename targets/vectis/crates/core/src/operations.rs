//! The judgment operation template: `guidance`, `build`, and `merge`,
//! over the shared [`phase`] scaffolding.
//!
//! Unlike omnia, vectis brackets the legs with the absorbed libraries:
//! the [`crate::prepare`] materialize step runs as the deterministic
//! *prelude* (replacing the legacy `adapter.yaml` `prepare.argv` hook),
//! and the [`crate::validate`] composition / tokens / assets
//! cross-checks run as the deterministic *postlude*, feeding a bounded
//! repair loop the way the contracts adapter's validators do.
//!
//! `build` decomposes along the build brief's own phase order: one
//! *composition* leg (Step 0.5 component inference plus Phase 1
//! composition regeneration) gated in-core by the composition validator,
//! one *core* leg (Phases 2–3), one *shell* leg per declared shell
//! platform (Phases 4–5), one *review* leg (Phases 6–7), then one report
//! leg (Phases 8–9). Host-command verification (cargo, xcodebuild,
//! Gradle, `specify extension run vectis -- …`) is process-spawning and
//! stays agent-side in the prompts; the deterministic tail checks what
//! the in-core validators and pure Rust over the mounted tree can.

use std::path::Path;

use serde_json::Value;
use specify_guest_kit::seam::{
    Changeset, Context, Error, Finding, Input, Report, Status, WorkingTree,
};
use specify_guest_kit::{Model, phase};

use crate::{VectisError, prepare, registry, validate};

/// Maximum composition validator repair iterations after the
/// composition leg, mirroring the contracts build's Phase 4 budget.
const MAX_VALIDATE_REPAIR_ITERATIONS: usize = 2;

/// The pointer at the adapter's own MCP reference shelf every judgment
/// leg's user prompt carries, so prompts stay lean and the agent fetches
/// specialist material lazily instead of getting it inlined.
const SHELF_POINTER: &str = "Every brief, reference, and rule document this adapter ships is \
     served by the granted `vectis-references` MCP shelf (`list_docs` / `read_doc`, \
     adapter-relative paths like `references/hard-rules-core.md` or \
     `briefs/build/ios/write.md`); fetch documents the briefs cite lazily from there.";

/// Guidance on the expected build artifacts for this target — the
/// embedded shape brief, returned deterministically (no judgment leg).
#[must_use]
pub fn guidance() -> &'static str {
    registry::body("briefs/shape.md")
}

/// Build a slice's Crux core, shell code, and regenerated
/// `composition.yaml` per the build brief's phase order:
///
/// 1. **Prelude (deterministic)** — [`prepare::materialize_step`]:
///    RFC §2.1 scope resolution plus the conditional scoped
///    `materialize assets` run. Its summary feeds the composition leg.
/// 2. **Composition leg** (Step 0.5 + Phase 1), then the in-core
///    composition validator gate with a bounded repair loop — an
///    exhausted budget parks the slice instead of burning the
///    downstream legs against a knowingly-broken composition.
/// 3. **Core leg** (Phases 2–3), one **shell leg** per declared shell
///    platform (Phases 4–5), then the **review leg** (Phases 6–7).
/// 4. One report leg (Phases 8–9), with the agent-run shell verify gate
///    instructed in its prompt.
/// 5. **Postlude (deterministic)** — the composition cross-checks re-run
///    in core plus the report-coherence walk, with one bounded repair
///    leg; residual findings force `failure` regardless of the answer.
///
/// # Errors
///
/// As [`specify_guest_kit::judgment`], plus [`Error::Io`] /
/// [`Error::InvalidRequest`] when the deterministic prelude cannot read
/// the workspace's design-system inputs.
pub async fn build<P: Model>(
    model: &P, ctx: &Context<'_>, slice: &str, inputs: &[Input], tree: &WorkingTree,
) -> Result<Report, Error> {
    let tree_root = ctx.tree_root(tree);
    let slice_dir_rel = format!(".specify/slices/{slice}");
    let slice_dir = tree_root.join(&slice_dir_rel);
    let slice_composition = slice_dir.join("composition.yaml");
    let inputs_block = phase::render_inputs(inputs);
    let build_brief = registry::body("briefs/build.md");

    // Deterministic prelude — prepare scope resolution + conditional
    // materialize over the effective assets.yaml, in-guest. The
    // host-bootstrap legs the legacy hook also ran (app-icon verify gate,
    // Android Gradle setup, iOS scaffold sync) are process-adjacent and
    // ride agent-side in the shell legs' prompts instead. The platform
    // scope derives from the same declared-platform read as the shell
    // legs, so a core-only project materializes nothing for shells it
    // will not build.
    let shell_platforms: Vec<String> =
        declared_shell_legs(&tree_root).iter().map(|leg| leg.name.to_string()).collect();
    let prelude = prepare::materialize_step(&slice_dir, &tree_root, &shell_platforms)
        .map_err(error_from_vectis)?;
    let prelude_block = render_prelude(&prelude);

    // Step 0.5 + Phase 1 — component inference and composition
    // regeneration. Catalog inference is CLI-assisted judgment the brief
    // owns, so the leg runs `specify catalog infer` itself in the lent
    // workspace; regeneration reads the updated catalog back.
    let system = assemble(&["briefs/build.md", "briefs/shape.md", "briefs/build/composition.md"]);
    let user = format!(
        "Run component inference (Step 0.5) and composition regeneration (Phase 1) of \
         the vectis build for slice `{slice}` (adapter `{}`).\n\n\
         The project workspace is lent to you. Run `specify catalog infer` yourself per \
         the build brief's Step 0.5 — this adapter cannot spawn it — then regenerate \
         `{slice_dir_rel}/composition.yaml` from the slice artifacts per the \
         composition sub-brief. For a slice with no UI surface, write no composition \
         and answer with `applicable: false`.\n\n\
         {prelude_block}\n\n{SHELF_POINTER}\n\n{inputs_block}",
        ctx.adapter_id,
    );
    let composition = phase::phase(model, ctx, system, user, "composition").await?;

    // The per-shell write briefs require the composition gate passed
    // before any platform phase: an exhausted repair budget parks the
    // slice with a deterministic failure report.
    let residual = composition_gate(model, ctx, slice, &slice_dir_rel, &slice_composition).await?;
    if !residual.is_empty() {
        return Ok(Report {
            status: Status::Failure,
            findings: residual.into_iter().map(Finding::blocking).collect(),
            outputs: Vec::new(),
            ui_surface: None,
        });
    }

    // Phases 2–3 — Crux core writer plus test writer: the core
    // verify-repair loop crosses both sub-briefs (a cargo failure
    // re-enters the writer), so one agent leg holds them together.
    let system =
        assemble(&["briefs/build.md", "briefs/build/core/write.md", "briefs/build/test.md"]);
    let user = format!(
        "Run the Crux core phases (2-3) of the vectis build for slice `{slice}`: \
         generate or update the shared core per the core write sub-brief, write the \
         Crux tests, then run the test sub-brief's core verify-repair loop yourself — \
         the cargo check / clippy / test commands run in the lent workspace; this \
         adapter cannot spawn them. Detect create vs update mode from the tree. \
         {SHELF_POINTER}\n\n{inputs_block}",
    );
    let core = phase::phase(model, ctx, system, user, "core").await?;

    // Phases 4–5 — per-shell writes, conditional on the declared
    // platform set (`project.yaml.platforms`); a core-only platform set
    // skips the shell legs wholesale, per the brief's platform scope.
    let mut shell_outcomes: Vec<(&'static str, phase::PhaseAnswer)> = Vec::new();
    for shell in declared_shell_legs(&tree_root) {
        let system = assemble(&["briefs/build.md", shell.write_brief]);
        let user = format!(
            "Run the {name} shell phase of the vectis build for slice `{slice}`: \
             scaffold the shell first when its tree is absent (`specify extension run \
             vectis -- scaffold {name} <APP_NAME>`), generate or update the shell per \
             the write sub-brief, run `specify extension run vectis -- sync \
             {name}-scaffold` once after writing, then run the sub-brief's \
             orchestrator-owned verify loop yourself in the lent workspace — this \
             adapter cannot spawn host commands. When the slice introduces no work for \
             this shell, write nothing and answer with `applicable: false`; when a host \
             prerequisite is missing, stop per the brief's deferred contract and report \
             it in your summary. {SHELF_POINTER}",
            name = shell.name,
        );
        let answer = phase::phase(model, ctx, system, user, shell.name).await?;
        shell_outcomes.push((shell.name, answer));
    }

    // Phases 6–7 — the review teams (parallel per the brief) and
    // § Consolidate review findings, one leg.
    let mut review_briefs = vec!["briefs/build.md", "briefs/build/core/review.md"];
    review_briefs.extend(declared_shell_legs(&tree_root).iter().map(|shell| shell.review_brief));
    let system = assemble(&review_briefs);
    let user = format!(
        "Run the review phases (6-7) of the vectis build for slice `{slice}`: spawn \
         the core reviewer team and, for each in-scope shell, its platform reviewer \
         team per the review sub-briefs (reviewers run in parallel), then run the build \
         brief's `## § Consolidate review findings` and drive any remediation in the \
         lent workspace. {SHELF_POINTER}",
    );
    let review = phase::phase(model, ctx, system, user, "review").await?;

    // Final leg — the shell verify gate (Phase 8, agent-run) and the
    // report answer (Phase 9), gated by the derived answer schema.
    let mut outcomes = vec![("composition", &composition), ("core", &core)];
    outcomes.extend(shell_outcomes.iter().map(|(name, answer)| (*name, answer)));
    outcomes.push(("review", &review));
    let user = format!(
        "Write the build report for slice `{slice}` per the build brief's `## Build \
         report`. First run the shell verify gate yourself in the lent workspace \
         (`specify extension run vectis -- verify --mode verify <PROJECT_DIR>`) — this \
         adapter cannot spawn it; a missing or empty tree for a supported declared \
         platform forces `status: failure`. Then mark the completed `tasks.md` \
         checkboxes in the slice directory per the brief before answering. A `success` \
         report carries only non-blocking findings; an exhausted verify-repair budget, \
         a failed composition gate, or unresolved blocking review findings mean \
         `status: failure`. Declare `outputs[]` per supported platform with paths \
         relative to the project root, and set `ui-surface.screens` from the slice's \
         own screen count.\n\n\
         Phase outcomes:\n{}",
        outcomes
            .iter()
            .map(|(name, answer)| phase::render_outcome(name, answer))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    let report = phase::report(model, ctx, build_brief.to_string(), user).await?;

    // Deterministic postlude: the in-core validator cross-checks plus
    // the report-coherence walk, one bounded repair leg, then
    // enforcement.
    gate_report(model, ctx, build_brief, report, &tree_root, &slice_composition, "build").await
}

/// Merge a built slice's delta into the baseline per the merge brief.
///
/// One judgment leg folds the delta and runs the brief's host cap-matrix
/// re-verification (agent-run in the lent workspace), then the
/// deterministic postlude re-runs the composition validator against the
/// merged baseline (`.specify/specs/composition.yaml`) plus the
/// report-coherence walk, with one bounded repair leg.
///
/// # Errors
///
/// As [`specify_guest_kit::judgment`].
pub async fn merge<P: Model>(
    model: &P, ctx: &Context<'_>, slice: &str, delta: &Changeset, tree: &WorkingTree,
) -> Result<Report, Error> {
    let tree_root = ctx.tree_root(tree);
    let baseline_composition = tree_root.join(".specify/specs/composition.yaml");
    let merge_brief = registry::body("briefs/merge.md");
    let delta_block = phase::render_delta(delta);

    let user = format!(
        "Merge slice `{slice}`'s built delta (adapter `{}`). The project workspace is \
         lent to you; the delta below applies against base `{}` (a 3-way merge: the \
         baseline is ours, the delta is theirs). Fold the changes in place — including \
         the slice's `composition.yaml` into the baseline and any operator-curated \
         `tokens.yaml` / `assets.yaml` updates into `design-system/` — then run the \
         merge brief's `## Post-merge — host cap-matrix re-verification` yourself: the \
         cargo / make / gradlew commands (and the adapter's host-prereq and \
         finalize-verify scripts) run in the lent workspace; this adapter cannot spawn \
         them. Any gate failure means `status: failure`. Answer with the report body. \
         {SHELF_POINTER}\n\n{delta_block}",
        ctx.adapter_id, delta.base,
    );
    let report = phase::report(model, ctx, merge_brief.to_string(), user).await?;

    gate_report(model, ctx, merge_brief, report, &tree_root, &baseline_composition, "merge").await
}

/// One per-shell write leg the declared platform set enables.
struct ShellLeg {
    /// Platform token (`ios` / `android`), used in prompts and answer
    /// schema names.
    name: &'static str,
    /// Registry path of the platform's write sub-brief.
    write_brief: &'static str,
    /// Registry path of the platform's review sub-brief.
    review_brief: &'static str,
}

/// The shell platforms with build sub-briefs, in the brief's dependency
/// order (core first is implicit; iOS and Android generation legs are
/// independent but run serially here — their verify halves share the
/// same cargo workspace lock anyway, per the brief).
const SHELL_LEGS: [ShellLeg; 2] = [
    ShellLeg {
        name: "ios",
        write_brief: "briefs/build/ios/write.md",
        review_brief: "briefs/build/ios/review.md",
    },
    ShellLeg {
        name: "android",
        write_brief: "briefs/build/android/write.md",
        review_brief: "briefs/build/android/review.md",
    },
];

/// The shell write legs the project's declared platform set enables.
///
/// Reads `project.yaml.platforms`: a declared set without `ios` /
/// `android` is a backend-only build and skips the shell legs wholesale;
/// an absent or unreadable declaration falls back to the adapter's
/// default shell set (both), with each leg still free to self-skip via
/// `applicable: false`. `web` / `desktop` have no sub-brief and are
/// silently skipped.
fn declared_shell_legs(project_root: &Path) -> Vec<&'static ShellLeg> {
    let declared = declared_platforms(project_root);
    SHELL_LEGS
        .iter()
        .filter(|leg| declared.as_ref().is_none_or(|set| set.iter().any(|p| p == leg.name)))
        .collect()
}

/// The `platforms:` list from `.specify/project.yaml`, or `None` when
/// the file is absent or does not carry a string array.
fn declared_platforms(project_root: &Path) -> Option<Vec<String>> {
    let source = std::fs::read_to_string(project_root.join(".specify/project.yaml")).ok()?;
    let doc: Value = serde_saphyr::from_str(&source).ok()?;
    let platforms = doc.get("platforms")?.as_array()?;
    Some(platforms.iter().filter_map(Value::as_str).map(str::to_string).collect())
}

/// The in-core composition validator gate, with its bounded repair loop
/// — the per-shell write briefs require this gate passed before any
/// platform phase, so it runs right after the composition leg rather
/// than with the postlude. Returns the residual findings after the
/// repair budget: non-empty means the gate did not clear and the build
/// must park the slice rather than run the platform phases.
async fn composition_gate<P: Model>(
    model: &P, ctx: &Context<'_>, slice: &str, slice_dir_rel: &str, composition: &Path,
) -> Result<Vec<String>, Error> {
    let mut findings = validation_findings(composition);
    for _ in 0..MAX_VALIDATE_REPAIR_ITERATIONS {
        if findings.is_empty() {
            break;
        }
        let system = assemble(&["briefs/build.md", "briefs/build/composition.md"]);
        let user = format!(
            "The deterministic composition validator found blocking issues in slice \
             `{slice}`'s regenerated `{slice_dir_rel}/composition.yaml`. Repair the \
             composition (or the operator-curated manifests it references) in place per \
             the composition sub-brief's validator gate.\n\n{}\n\n\
             Answer `applicable: true` with a summary of the repairs. {SHELF_POINTER}",
            findings.join("\n"),
        );
        phase::phase(model, ctx, system, user, "composition-repair").await?;
        findings = validation_findings(composition);
    }
    Ok(findings)
}

/// Assemble a system prompt from embedded brief bodies.
fn assemble(briefs: &[&str]) -> String {
    let bodies: Vec<&str> = briefs.iter().map(|brief| registry::body(brief)).collect();
    phase::assemble_system(&bodies)
}

/// The deterministic gate after the report answer lands, with one
/// bounded repair leg: the in-core composition cross-checks (schema,
/// structural identity, sibling tokens / assets auto-invoke, reference
/// resolution) re-run against `composition`, and a `success` report's
/// declared outputs must exist under the tree root. Residual findings
/// then force `failure` regardless of the answer.
async fn gate_report<P: Model>(
    model: &P, ctx: &Context<'_>, brief: &str, mut report: Report, tree_root: &Path,
    composition: &Path, operation: &str,
) -> Result<Report, Error> {
    let mut residual = validation_findings(composition);
    let mut missing = phase::missing_outputs(&report, tree_root);
    if !residual.is_empty() || !missing.is_empty() {
        let user = format!(
            "The deterministic report gate rejected the {operation} report:\n\n{}\n\n\
             Repair the working tree (or correct the report), then answer with the \
             corrected report body.",
            residual.iter().chain(missing.iter()).cloned().collect::<Vec<_>>().join("\n"),
        );
        report = phase::report(model, ctx, brief.to_string(), user).await?;
        residual = validation_findings(composition);
        missing = phase::missing_outputs(&report, tree_root);
    }
    let findings = residual.into_iter().chain(missing).map(Finding::blocking).collect();
    Ok(phase::enforce(report, findings))
}

/// The in-core validator findings for one composition artifact, one
/// findings-style line each. An absent artifact is clean by design (a
/// core-only slice or a pre-first-merge baseline carries none); an
/// unreadable one surfaces as a finding rather than an error so the
/// bounded repair leg gets a chance to fix the tree.
fn validation_findings(composition: &Path) -> Vec<String> {
    if !composition.exists() {
        return Vec::new();
    }
    match validate::run(validate::ValidateMode::Composition, Some(composition)) {
        Ok(envelope) => {
            let mut findings = Vec::new();
            collect_envelope_errors(&envelope, "composition", &mut findings);
            findings
        }
        Err(err) => vec![format!("- [composition] {}: {err}", composition.display())],
    }
}

/// Fold a validation envelope's `errors` (and any auto-invoked
/// sub-reports under `results`) into findings-style lines.
fn collect_envelope_errors(envelope: &Value, mode: &str, findings: &mut Vec<String>) {
    let mode = envelope.get("mode").and_then(Value::as_str).unwrap_or(mode);
    if let Some(errors) = envelope.get("errors").and_then(Value::as_array) {
        for error in errors {
            let path = error.get("path").and_then(Value::as_str).unwrap_or("");
            let message = error.get("message").and_then(Value::as_str).unwrap_or("");
            findings.push(format!("- [{mode}] `{path}`: {message}"));
        }
    }
    if let Some(results) = envelope.get("results").and_then(Value::as_array) {
        for entry in results {
            if let Some(report) = entry.get("report") {
                collect_envelope_errors(report, mode, findings);
            }
        }
    }
}

/// Map an absorbed-library failure onto the seam error vocabulary: I/O
/// failures map through, a broken project or design-system input is an
/// invalid request (the workspace is part of the call's contract), and
/// internal invariants stay internal.
fn error_from_vectis(err: VectisError) -> Error {
    match err {
        VectisError::Io(io) => Error::Io(io.to_string()),
        VectisError::InvalidProject { message } => Error::InvalidRequest(message),
        VectisError::Internal { message } => Error::Internal(message),
    }
}

/// Render the deterministic prelude's materialize summary for the
/// composition leg's prompt.
fn render_prelude(summary: &Value) -> String {
    format!(
        "### prepare prelude (already run in-guest)\n\n\
         The adapter resolved the slice's materialize scope and ran the deterministic \
         `materialize assets` step before this leg; do not re-run it. Summary:\n\n{}",
        serde_json::to_string(summary).unwrap_or_else(|_| "{}".to_string()),
    )
}
