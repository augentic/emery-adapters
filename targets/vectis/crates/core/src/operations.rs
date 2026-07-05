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
//! leg (Phases 8–9). The absorbed libraries carry every leg that used to
//! be a `vectis` CLI call: the infer report and missing-tree scaffolds
//! run as deterministic preludes, scaffold sync as a per-shell
//! postlude, and the shell verify gate inside the deterministic report
//! gate. Host-command verification (cargo, xcodebuild, Gradle) is
//! process-spawning and stays agent-side in the prompts.

use std::path::Path;

use serde_json::Value;
use specify_guest_kit::seam::{
    Changeset, Context, Error, Finding, Input, Report, Status, WorkingTree,
};
use specify_guest_kit::{Model, phase};

use crate::{
    VectisError, android, android_scaffold, infer, ios_scaffold, prepare, registry, scaffold,
    shell, validate, verify,
};

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
/// embedded guidance brief, returned deterministically (no judgment leg).
#[must_use]
pub fn guidance() -> &'static str {
    registry::body("briefs/guidance.md")
}

/// Build a slice's Crux core, shell code, and regenerated
/// `composition.yaml` per the build brief's phase order:
///
/// 1. **Prelude (deterministic)** — [`prepare::materialize_step`]:
///    RFC §2.1 scope resolution plus the conditional scoped
///    `materialize assets` run (its summary feeds the composition leg),
///    then the §L bootstrap app-icon gate — error findings park the
///    build before any judgment leg.
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
#[expect(
    clippy::too_many_lines,
    reason = "One linear leg-by-leg walk of the brief's phase order; splitting hides the order."
)]
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

    // Deterministic bootstrap gate (§L): the launcher app-icon must be
    // satisfiable for every declared UI platform before any write leg;
    // error findings park the build like the legacy prepare hook did.
    let bootstrap = bootstrap_findings(&tree_root);
    if !bootstrap.is_empty() {
        return Ok(Report {
            status: Status::Failure,
            findings: bootstrap.into_iter().map(Finding::blocking).collect(),
            outputs: Vec::new(),
            ui_surface: None,
        });
    }

    // Step 0.5 + Phase 1 — component inference and composition
    // regeneration. Component *identity* is deterministic and runs
    // in-guest here (the name-free cluster report); *naming* is the
    // leg's judgment, recorded as a bindings file the workflow's
    // deterministic bind bookkeeping projects into the catalog.
    let infer_block = render_infer_report(&tree_root);
    let system =
        assemble(&["briefs/build.md", "briefs/guidance.md", "briefs/build/composition.md"]);
    let user = format!(
        "Run component inference (Step 0.5) and composition regeneration (Phase 1) of \
         the vectis build for slice `{slice}` (adapter `{}`).\n\n\
         The project workspace is lent to you. The adapter already ran the \
         deterministic component-identity clustering in-guest — the name-free cluster \
         report is below; do not attempt to re-run it. Decide what each unbound \
         cluster is and what to call it per the build brief's Step 0.5, write your \
         `{{ fingerprint -> slug }}` decisions to \
         `{slice_dir_rel}/build/component-bindings.yaml` (echo populated `bound-slug` \
         names verbatim — operator parts carry naming authority), then regenerate \
         `{slice_dir_rel}/composition.yaml` from the slice artifacts per the \
         composition sub-brief, treating your fresh bindings plus the existing catalog \
         as the effective component set. For a slice with no UI surface, write no \
         composition and answer with `applicable: false`.\n\n\
         {infer_block}\n\n\
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

    // Deterministic scaffold prelude — stand up the missing core /
    // shell trees from the embedded templates before the write legs
    // (create mode). The summary rides into the leg prompts; a scaffold
    // the adapter could not run deterministically falls to the leg's
    // writer per the write sub-brief.
    let scaffold_block = scaffold_missing_trees(&tree_root);

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
         adapter cannot spawn them. Detect create vs update mode from the tree.\n\n\
         {scaffold_block}\n\n{SHELF_POINTER}\n\n{inputs_block}",
    );
    let core = phase::phase(model, ctx, system, user, "core").await?;

    // Phases 4–5 — per-shell writes, conditional on the declared
    // platform set (`project.yaml.platforms`); a core-only platform set
    // skips the shell legs wholesale, per the brief's platform scope.
    // The adapter re-renders the agent-immutable scaffold files
    // deterministically before each write leg (repairing prior drift
    // ahead of the leg's verify loop) and again after it (the legacy
    // `sync` step).
    let mut shell_outcomes: Vec<(&'static str, phase::PhaseAnswer)> = Vec::new();
    let mut sync_notes: Vec<String> = Vec::new();
    for shell in declared_shell_legs(&tree_root) {
        if let Some(note) = sync_shell_scaffold(&tree_root, shell.name) {
            sync_notes.push(note);
        }
        let system = assemble(&["briefs/build.md", shell.write_brief]);
        let user = format!(
            "Run the {name} shell phase of the vectis build for slice `{slice}`: \
             generate or update the shell per the write sub-brief (the adapter already \
             scaffolded any absent declared tree deterministically — see below; do not \
             hand-write scaffold boilerplate), then run the sub-brief's \
             orchestrator-owned verify loop yourself in the lent workspace — this \
             adapter cannot spawn host commands. The agent-immutable scaffold files \
             (Makefiles, `project.yml`, assembly Gradle files, `.vectis/` scripts) are \
             re-rendered deterministically by the adapter before and after this leg; \
             never edit them. When the slice introduces no work for this shell, write \
             nothing and answer with `applicable: false`; when a host prerequisite is \
             missing, stop per the brief's deferred contract and report it in your \
             summary.\n\n{scaffold_block}\n\n{SHELF_POINTER}",
            name = shell.name,
        );
        let answer = phase::phase(model, ctx, system, user, shell.name).await?;
        shell_outcomes.push((shell.name, answer));
        if let Some(note) = sync_shell_scaffold(&tree_root, shell.name) {
            sync_notes.push(note);
        }
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

    // Final leg — the deterministic shell verify gate (Phase 8, run
    // in-guest) feeds the report answer (Phase 9), gated by the derived
    // answer schema.
    let verify_block = render_verify_gate(&tree_root);
    let sync_block = if sync_notes.is_empty() {
        String::new()
    } else {
        format!("\n\nScaffold sync notes:\n{}", sync_notes.join("\n"))
    };
    let mut outcomes = vec![("composition", &composition), ("core", &core)];
    outcomes.extend(shell_outcomes.iter().map(|(name, answer)| (*name, answer)));
    outcomes.push(("review", &review));
    let user = format!(
        "Write the build report for slice `{slice}` per the build brief's `## Build \
         report`. The adapter already ran the deterministic shell verify gate in-guest \
         — its findings are below and re-run after your answer; a missing or empty \
         tree for a supported declared platform forces `status: failure`, so repair \
         the tree first when the gate reports errors. Then mark the completed \
         `tasks.md` checkboxes in the slice directory per the brief before answering. \
         A `success` report carries only non-blocking findings; an exhausted \
         verify-repair budget, a failed composition gate, or unresolved blocking \
         review findings mean `status: failure`. Declare `outputs[]` per supported \
         platform with paths relative to the project root, and set \
         `ui-surface.screens` from the slice's own screen count.\n\n\
         {verify_block}{sync_block}\n\n\
         Phase outcomes:\n{}",
        outcomes
            .iter()
            .map(|(name, answer)| phase::render_outcome(name, answer))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    let report = phase::report(model, ctx, build_brief.to_string(), user).await?;

    // Deterministic postlude: the in-core validator cross-checks, the
    // shell verify gate, and the report-coherence walk, one bounded
    // repair leg, then enforcement.
    gate_report(model, ctx, build_brief, report, &tree_root, &slice_composition, "build", true)
        .await
}

/// Merge a built slice's delta into the baseline per the merge brief.
///
/// A deterministic pre-merge gate validates the staged slice
/// composition (blocking findings park the merge before the delta
/// folds). One judgment leg then folds the delta and runs the brief's
/// host cap-matrix re-verification (agent-run in the lent workspace),
/// and the deterministic postlude re-runs the composition validator
/// against the merged baseline (`.specify/specs/composition.yaml`) plus
/// the report-coherence walk, with one bounded repair leg.
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

    // Deterministic pre-merge gate: an invalid staged slice composition
    // blocks the merge before the delta folds, per the merge brief.
    let staged = tree_root.join(format!(".specify/slices/{slice}/composition.yaml"));
    let staged_findings = validation_findings(&staged);
    if !staged_findings.is_empty() {
        return Ok(Report {
            status: Status::Failure,
            findings: staged_findings.into_iter().map(Finding::blocking).collect(),
            outputs: Vec::new(),
            ui_surface: None,
        });
    }

    let user = format!(
        "Merge slice `{slice}`'s built delta (adapter `{}`). The project workspace is \
         lent to you; the delta below applies against base `{}` (a 3-way merge: the \
         baseline is ours, the delta is theirs). Fold the changes in place — including \
         the slice's `composition.yaml` into the baseline and any operator-curated \
         `tokens.yaml` / `assets.yaml` updates into `design-system/` — then run the \
         merge brief's `## Post-merge — host cap-matrix re-verification` yourself: the \
         cargo / make / gradlew commands run in the lent workspace; this adapter \
         cannot spawn them. The composition validator re-runs deterministically \
         in-guest after your answer. Any gate failure means `status: failure`. Answer \
         with the report body. {SHELF_POINTER}\n\n{delta_block}",
        ctx.adapter_id, delta.base,
    );
    let report = phase::report(model, ctx, merge_brief.to_string(), user).await?;

    gate_report(model, ctx, merge_brief, report, &tree_root, &baseline_composition, "merge", false)
        .await
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
/// resolution) re-run against `composition`, the shell verify gate
/// re-runs when `shell_verify` is set (the build's Phase 8), and a
/// `success` report's declared outputs must exist under the tree root.
/// Residual findings then force `failure` regardless of the answer.
#[expect(clippy::too_many_arguments, reason = "One internal gate call site per operation.")]
async fn gate_report<P: Model>(
    model: &P, ctx: &Context<'_>, brief: &str, mut report: Report, tree_root: &Path,
    composition: &Path, operation: &str, shell_verify: bool,
) -> Result<Report, Error> {
    let gather = |report: &Report| {
        let mut residual = validation_findings(composition);
        if shell_verify {
            residual.extend(shell_verify_findings(tree_root));
        }
        residual.extend(phase::missing_outputs(report, tree_root));
        residual
    };
    let mut residual = gather(&report);
    if !residual.is_empty() {
        let user = format!(
            "The deterministic report gate rejected the {operation} report:\n\n{}\n\n\
             Repair the working tree (or correct the report), then answer with the \
             corrected report body.",
            residual.join("\n"),
        );
        report = phase::report(model, ctx, brief.to_string(), user).await?;
        residual = gather(&report);
    }
    let findings = residual.into_iter().map(Finding::blocking).collect();
    Ok(phase::enforce(report, findings))
}

/// Error-severity findings from the deterministic shell verify gate
/// ([`verify::run`] in `verify` mode), one findings-style line each.
/// Without a declared platform set (`.specify/project.yaml` absent) the
/// gate has nothing to verify against and stays silent; a present but
/// unreadable declaration surfaces as a finding so the bounded repair
/// leg gets a chance to fix the tree.
/// The deterministic bootstrap app-icon gate's error findings (§L):
/// one line per declared UI platform whose launcher icon is not
/// satisfiable. Skipped when no platform set is declared.
fn bootstrap_findings(tree_root: &Path) -> Vec<String> {
    if !tree_root.join(".specify/project.yaml").exists() {
        return Vec::new();
    }
    match verify::run(verify::VerifyMode::BootstrapAppIcon, tree_root) {
        Ok(payload) => payload
            .get("findings")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|f| f.get("severity").and_then(Value::as_str) == Some("error"))
            .map(|f| {
                format!(
                    "- [bootstrap-app-icon] {}: {}",
                    f.get("id").and_then(Value::as_str).unwrap_or("finding"),
                    f.get("message").and_then(Value::as_str).unwrap_or(""),
                )
            })
            .collect(),
        Err(err) => vec![format!("- [bootstrap-app-icon] {err}")],
    }
}

fn shell_verify_findings(tree_root: &Path) -> Vec<String> {
    if !tree_root.join(".specify/project.yaml").exists() {
        return Vec::new();
    }
    match verify::run(verify::VerifyMode::Verify, tree_root) {
        Ok(payload) => payload
            .get("findings")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|f| f.get("severity").and_then(Value::as_str) == Some("error"))
            .map(|f| {
                format!(
                    "- [shell-verify] {}: {}",
                    f.get("id").and_then(Value::as_str).unwrap_or("finding"),
                    f.get("message").and_then(Value::as_str).unwrap_or(""),
                )
            })
            .collect(),
        Err(err) => vec![format!("- [shell-verify] {err}")],
    }
}

