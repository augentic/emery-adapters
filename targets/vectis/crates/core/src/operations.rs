//! The judgment operation template: `guidance`, `build`, and `merge`.
//!
//! Each judgment leg is bracketed by deterministic guest code. The core
//! assembles a prompt from the embedded briefs plus the typed inputs,
//! issues a single-shot `create` through the shared
//! [`specify_guest_kit::judgment`] helper with a schema-gated `format`,
//! and — unlike omnia — brackets the legs with the absorbed vectis
//! libraries: the [`crate::prepare`] materialize step runs as the
//! deterministic *prelude* (replacing the legacy `adapter.yaml`
//! `prepare.argv` hook), and the [`crate::validate`] composition /
//! tokens / assets cross-checks run as the deterministic *postlude*,
//! feeding a bounded repair loop the way the contracts adapter's
//! validators do. All state between calls lives in the workspace tree —
//! the session-less shape.
//!
//! `build` decomposes along the build brief's own phase order: one
//! *composition* leg (Step 0.5 component inference plus Phase 1
//! composition regeneration) gated in-core by the composition validator
//! (the per-shell write briefs require that gate passed first), one
//! *core* leg (Phases 2–3 — Crux core writer, test writer, and the
//! cargo verify-repair loop only the spawned agent can run in the lent
//! workspace), one *shell* leg per declared shell platform (Phases 4–5
//! — iOS / Android writers with their orchestrator-run verify loops),
//! one *review* leg (Phases 6–7 — the per-platform review teams and
//! § Consolidate review findings), then one report leg (Phases 8–9 —
//! the agent-run shell verify gate and the report body). Host-command
//! verification (cargo, xcodebuild, Gradle, `specify extension run
//! vectis -- …`, and the host-prereq / finalize-verify scripts) is
//! process-spawning and stays agent-side in the prompts; the
//! deterministic tail checks what the in-core validators and pure Rust
//! over the mounted tree can: the composition cross-checks hold and
//! declared `outputs` paths exist.

use std::path::Path;

use serde::Deserialize;
use serde_json::Value;
use specify_guest_kit::answers::{REPORT_ANSWER_SCHEMA, ReportAnswer};
use specify_guest_kit::seam::{
    Changeset, Context, Error, Finding, Input, Report, Severity, Status, WorkingTree,
};
use specify_guest_kit::{Model, judgment};

use crate::{VectisError, prepare, registry, validate};

/// Maximum composition validator repair iterations after the
/// composition leg, mirroring the contracts build's Phase 4 budget.
const MAX_VALIDATE_REPAIR_ITERATIONS: usize = 2;

/// Adapter-internal answer schema for one phase leg. Internal legs are
/// not part of the `augentic:specify` contract, so this schema lives
/// here rather than deriving from a canonical schema.
const PHASE_ANSWER_SCHEMA: &str = r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "additionalProperties": false,
  "properties": {
    "applicable": {
      "description": "Whether the phase had work to do. `false` means the phase wrote nothing (e.g. a shell write for a no-op platform, or composition regeneration for a slice with no UI surface).",
      "type": "boolean"
    },
    "summary": {
      "description": "One-paragraph account of what was generated, reviewed, repaired, or why the phase was skipped.",
      "minLength": 1,
      "type": "string"
    },
    "written": {
      "default": [],
      "description": "Workspace-relative paths of files this phase created or modified.",
      "items": { "type": "string" },
      "type": "array"
    }
  },
  "required": ["applicable", "summary"]
}"#;

/// One phase leg's schema-gated answer.
#[derive(Debug, Deserialize)]
struct PhaseAnswer {
    applicable: bool,
    summary: String,
    #[serde(default)]
    written: Vec<String>,
}

