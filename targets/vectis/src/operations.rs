//! `guidance` / `build` / `merge` over shared [`phase`] scaffolding.
//!
//! Judgment legs sit between a deterministic prepare prelude and a
//! validate / report-coherence postlude. Build order: composition →
//! core → per-shell → review → report. Host verify stays agent-side.

use std::path::Path;

use adapter::registry::Doc;
use adapter::seam::{
    BuildInput, Context, Error, Finding, Input, MergePhase, Platform, PlatformsCapability, Report,
    Severity, Status, TargetMetadata, WorkingTree,
};
use adapter::{AdapterIdentity, Model, Target, phase};
use serde_json::Value;

use crate::{
    VectisError, android_scaffold, infer, ios_scaffold, prepare, registry, scaffold, shell,
    validate, verify,
};

const MAX_VALIDATE_REPAIR_ITERATIONS: usize = 2;

const REFERENCES_POINTER: &str = "Every prompt, reference, and rule document this adapter ships is \
     served by the granted `vectis-references` MCP references (`list_docs` / `read_doc`, \
     adapter-relative paths like `references/hard-rules-core.md` or \
     `prompts/build/ios/write.md`); fetch documents the prompts cite lazily from there.";

/// Host-FS bootstrap contract for greenfield trees (typescript-style).
///
/// The target guest only mounts the project root, so a sibling
/// `../vectis-exemplar` is invisible in-guest. The build agent performs
/// allowlisted copy via [`scaffold::materialize`] on the host filesystem.
const BINDING_NOTE: &str = "Resolve `$TEMPLATE_DIR` before any greenfield write: default \
                            `../vectis-exemplar` relative to the consumer project root, or the \
                            absolute path in `VECTIS_EXEMPLAR_DIR`. Clone \
                            https://github.com/augentic/vectis-exemplar.git if missing — fail \
                            closed; do not invent a scaffold or version pins. This is **template \
                            materialize** (`vectis::scaffold::materialize`), not asset \
                            materialize (`vectis::materialize`). Strip grammar: \
                            `$TEMPLATE_DIR/AGENTS.md` (not copied). Late-cap adoption: \
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
            emery_floor: Some("0.28.0".to_string()),
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

    #[expect(
        clippy::too_many_lines,
        reason = "One linear leg-by-leg walk of the prompt's phase order; splitting hides the order."
    )]
    async fn build<P: Model>(
        model: &P, ctx: &Context<'_>, slice: &str, inputs: &[Input], tree: &WorkingTree,
    ) -> Result<Report, Error> {
        let tree_root = ctx.tree_root(tree);
        let slice_dir_rel = format!(".emery/slices/{slice}");
        let slice_dir = tree_root.join(&slice_dir_rel);
        let slice_composition = slice_dir.join("composition.yaml");
        let inputs_block = phase::render_inputs(inputs);
        let build_prompt = registry::body("prompts/build.md");

        // The materialize scope derives from the same declared-platform
        // read as the shell legs, so a core-only project materializes
        // nothing for shells it will not build.
        let shell_platforms: Vec<String> =
            declared_shell_legs(&tree_root).iter().map(|leg| leg.name.to_string()).collect();
        let prelude = prepare::materialize_step(&slice_dir, &tree_root, &shell_platforms)
            .map_err(error_from_vectis)?;
        let prelude_block = render_prelude(&prelude);

        // Bootstrap gate (§L): the launcher app-icon must be satisfiable for
        // every declared UI platform before any write leg.
        let bootstrap = bootstrap_findings(&tree_root);
        if !bootstrap.is_empty() {
            return Ok(Report {
                status: Status::Failure,
                findings: bootstrap.into_iter().map(Finding::blocking).collect(),
                outputs: Vec::new(),
                ui_surface: None,
            });
        }

        // Component *identity* is deterministic and runs in-guest (the
        // name-free cluster report); *naming* is the leg's judgment,
        // recorded as a bindings file the workflow's deterministic bind
        // bookkeeping projects into the catalog.
        let infer_block = render_infer_report(&tree_root);
        let system =
            assemble(&["prompts/build.md", "prompts/guidance.md", "prompts/build/composition.md"]);
        let user = format!(
            "Run component inference (Step 0.5) and composition regeneration (Phase 1) of \
         the vectis build for slice `{slice}` (adapter `{}`).\n\n\
         The project workspace is lent to you. The adapter already ran the \
         deterministic component-identity clustering in-guest — the name-free cluster \
         report is below; do not attempt to re-run it. Decide what each unbound \
         cluster is and what to call it per the build prompt's Step 0.5, write your \
         `{{ fingerprint -> slug }}` decisions to \
         `{slice_dir_rel}/build/component-bindings.yaml` (echo populated `bound-slug` \
         names verbatim — operator parts carry naming authority), then regenerate \
         `{slice_dir_rel}/composition.yaml` from the slice artifacts per the \
         composition prompt, treating your fresh bindings plus the existing catalog \
         as the effective component set. For a slice with no UI surface, write no \
         composition and answer with `applicable: false`.\n\n\
         {infer_block}\n\n\
         {prelude_block}\n\n{REFERENCES_POINTER}\n\n{inputs_block}",
            ctx.adapter_id,
        );
        let composition = phase::phase(model, ctx, system, user, "composition").await?;

        // The per-shell write prompts require the composition gate passed
        // before any platform phase: an exhausted repair budget parks the
        // slice with a deterministic failure report.
        let residual =
            composition_gate(model, ctx, slice, &slice_dir_rel, &slice_composition).await?;
        if !residual.is_empty() {
            return Ok(Report {
                status: Status::Failure,
                findings: residual.into_iter().map(Finding::blocking).collect(),
                outputs: Vec::new(),
                ui_surface: None,
            });
        }

        let scaffold_block = scaffold_missing_trees(&tree_root);

        // The core verify-repair loop crosses the write and test prompts
        // (a cargo failure re-enters the writer), so one agent leg holds
        // them together.
        let system =
            assemble(&["prompts/build.md", "prompts/build/core/write.md", "prompts/build/test.md"]);
        let user = format!(
            "Run the Crux core phases (2-3) of the vectis build for slice `{slice}`: \
         generate or update the shared core per the core write prompt, write the \
         Crux tests, then run the test prompt's core verify-repair loop yourself — \
         the cargo check / clippy / test commands run in the lent workspace; this \
         adapter cannot spawn them. Detect create vs update mode from the tree. When \
         the template-materialize prelude below lists absent trees, materialize from \
         `$TEMPLATE_DIR` first (host FS) before writing feature code.\n\n\
         {BINDING_NOTE}\n\n{scaffold_block}\n\n{REFERENCES_POINTER}\n\n{inputs_block}",
        );
        let core = phase::phase(model, ctx, system, user, "core").await?;

        // DX files stay consistent with `$TEMPLATE_DIR` after identity
        // substitution; the guest does not re-render them from embedded
        // templates (sibling checkout is outside the project mount).
        let mut shell_outcomes: Vec<(&'static str, phase::PhaseAnswer)> = Vec::new();
        for shell in declared_shell_legs(&tree_root) {
            let system = assemble(&["prompts/build.md", shell.write_prompt]);
            let user = format!(
                "Run the {name} shell phase of the vectis build for slice `{slice}`: \
             generate or update the shell per the write prompt. When the \
             template-materialize prelude below lists absent trees, materialize from \
             `$TEMPLATE_DIR` on the host FS first — do not hand-invent scaffold \
             boilerplate or version pins. Then run the write prompt's \
             orchestrator-owned verify loop yourself in the lent workspace — this \
             adapter cannot spawn host commands. Keep DX files (Makefiles, \
             `project.yml`, assembly Gradle files, BoltFFI pack recipes) consistent \
             with `$TEMPLATE_DIR` after identity substitution; refresh by re-copying \
             those paths from the template, never by guessing pins. When the slice \
             introduces no work for this shell, write nothing \
             and answer with `applicable: false`; when a host prerequisite is \
             missing, stop per the prompt's deferred contract and report it in your \
             summary.\n\n{BINDING_NOTE}\n\n{scaffold_block}\n\n{REFERENCES_POINTER}",
                name = shell.name,
            );
            let answer = phase::phase(model, ctx, system, user, shell.name).await?;
            shell_outcomes.push((shell.name, answer));
        }

        let mut review_prompts = vec!["prompts/build.md", "prompts/build/core/review.md"];
        review_prompts
            .extend(declared_shell_legs(&tree_root).iter().map(|shell| shell.review_prompt));
        let system = assemble(&review_prompts);
        let user = format!(
            "Run the review phases (6-7) of the vectis build for slice `{slice}`: spawn \
         the core reviewer team and, for each in-scope shell, its platform reviewer \
         team per the review prompts (reviewers run in parallel), then run the build \
         prompt's `## § Consolidate review findings` and drive any remediation in the \
         lent workspace. {REFERENCES_POINTER}",
        );
        let review = phase::phase(model, ctx, system, user, "review").await?;

        // The deterministic shell verify gate runs in-guest and feeds the
        // report leg, gated by the derived answer schema.
        let verify_block = render_verify_gate(&tree_root);
        let mut outcomes = vec![("composition", &composition), ("core", &core)];
        outcomes.extend(shell_outcomes.iter().map(|(name, answer)| (*name, answer)));
        outcomes.push(("review", &review));
        let user = format!(
            "Write the build report for slice `{slice}` per the build prompt's `## Build \
         report`. The adapter already ran the deterministic shell verify gate in-guest \
         — its findings are below and re-run after your answer; a missing or empty \
         tree for a supported declared platform forces `status: failure`, so repair \
         the tree first when the gate reports errors. Then mark the completed \
         `tasks.md` checkboxes in the slice directory per the prompt before answering. \
         A `success` report carries only non-blocking findings; an exhausted \
         verify-repair budget, a failed composition gate, or unresolved blocking \
         review findings mean `status: failure`. Declare `outputs[]` per supported \
         platform with paths relative to the project root, and set \
         `ui-surface.screens` from the slice's own screen count.\n\n\
         {verify_block}\n\n\
         Phase outcomes:\n{}",
            outcomes
                .iter()
                .map(|(name, answer)| phase::render_outcome(name, answer))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        let report = phase::report(model, ctx, build_prompt.to_string(), user).await?;

        let mut report = gate_report(
            model,
            ctx,
            build_prompt,
            report,
            &tree_root,
            &slice_composition,
            "build",
            true,
        )
        .await?;

        // Suggestion findings only — they ride the report but never fail
        // it or trigger the repair leg.
        let coherence = ui_surface_coherence(&report, &slice_composition);
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
            let staged_findings = validation_findings(&staged);
            if staged_findings.is_empty() {
                return Ok(Report::success());
            }
            return Ok(Report {
                status: Status::Failure,
                findings: staged_findings.into_iter().map(Finding::blocking).collect(),
                outputs: Vec::new(),
                ui_surface: None,
            });
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

        gate_report(
            model,
            ctx,
            merge_prompt,
            report,
            &tree_root,
            &baseline_composition,
            "merge-postflight",
            false,
        )
        .await
    }
}

struct ShellLeg {
    name: &'static str,
    write_prompt: &'static str,
    review_prompt: &'static str,
}

const SHELL_LEGS: [ShellLeg; 2] = [
    ShellLeg {
        name: "ios",
        write_prompt: "prompts/build/ios/write.md",
        review_prompt: "prompts/build/ios/review.md",
    },
    ShellLeg {
        name: "android",
        write_prompt: "prompts/build/android/write.md",
        review_prompt: "prompts/build/android/review.md",
    },
];

// Absent / unreadable `project.yaml.platforms` → both shells; `web` /
// `desktop` have no prompt and never match.
fn declared_shell_legs(project_root: &Path) -> Vec<&'static ShellLeg> {
    let declared = declared_platforms(project_root);
    SHELL_LEGS
        .iter()
        .filter(|leg| declared.as_ref().is_none_or(|set| set.iter().any(|p| p == leg.name)))
        .collect()
}