/// Render the deterministic shell verify gate's full payload for the
/// report leg's prompt.
fn render_verify_gate(tree_root: &Path) -> String {
    let body = if tree_root.join(".specify/project.yaml").exists() {
        match verify::run(verify::VerifyMode::Verify, tree_root) {
            Ok(payload) => serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string()),
            Err(err) => format!("verify gate could not run: {err}"),
        }
    } else {
        "no declared platform set (`.specify/project.yaml` absent) — gate skipped".to_string()
    };
    format!("### shell verify gate (already run in-guest)\n\n{body}")
}

/// Run the deterministic component-identity clustering (the catalog
/// infer *report* phase) in-guest and render it for the composition
/// leg's prompt. Mirrors the workflow verb's input wiring: the merged
/// baseline, the screenshots candidate cache when present, and the
/// operator `parts.yaml` when present. An absent baseline is an empty
/// report (nothing to name).
fn render_infer_report(tree_root: &Path) -> String {
    let composition = tree_root.join(".specify/specs/composition.yaml");
    let report = if composition.exists() {
        let args = infer::InferArgs {
            composition,
            candidate_cache: Some(tree_root.join(".specify/.cache/component-candidates"))
                .filter(|p| p.is_dir()),
            parts: Some(tree_root.join(".specify/design-system/parts.yaml"))
                .filter(|p| p.is_file()),
            min_occurrences: infer::DEFAULT_MIN_OCCURRENCES,
        };
        match infer::run(&args) {
            Ok(payload) => serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string()),
            Err(err) => format!("inference could not run: {err}"),
        }
    } else {
        "no merged baseline yet — empty report (nothing to name)".to_string()
    };
    format!("### component-identity cluster report (already run in-guest)\n\n{report}")
}