/// The pointer at the adapter's own MCP reference shelf every judgment
/// leg's user prompt carries: the shelf serves the full `briefs/` /
/// `references/` / `rules/` trees, so prompts stay lean and the agent
/// fetches specialist material lazily instead of getting it inlined.
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
/// `composition.yaml` per the build brief's phase order.
///
/// Session-less decomposition along the brief's own structure, with the
/// absorbed libraries bracketing the legs:
///
/// 1. **Prelude (deterministic)** — [`prepare::materialize_step`]:
///    RFC §2.1 scope resolution plus the conditional scoped
///    `materialize assets` run, replacing the legacy `prepare.argv`
///    hook. Its summary feeds the composition leg's prompt.
/// 2. **Composition leg** (Step 0.5 + Phase 1) — component inference
///    and composition regeneration — then the in-core composition
///    validator gate with a bounded repair loop (the shell write briefs
///    require the gate passed before any platform phase).
/// 3. **Core leg** (Phases 2–3), then one **shell leg** per declared
///    shell platform (Phases 4–5; a slice with no work for a shell
///    answers `applicable: false`), then the **review leg**
///    (Phases 6–7).
/// 4. One report leg (Phases 8–9) gated by the derived answer schema,
///    with the agent-run shell verify gate instructed in its prompt.
/// 5. **Postlude (deterministic)** — the composition / tokens / assets
///    cross-checks re-run in core plus the report-coherence walk
///    (declared outputs exist under the tree root), with one bounded
///    repair leg; residual findings force `failure` regardless of the
///    answer.
///
/// # Errors
///
/// Returns [`Error::InvalidRequest`] when the model rejects a request
/// as malformed or the workspace's design-system inputs are unreadable
/// where the prelude needs them, [`Error::Io`] for prelude filesystem
/// failures, and [`Error::Internal`] for other model failures or an
/// answer that does not deserialize.
pub async fn build<P: Model>(
    model: &P, ctx: &Context<'_>, slice: &str, inputs: &[Input], tree: &WorkingTree,
) -> Result<Report, Error> {
    let tree_root = ctx.tree_root(tree);
    let slice_dir_rel = format!(".specify/slices/{slice}");
    let slice_dir = tree_root.join(&slice_dir_rel);
    let slice_composition = slice_dir.join("composition.yaml");
    let inputs_block = render_inputs(inputs);
    let build_brief = registry::body("briefs/build.md");

    // Deterministic prelude — prepare scope resolution + conditional
    // materialize over the effective assets.yaml, in-guest. This is the
    // absorbed `prepare build` materialize step; the host-bootstrap legs
    // the legacy hook also ran (app-icon verify gate, Android Gradle
    // setup, iOS scaffold sync) are process-adjacent and ride agent-side
    // in the shell legs' prompts instead. The platform scope derives
    // from the same declared-platform read as the shell legs, so a
    // core-only project materializes nothing for shells it will not
    // build.
    let shell_platforms: Vec<String> =
        declared_shell_legs(&tree_root).iter().map(|leg| leg.name.to_string()).collect();
    let prelude = prepare::materialize_step(&slice_dir, &tree_root, &shell_platforms)
        .map_err(error_from_vectis)?;
    let prelude_block = render_prelude(&prelude);

    // Step 0.5 + Phase 1 — component inference and composition
    // regeneration. Catalog inference is CLI-assisted judgment the brief
    // owns, so the leg runs `specify catalog infer` itself in the lent
    // workspace; regeneration reads the updated catalog back.
    let system =
        assemble_system(&["briefs/build.md", "briefs/shape.md", "briefs/build/composition.md"]);
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
    let composition = phase_call(model, ctx, system, user, "composition").await?;

    // The per-shell write briefs require the composition gate passed
    // before any platform phase: an exhausted repair budget parks the
    // slice with a deterministic failure report instead of burning the
    // downstream core / shell / review / report legs against a
    // knowingly-broken composition.
    let residual = composition_gate(model, ctx, slice, &slice_dir_rel, &slice_composition).await?;
    if !residual.is_empty() {
        return Ok(gate_failure_report(residual));
    }

    // Phases 2–3 — Crux core writer plus test writer: the core
    // verify-repair loop crosses both sub-briefs (a cargo failure
    // re-enters the writer), so one agent leg holds them together.
    let system =
        assemble_system(&["briefs/build.md", "briefs/build/core/write.md", "briefs/build/test.md"]);
    let user = format!(
        "Run the Crux core phases (2-3) of the vectis build for slice `{slice}`: \
         generate or update the shared core per the core write sub-brief, write the \
         Crux tests, then run the test sub-brief's core verify-repair loop yourself — \
         the cargo check / clippy / test commands run in the lent workspace; this \
         adapter cannot spawn them. Detect create vs update mode from the tree. \
         {SHELF_POINTER}\n\n{inputs_block}",
    );
    let core = phase_call(model, ctx, system, user, "core").await?;

    // Phases 4–5 — per-shell writes, conditional on the declared
    // platform set (`project.yaml.platforms`); a core-only platform set
    // skips the shell legs wholesale, per the brief's platform scope.
    let mut shell_outcomes: Vec<(&'static str, PhaseAnswer)> = Vec::new();
    for shell in declared_shell_legs(&tree_root) {
        let system = assemble_system(&["briefs/build.md", shell.write_brief]);
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
        let answer = phase_call(model, ctx, system, user, shell.name).await?;
        shell_outcomes.push((shell.name, answer));
    }

    // Phases 6–7 — the review teams (parallel per the brief) and
    // § Consolidate review findings, one leg.
    let mut review_briefs = vec!["briefs/build.md", "briefs/build/core/review.md"];
    review_briefs.extend(declared_shell_legs(&tree_root).iter().map(|shell| shell.review_brief));
    let system = assemble_system(&review_briefs);
    let user = format!(
        "Run the review phases (6-7) of the vectis build for slice `{slice}`: spawn \
         the core reviewer team and, for each in-scope shell, its platform reviewer \
         team per the review sub-briefs (reviewers run in parallel), then run the build \
         brief's `## § Consolidate review findings` and drive any remediation in the \
         lent workspace. {SHELF_POINTER}",
    );
    let review = phase_call(model, ctx, system, user, "review").await?;

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
        outcomes.iter().map(render_phase_outcome).collect::<Vec<_>>().join("\n"),
    );
    let report = report_call(model, ctx, build_brief.to_string(), user).await?;

    // Deterministic postlude: the in-core validator cross-checks plus
    // the report-coherence walk, one bounded repair leg, then
    // enforcement.
    gate_report(model, ctx, build_brief, report, &tree_root, &slice_composition, "build").await
}