fn declared_platforms(project_root: &Path) -> Option<Vec<String>> {
    let source = std::fs::read_to_string(project_root.join(".emery/project.yaml")).ok()?;
    let doc: Value = serde_saphyr::from_str(&source).ok()?;
    let platforms = doc.get("platforms")?.as_array()?;
    Some(platforms.iter().filter_map(Value::as_str).map(str::to_string).collect())
}

async fn composition_gate<P: Model>(
    model: &P, ctx: &Context<'_>, slice: &str, slice_dir_rel: &str, composition: &Path,
) -> Result<Vec<String>, Error> {
    let mut findings = validation_findings(composition);
    for _ in 0..MAX_VALIDATE_REPAIR_ITERATIONS {
        if findings.is_empty() {
            break;
        }
        let system = assemble(&["prompts/build.md", "prompts/build/composition.md"]);
        let user = format!(
            "The deterministic composition validator found blocking issues in slice \
             `{slice}`'s regenerated `{slice_dir_rel}/composition.yaml`. Repair the \
             composition (or the operator-curated manifests it references) in place per \
             the composition prompt's validator gate.\n\n{}\n\n\
             Answer `applicable: true` with a summary of the repairs. {REFERENCES_POINTER}",
            findings.join("\n"),
        );
        phase::phase(model, ctx, system, user, "composition-repair").await?;
        findings = validation_findings(composition);
    }
    Ok(findings)
}