/// Deterministic create-mode scaffolding: stand up the core tree and
/// every declared shell tree that is absent, from the embedded
/// templates, then render a summary block for the write legs' prompts.
/// The app name resolves from an existing shell first, then Pascal case
/// of the `project.yaml` `name:`; when no name resolves (or a scaffold
/// refuses), the note tells the leg's writer the tree is still absent.
fn scaffold_missing_trees(tree_root: &Path) -> String {
    let mut notes: Vec<String> = Vec::new();
    let mut targets: Vec<&'static str> = Vec::new();
    if !shell::shell_present(tree_root, "core") {
        targets.push("core");
    }
    for leg in declared_shell_legs(tree_root) {
        if !shell::shell_present(tree_root, leg.name) {
            targets.push(leg.name);
        }
    }
    if targets.is_empty() {
        return "### scaffold prelude (already run in-guest)\n\nAll declared trees were \
                already present; nothing was scaffolded."
            .to_string();
    }
    match resolve_scaffold_app_name(tree_root) {
        Some(app_name) => {
            for target in targets {
                let common = scaffold::CommonArgs::for_app(app_name.clone());
                let command = match target {
                    "ios" => scaffold::ScaffoldCommand::Ios(scaffold::IosArgs { common }),
                    "android" => scaffold::ScaffoldCommand::Android(scaffold::AndroidArgs {
                        common,
                        android_package: None,
                    }),
                    _ => scaffold::ScaffoldCommand::Core(scaffold::CoreArgs {
                        common,
                        android_package: None,
                    }),
                };
                match scaffold::run_at(tree_root, &command) {
                    Ok(_) => notes.push(format!(
                        "- scaffolded `{target}` for app `{app_name}` from the embedded templates"
                    )),
                    Err(err) => notes.push(format!(
                        "- could not scaffold `{target}` ({err}); stand the tree up per the \
                         write sub-brief"
                    )),
                }
            }
        }
        None => notes.push(
            "- absent trees could not be scaffolded (no app name resolves from the \
             existing shells or `project.yaml` `name:`); stand them up per the write \
             sub-briefs"
                .to_string(),
        ),
    }
    format!(
        "### scaffold prelude (already run in-guest)\n\nThe adapter scaffolded absent \
         declared trees deterministically before this leg:\n{}",
        notes.join("\n"),
    )
}