/// Merge a built slice's delta into the baseline per the merge brief.
///
/// One judgment leg folds the delta and runs the brief's host
/// cap-matrix re-verification (agent-run in the lent workspace — the
/// `host_prereq` / `finalize_verify` scripts and the cargo / make /
/// gradlew matrix are process-spawning and stay agent-side) and answers
/// with the report; the deterministic postlude then re-runs the
/// composition validator against the merged baseline
/// (`.specify/specs/composition.yaml`, with its sibling tokens / assets
/// auto-invoke) plus the report-coherence walk, with one bounded repair
/// leg — mirroring the contracts merge shape.
///
/// # Errors
///
/// Returns [`Error::InvalidRequest`] when the model rejects a request
/// as malformed, and [`Error::Internal`] for other model failures or an
/// answer that does not deserialize.
pub async fn merge<P: Model>(
    model: &P, ctx: &Context<'_>, slice: &str, delta: &Changeset, tree: &WorkingTree,
) -> Result<Report, Error> {
    let tree_root = ctx.tree_root(tree);
    let baseline_composition = tree_root.join(".specify/specs/composition.yaml");
    let merge_brief = registry::body("briefs/merge.md");
    let delta_block = render_delta(delta);

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
    let report = report_call(model, ctx, merge_brief.to_string(), user).await?;

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
/// `android` is a backend-only build and skips the shell legs
/// wholesale (the brief's platform scope); an absent or unreadable
/// declaration falls back to the adapter's default shell set (both),
/// with each leg still free to self-skip via `applicable: false`.
/// `web` / `desktop` have no sub-brief and are silently skipped.
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
        let system = assemble_system(&["briefs/build.md", "briefs/build/composition.md"]);
        let user = format!(
            "The deterministic composition validator found blocking issues in slice \
             `{slice}`'s regenerated `{slice_dir_rel}/composition.yaml`. Repair the \
             composition (or the operator-curated manifests it references) in place per \
             the composition sub-brief's validator gate.\n\n{}\n\n\
             Answer `applicable: true` with a summary of the repairs. {SHELF_POINTER}",
            findings.join("\n"),
        );
        phase_call(model, ctx, system, user, "composition-repair").await?;
        findings = validation_findings(composition);
    }
    Ok(findings)
}

/// The deterministic failure report an exhausted composition gate
/// parks the slice with — no downstream legs run, per the shell write
/// briefs' gate-passed-first requirement.
fn gate_failure_report(residual: Vec<String>) -> Report {
    Report {
        status: Status::Failure,
        findings: residual
            .into_iter()
            .map(|detail| Finding {
                rule_id: None,
                severity: Severity::Important,
                detail,
            })
            .collect(),
        outputs: Vec::new(),
        ui_surface: None,
    }
}

/// Assemble a system prompt from embedded brief bodies, separated the
/// way the contracts template separates orchestrator and sub-brief.
fn assemble_system(briefs: &[&str]) -> String {
    briefs.iter().map(|brief| registry::body(brief)).collect::<Vec<_>>().join("\n\n---\n\n")
}