fn assemble(prompts: &[&str]) -> String {
    let bodies: Vec<&str> = prompts.iter().map(|prompt| registry::body(prompt)).collect();
    phase::assemble_system(&bodies)
}

#[expect(clippy::too_many_arguments, reason = "One internal gate call site per operation.")]
async fn gate_report<P: Model>(
    model: &P, ctx: &Context<'_>, prompt: &str, mut report: Report, tree_root: &Path,
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
        report = phase::report(model, ctx, prompt.to_string(), user).await?;
        residual = gather(&report);
    }
    let findings = residual.into_iter().map(Finding::blocking).collect();
    Ok(phase::enforce(report, findings))
}

fn bootstrap_findings(tree_root: &Path) -> Vec<String> {
    if !tree_root.join(".emery/project.yaml").exists() {
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
    if !tree_root.join(".emery/project.yaml").exists() {
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

fn render_verify_gate(tree_root: &Path) -> String {
    let body = if tree_root.join(".emery/project.yaml").exists() {
        match verify::run(verify::VerifyMode::Verify, tree_root) {
            Ok(payload) => serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string()),
            Err(err) => format!("verify gate could not run: {err}"),
        }
    } else {
        "no declared platform set (`.emery/project.yaml` absent) — gate skipped".to_string()
    };
    format!("### shell verify gate (already run in-guest)\n\n{body}")
}

fn render_infer_report(tree_root: &Path) -> String {
    let composition = tree_root.join(".emery/specs/composition.yaml");
    let report = if composition.exists() {
        let args = infer::InferArgs {
            composition,
            candidate_cache: Some(tree_root.join(".emery/.cache/component-candidates"))
                .filter(|p| p.is_dir()),
            parts: Some(tree_root.join(".emery/design-system/parts.yaml")).filter(|p| p.is_file()),
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

fn scaffold_missing_trees(tree_root: &Path) -> String {
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
        return "### template-materialize prelude\n\nAll declared trees were already present; \
                skip greenfield materialize. For DX pin drift, re-copy the drifted paths from \
                `$TEMPLATE_DIR` with identity substitution — never invent versions."
            .to_string();
    }
    let identity = resolve_scaffold_app_name(tree_root).map_or_else(
        || {
            "- Resolve `app_name` (PascalCase from `design.md` `App` / `project.yaml` \
             `name:`) and `android_package` before materialize; refuse to invent them."
                .to_string()
        },
        |app_name| {
            let package = scaffold::default_android_package(&app_name);
            format!(
                "- Suggested identity: app_name=`{app_name}`, android_package=`{package}` \
                 (override the package from `design.md` when it declares one)."
            )
        },
    );
    let absent = targets.iter().map(|t| format!("`{t}`")).collect::<Vec<_>>().join(", ");
    format!(
        "### template-materialize prelude\n\nAbsent declared trees: {absent}. The guest did \
         **not** write them — target guests cannot see a sibling `$TEMPLATE_DIR`.\n\n\
         Before any write leg for those trees:\n\
         1. Resolve `$TEMPLATE_DIR` (`VECTIS_EXEMPLAR_DIR` or `../vectis-exemplar`); fail \
         closed if missing.\n\
         2. Run the allowlisted copy in `vectis::scaffold::materialize` (root DX + \
         `shared/` + `iOS/` + `Android/` + `supply-chain/` + `.maestro/`; never `web/`, \
         `.git/`, `.github/`, or `AGENTS.md`). One materialize covers the workspace — do \
         not invent per-shell scaffolds or pins.\n\
         3. Strip `VECTIS-OPTIONAL` per `$TEMPLATE_DIR/AGENTS.md` against the \
         `design.md` capability matrix (`http` / `kv` / `time` / `sse` / `demo`).\n\
         4. iOS: regenerate the Xcode project (`make -C iOS generate-project` / \
         `xcodegen`) — `.xcodeproj` is denylisted on purpose.\n\
         {identity}"
    )
}

fn resolve_scaffold_app_name(tree_root: &Path) -> Option<String> {
    if let Ok(name) = ios_scaffold::resolve_ios_app_name(tree_root) {
        return Some(name);
    }
    if let Ok(name) = android_scaffold::resolve_android_app_name(tree_root) {
        return Some(name);
    }
    let source = std::fs::read_to_string(tree_root.join(".emery/project.yaml")).ok()?;
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

// A4 self-consistency only (`suggestion`); never fails the report.
fn ui_surface_coherence(report: &Report, composition: &Path) -> Vec<Finding> {
    let Some(ui_surface) = report.ui_surface else {
        return Vec::new();
    };
    let has_surface = composition_declares_surface(composition);
    let mut warnings = Vec::new();
    if ui_surface.screens == 0 && has_surface {
        warnings.push(ui_surface_warning(
            "composition-unexpected-for-non-ui-slice",
            "the report claims `ui-surface.screens: 0` but produced a non-empty \
             composition.yaml; the UI-surface judgement contradicts the composition output"
                .to_string(),
        ));
    }
    if ui_surface.screens > 0 && !has_surface {
        warnings.push(ui_surface_warning(
            "composition-empty-for-ui-slice",
            format!(
                "the report claims `ui-surface.screens: {}` but produced an absent or empty \
                 composition.yaml; the UI-surface judgement contradicts the composition output",
                ui_surface.screens
            ),
        ));
    }
    warnings
}

fn ui_surface_warning(rule_id: &str, detail: String) -> Finding {
    Finding {
        rule_id: Some(rule_id.to_string()),
        severity: Severity::Suggestion,
        detail,
    }
}

fn composition_declares_surface(path: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };
    let Ok(doc) = serde_saphyr::from_str::<Value>(&text) else {
        return false;
    };

    if doc.get("screens").and_then(Value::as_object).is_some_and(|s| !s.is_empty()) {
        return true;
    }

    doc.get("delta").and_then(Value::as_object).is_some_and(|delta| {
        ["added", "modified", "removed"]
            .iter()
            .any(|key| delta.get(*key).and_then(Value::as_object).is_some_and(|m| !m.is_empty()))
    })
}

// Absent composition is clean (core-only / pre-first-merge).
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

fn error_from_vectis(err: VectisError) -> Error {
    match err {
        VectisError::Io(io) => Error::Io(io.to_string()),
        VectisError::InvalidProject { message } => Error::InvalidRequest(message),
        VectisError::Internal { message } => Error::Internal(message),
    }
}

fn render_prelude(summary: &Value) -> String {
    format!(
        "### prepare prelude (already run in-guest)\n\n\
         The adapter resolved the slice's materialize scope and ran the deterministic \
         `materialize assets` step before this leg; do not re-run it. Summary:\n\n{}",
        serde_json::to_string(summary).unwrap_or_else(|_| "{}".to_string()),
    )
}