/// Resolve the scaffold app name: an existing iOS or Android shell
/// first (their trees carry the name), then Pascal case of the
/// `project.yaml` `name:` field.
fn resolve_scaffold_app_name(tree_root: &Path) -> Option<String> {
    if let Ok(name) = ios_scaffold::resolve_ios_app_name(tree_root) {
        return Some(name);
    }
    if let Ok(name) = android_scaffold::resolve_android_app_name(tree_root) {
        return Some(name);
    }
    let source = std::fs::read_to_string(tree_root.join(".specify/project.yaml")).ok()?;
    let doc: Value = serde_saphyr::from_str(&source).ok()?;
    let raw = doc.get("name")?.as_str()?;
    let pascal: String = raw
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            chars.next().map_or_else(String::new, |first| {
                let mut word = first.to_ascii_uppercase().to_string();
                word.push_str(chars.as_str());
                word
            })
        })
        .collect();
    scaffold::validate_app_name(&pascal).ok().map(|()| pascal)
}

/// Re-render one shell's agent-immutable scaffold files around its
/// write leg (the legacy `sync` step; for Android also the idempotent
/// vendored Gradle-wrapper install the legacy Makefile `setup` ran).
/// Returns a note for the report leg when the sync could not run; a
/// clean sync is silent (drift would resurface through the
/// deterministic verify gate anyway).
fn sync_shell_scaffold(tree_root: &Path, name: &str) -> Option<String> {
    if !shell::shell_present(tree_root, name) {
        return None;
    }
    let outcome = match name {
        "ios" => ios_scaffold::sync_ios_scaffold_files(tree_root).map(|_| ()),
        "android" => android_scaffold::sync_android_scaffold_files(tree_root).and_then(|_| {
            let setup = android::run_for_shell_dir(&tree_root.join("Android"));
            if android::setup_exit_code(&setup) == 0 {
                Ok(())
            } else {
                Err(VectisError::InvalidProject {
                    message: format!("gradle wrapper install failed: {setup}"),
                })
            }
        }),
        _ => Ok(()),
    };
    outcome.err().map(|err| format!("- deterministic `{name}` scaffold sync could not run: {err}"))
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