/// Issue one internal phase leg through the shared judgment helper.
async fn phase_call<P: Model>(
    model: &P, ctx: &Context<'_>, system: String, user: String, name: &str,
) -> Result<PhaseAnswer, Error> {
    judgment(model, ctx, system, user, name, PHASE_ANSWER_SCHEMA).await
}

/// Issue one report leg gated by the derived answer schema and project
/// the answer onto the seam-facing report.
async fn report_call<P: Model>(
    model: &P, ctx: &Context<'_>, system: String, user: String,
) -> Result<Report, Error> {
    judgment::<P, ReportAnswer>(model, ctx, system, user, "report", REPORT_ANSWER_SCHEMA)
        .await
        .map(ReportAnswer::into_report)
}

/// The deterministic gate after the report answer lands, with one
/// bounded repair leg — the vectis counterpart of the contracts
/// validator gate: the in-core composition cross-checks (schema,
/// structural identity, sibling tokens / assets auto-invoke, reference
/// resolution) re-run against `composition`, and a `success` report's
/// declared outputs must exist under the tree root. Residual findings
/// then force `failure` regardless of the answer.
async fn gate_report<P: Model>(
    model: &P, ctx: &Context<'_>, brief: &str, mut report: Report, tree_root: &Path,
    composition: &Path, operation: &str,
) -> Result<Report, Error> {
    let mut residual = validation_findings(composition);
    let mut missing = missing_outputs(&report, tree_root);
    if !residual.is_empty() || !missing.is_empty() {
        let user = format!(
            "The deterministic report gate rejected the {operation} report:\n\n{}\n\n\
             Repair the working tree (or correct the report), then answer with the \
             corrected report body.",
            residual.iter().chain(missing.iter()).cloned().collect::<Vec<_>>().join("\n"),
        );
        report = report_call(model, ctx, brief.to_string(), user).await?;
        residual = validation_findings(composition);
        missing = missing_outputs(&report, tree_root);
    }
    Ok(enforce_gate(report, &residual, &missing))
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

/// The declared outputs a `success` report claims that the mounted tree
/// does not contain, one findings-style line each. A `failure` report
/// is already parked for human review per the briefs' stop contract, so
/// its output claims are not re-litigated.
fn missing_outputs(report: &Report, tree_root: &Path) -> Vec<String> {
    if report.status == Status::Failure {
        return Vec::new();
    }
    report
        .outputs
        .iter()
        .filter(|output| !tree_root.join(&output.path).exists())
        .map(|output| {
            format!("- declared output `{}` does not exist in the working tree", output.path)
        })
        .collect()
}

/// Deterministic guard after the final answer lands: residual validator
/// findings or output discrepancies force `failure` and are appended to
/// the report; a `success` answer carrying blocking findings is
/// downgraded the same way.
fn enforce_gate(mut report: Report, residual: &[String], missing: &[String]) -> Report {
    if !residual.is_empty() || !missing.is_empty() {
        report.status = Status::Failure;
        report.findings.extend(residual.iter().chain(missing.iter()).map(|detail| Finding {
            rule_id: None,
            severity: Severity::Important,
            detail: detail.clone(),
        }));
    }
    if report.status == Status::Success
        && report.findings.iter().any(|finding| finding.severity.blocking())
    {
        report.status = Status::Failure;
    }
    report
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

/// Render one phase leg's outcome for the report prompt.
fn render_phase_outcome((name, answer): &(&str, &PhaseAnswer)) -> String {
    format!(
        "- {name}: applicable={}, wrote {:?} — {}",
        answer.applicable, answer.written, answer.summary
    )
}

/// Render the typed inputs as labeled prompt sections.
fn render_inputs(inputs: &[Input]) -> String {
    if inputs.is_empty() {
        return "(no slice artifacts were provided)".to_string();
    }
    inputs
        .iter()
        .map(|input| format!("### input: {}\n\n{}", input.label(), input.body()))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Render a changeset's edits for the merge prompt.
fn render_delta(delta: &Changeset) -> String {
    if delta.edits.is_empty() {
        return "### delta\n\n(empty changeset — the slice wrote no files)".to_string();
    }
    let edits = delta
        .edits
        .iter()
        .map(|edit| {
            edit.content.as_ref().map_or_else(
                || format!("- {} (deleted)", edit.path),
                |content| format!("- {} (content: {content})", edit.path),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("### delta (base {})\n\n{edits}", delta.base)
}
