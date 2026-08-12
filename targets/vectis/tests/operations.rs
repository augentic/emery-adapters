//! Vectis operation behavior across the RFC-90 six-operation split:
//! `build` (generation), `verify` / `repair` / `review` (one pass
//! each), and the phased `merge` gates.

use std::fs;
use std::path::Path;

use adapter::answers::PHASE_REPORT_ANSWER_SCHEMA;
use adapter::seam::{
    ArtifactStage, BuildContext, Context, DiagnosticSource, FindingArtifact, FindingEvidence,
    FindingKind, Input, MergePhase, Payload, PhaseFinding, PhaseOutcome, PhaseReport, PhaseSource,
    RepairOrigin, Severity, Status, Workspace, WritableArtifact, WritableArtifactKind,
};
use adapter::{Format, Request, Target as _};
use omnia_testkit::model::{Harness, mcp_grants};
use tempfile::TempDir;
use vectis::Adapter;

const PHASE_DONE: &str = r#"{"applicable":true,"summary":"phase complete"}"#;
const SHELL_SKIPPED: &str = r#"{"applicable":false,"summary":"no shell work in this slice"}"#;
const PHASE_REPORT_CLEAN: &str = r#"{"outcome":"completed","source":"model-assisted"}"#;
const SUCCESS_REPORT: &str = r#"{"status":"success","findings":[]}"#;

fn ctx<'a>(root: &'a Path, mcp_url: Option<&str>) -> Context<'a> {
    Context {
        adapter_id: "target:vectis",
        project_root: root,
        mcp_url: mcp_url.map(str::to_owned),
        lend: root.display().to_string(),
    }
}

// The degenerate stage-less shape: workspace root and artifact root
// both point at the test tree, no lent artifact stage.
fn workspace(root: &Path) -> Workspace {
    Workspace {
        id: "ws-1".to_string(),
        root: root.display().to_string(),
        artifacts: root.display().to_string(),
        artifact_stage: None,
    }
}

// The RFC-90 shape: the engine lends a writable artifact stage beside
// the workspace.
fn staged_workspace(root: &Path, stage: &Path) -> Workspace {
    Workspace {
        artifact_stage: Some(ArtifactStage {
            id: "stage-1".to_string(),
            root: stage.display().to_string(),
        }),
        ..workspace(root)
    }
}

fn schema_format(request: &Request) -> (&str, &str) {
    match &request.format {
        Format::Schema(schema) => (&schema.name, &schema.schema),
        other => panic!("expected schema format, got {other:?}"),
    }
}

/// Re-bloat guard: each leg's system assemble is a pure function over
/// the embedded prose registry, so its byte size is locked at the
/// measured baseline plus ~10% headroom.
fn assert_system_budget(request: &Request, leg: &str, budget: usize) {
    let bytes = request.system.as_deref().map_or(0, str::len);
    println!("{leg} system assemble: {bytes} bytes (budget {budget})");
    assert!(
        bytes <= budget,
        "{leg} system assemble is {bytes} bytes, over its {budget}-byte budget — \
         trim the assemble or deliberately raise the budget"
    );
}

/// A check-pass report is sanitized: no outputs, no UI surface, no
/// continuation change.
fn assert_check_shape(report: &PhaseReport) {
    assert!(report.outputs.is_empty(), "check passes declare no outputs");
    assert!(report.ui_surface.is_none(), "check passes carry no UI surface");
    assert!(report.next_continuation.is_none(), "vectis preserves the continuation");
}

fn blocking_finding(title: &str) -> PhaseFinding {
    PhaseFinding {
        id: "GATE-0001".to_string(),
        rule_id: None,
        related_rule_ids: Vec::new(),
        title: title.to_string(),
        severity: Severity::Important,
        source: DiagnosticSource::Deterministic,
        kind: FindingKind::Violation,
        artifact: FindingArtifact::Code,
        location: None,
        evidence: FindingEvidence::Snippet {
            value: title.to_string(),
        },
        impact: "blocking".to_string(),
        remediation: "fix it".to_string(),
        confidence: None,
        fingerprint: String::new(),
    }
}

#[test]
fn metadata_grants() {
    let metadata = Adapter::metadata();
    assert_eq!(metadata.emery_floor.as_deref(), Some("0.38.0"));
    let grants: Vec<(&str, WritableArtifactKind)> = metadata
        .writable_artifacts
        .iter()
        .map(|artifact| (artifact.path.as_str(), artifact.kind))
        .collect();
    assert_eq!(
        grants,
        vec![
            ("tasks.md", WritableArtifactKind::File),
            ("composition.yaml", WritableArtifactKind::File),
            ("build", WritableArtifactKind::Tree),
        ],
        "RFC-90 D5: tasks, composition, and the build bookkeeping subtree"
    );
    assert_eq!(metadata.writable_artifacts[2], WritableArtifact::tree("build"));
    assert!(metadata.platforms.is_some_and(|capability| capability.required));
}

#[tokio::test]
async fn build_phase_legs() {
    let tmp = TempDir::new().unwrap();
    let stage = TempDir::new().unwrap();
    let model = Harness::answering([
        PHASE_DONE,    // composition
        PHASE_DONE,    // core
        SHELL_SKIPPED, // ios
        SHELL_SKIPPED, // android
        r#"{"outcome":"completed","source":"model-assisted",
            "outputs":[{"platform":"core","path":"shared/"}],
            "ui-surface":{"screens":1}}"#, // build report
    ]);
    let input = |path: &str| Payload::Path(path.to_string());
    let inputs = vec![
        Input::Proposal(input(".emery/slices/demo/proposal.md")),
        Input::Spec(input(".emery/slices/demo/specs/core/spec.md")),
        Input::Design(input(".emery/slices/demo/design.md")),
    ];

    let report = Adapter::build(
        &model,
        &ctx(tmp.path(), Some("http://references/mcp")),
        "demo",
        &inputs,
        &BuildContext::default(),
        &staged_workspace(tmp.path(), stage.path()),
    )
    .await
    .unwrap();

    assert_eq!(report.outcome, PhaseOutcome::Completed);
    assert_eq!(
        report.source,
        PhaseSource::ModelAssisted,
        "no project.yaml and no staged composition — no in-guest check contributed"
    );
    assert_eq!(report.outputs.len(), 1, "build carries the per-platform outputs");
    assert_eq!(report.ui_surface.map(|surface| surface.screens), Some(1));
    assert!(report.next_continuation.is_none(), "vectis carries no writer-session state");

    let requests = model.requests();
    assert_eq!(
        requests.len(),
        5,
        "composition, core, two shells, then one report leg — no \
         verify / repair / review legs inside build"
    );
    // Budget = measured baseline (per-leg comment, 2026-08-11) + ~10%.
    for (i, (leg, budget)) in [
        ("composition", 35_400),  // baseline 32_199
        ("core", 26_700),         // baseline 24_275
        ("ios", 23_200),          // baseline 21_118
        ("android", 25_400),      // baseline 23_056
        ("build-report", 18_400), // baseline 16_713
    ]
    .into_iter()
    .enumerate()
    {
        assert_system_budget(&requests[i], leg, budget);
    }

    let stage_display = stage.path().display().to_string();
    assert_composition_leg(&requests[0], &stage_display);
    assert_generation_legs(&requests);
    assert_report_leg(&requests[4], &stage_display);
}

/// Composition leg: assemble, path-form inputs, stage-routed writes.
fn assert_composition_leg(request: &Request, stage_display: &str) {
    let system = request.system.as_deref().unwrap();
    assert!(system.contains("# Vectis target — build prompt"), "build prompt in system");
    assert!(
        !system.contains("# Vectis target — `guidance`"),
        "guidance stays on the guidance operation — never assembled into composition"
    );
    assert!(system.contains("# Vectis build — composition"), "composition prompt in system");
    assert!(
        system.contains("## Step 0.5 — component inference"),
        "Step 0.5 contract rides the composition prompt, not the shared preamble"
    );
    let user = &request.messages[0].content;
    assert!(
        user.contains("/.emery/slices/demo/proposal.md")
            && user.contains("/.emery/slices/demo/design.md"),
        "typed inputs render as artifact-rooted path sections: {user}"
    );
    assert!(!user.contains("PROPOSAL-BODY"), "artifact bodies are not inlined");
    assert!(user.contains("Read each path"), "read-before-writing instruction rides the inputs");
    assert!(user.contains("slice `demo`"), "slice named");
    assert!(user.contains("prepare prelude"), "prelude summary feeds the first leg");
    assert!(user.contains("component-identity cluster report"), "in-guest infer report feeds it");
    assert!(
        user.contains(&format!("{stage_display}/build/component-bindings.yaml")),
        "bindings write routes to the artifact stage: {user}"
    );
    assert!(
        user.contains(&format!("{stage_display}/composition.yaml")),
        "composition write routes to the artifact stage"
    );
    assert!(user.contains("vectis-references"), "user prompt points at the MCP references");
    let (name, schema) = schema_format(request);
    assert_eq!(name, "composition");
    let compiled = serde_json::from_str::<serde_json::Value>(schema).unwrap();
    assert!(jsonschema::validator_for(&compiled).is_ok(), "internal schema compiles");
    assert!(
        request.workspace.as_deref().is_some_and(|path| !path.is_empty()),
        "the build leg lends the prepared workspace path"
    );
    assert_eq!(mcp_grants(request)[0].url, "http://references/mcp");
}

/// Core and shell legs are generation-only.
fn assert_generation_legs(requests: &[Request]) {
    let core = &requests[1];
    assert_eq!(schema_format(core).0, "core");
    assert!(core.system.as_deref().unwrap().contains("# Vectis build — core (write)"));
    assert!(core.system.as_deref().unwrap().contains("# Vectis build — Crux tests"));
    assert!(
        core.messages[0].content.contains("generation-only pass"),
        "core leg is generation only"
    );
    assert!(
        core.messages[0].content.contains("separate verify operation"),
        "verification is dispatched by the engine, not the core leg"
    );
    let ios = &requests[2];
    assert_eq!(schema_format(ios).0, "ios");
    assert!(ios.system.as_deref().unwrap().contains("# Vectis build — iOS shell (write)"));
    assert!(ios.messages[0].content.contains("applicable: false"), "shell legs may self-skip");
    assert!(ios.messages[0].content.contains("generation-only pass"));
    let android = &requests[3];
    assert_eq!(schema_format(android).0, "android");
    assert!(android.system.as_deref().unwrap().contains("# Vectis build — Android shell (write)"));
}

/// Report leg: the typed phase report, tasks marked in the stage.
fn assert_report_leg(request: &Request, stage_display: &str) {
    let (name, schema) = schema_format(request);
    assert_eq!(name, "build-report");
    assert_eq!(schema, PHASE_REPORT_ANSWER_SCHEMA);
    let report_system = request.system.as_deref().unwrap();
    assert!(
        report_system.contains("# Vectis build — phase report"),
        "phase-report contract rides the report phase prompt"
    );
    let report_user = &request.messages[0].content;
    assert!(report_user.contains("no shell work"), "phase outcomes feed the report leg");
    assert!(
        report_user.contains(&format!("{stage_display}/tasks.md")),
        "tasks bookkeeping routes to the artifact stage"
    );
    assert!(
        report_user.contains("never write `build/report.yaml`"),
        "terminal report assembly is engine-owned"
    );
}

#[tokio::test]
async fn core_only_skips_shells() {
    let tmp = TempDir::new().unwrap();
    let stage = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join(".emery")).unwrap();
    fs::write(tmp.path().join(".emery/project.yaml"), "name: demo-app\nplatforms:\n  - core\n")
        .unwrap();
    let model = Harness::answering([PHASE_DONE, PHASE_DONE, PHASE_REPORT_CLEAN]);

    let report = Adapter::build(
        &model,
        &ctx(tmp.path(), None),
        "demo",
        &[],
        &BuildContext::default(),
        &staged_workspace(tmp.path(), stage.path()),
    )
    .await
    .unwrap();

    assert_eq!(report.outcome, PhaseOutcome::Completed);
    assert_eq!(
        report.source,
        PhaseSource::Hybrid,
        "the deterministic bootstrap gate ran alongside the model legs"
    );
    let requests = model.requests();
    assert_eq!(requests.len(), 3, "composition, core, report — no shell legs");
    assert_eq!(schema_format(&requests[1]).0, "core");
    assert_eq!(schema_format(&requests[2]).0, "build-report");
    let core_user = &requests[1].messages[0].content;
    assert!(
        core_user.contains("template-materialize prelude"),
        "prelude names the host-side template materialize contract"
    );
}

#[tokio::test]
async fn guest_does_not_embed_scaffold() {
    let tmp = TempDir::new().unwrap();
    let stage = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join(".emery")).unwrap();
    fs::write(tmp.path().join(".emery/project.yaml"), "name: demo-app\nplatforms:\n  - core\n")
        .unwrap();
    // Absent `shared/` → prelude asks the host agent to materialize; the
    // guest must not write from embedded templates.
    let model = Harness::answering([PHASE_DONE, PHASE_DONE, PHASE_REPORT_CLEAN]);

    Adapter::build(
        &model,
        &ctx(tmp.path(), None),
        "demo",
        &[],
        &BuildContext::default(),
        &staged_workspace(tmp.path(), stage.path()),
    )
    .await
    .unwrap();

    let core_user = &model.requests()[1].messages[0].content;
    assert!(core_user.contains("Absent declared trees: `core`"));
    assert!(core_user.contains("$TEMPLATE_DIR"));
    assert!(core_user.contains("references/template-materialize.md"));
    assert!(
        core_user.contains("ui-contract/"),
        "prelude must name ui-contract in the allowlisted copy set"
    );
    assert!(
        !tmp.path().join("shared/src/app.rs").is_file(),
        "guest must not write trees from embedded templates"
    );
}

#[tokio::test]
async fn build_blocks_on_invalid_composition() {
    let tmp = TempDir::new().unwrap();
    let stage = TempDir::new().unwrap();
    // The composition leg leaves an unparseable staged composition; the
    // in-guest validator blocks generation and its findings ride the
    // build report — no repair loop runs inside build.
    fs::write(stage.path().join("composition.yaml"), "screens: [broken\n").unwrap();
    let model = Harness::answering([PHASE_DONE]);

    let report = Adapter::build(
        &model,
        &ctx(tmp.path(), None),
        "demo",
        &[],
        &BuildContext::default(),
        &staged_workspace(tmp.path(), stage.path()),
    )
    .await
    .unwrap();

    assert_eq!(report.outcome, PhaseOutcome::Completed);
    assert_eq!(
        report.source,
        PhaseSource::Hybrid,
        "in-guest validator findings plus a model leg is a hybrid report"
    );
    assert!(!report.findings.is_empty(), "validator findings ride the report");
    let finding = &report.findings[0];
    assert_eq!(finding.severity, Severity::Important);
    assert_eq!(finding.kind, FindingKind::Violation);
    assert_eq!(finding.source, DiagnosticSource::Deterministic);
    assert_eq!(finding.artifact, FindingArtifact::Composition);
    assert!(finding.title.contains("[composition]"), "finding names the validator: {finding:?}");
    assert!(report.outputs.is_empty(), "no platform phase ran, so no outputs are declared");

    let requests = model.requests();
    assert_eq!(requests.len(), 1, "the composition leg only — repair routing is engine policy");
}

// RFC-90 grants: a slice-local `assets.yaml` staged by the engine is
// read from the stage, but the materialize prelude's exports must land
// under the product workspace's `design-system/` — never onto the
// stage, which only grants `tasks.md`, `composition.yaml`, and
// `build/`.
#[tokio::test]
async fn build_materialize_exports_land_in_workspace() {
    const SQUARE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <path fill="#336699" d="M2 2h20v20H2z"/>
</svg>"##;

    let tmp = TempDir::new().unwrap();
    let stage = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join(".emery")).unwrap();
    fs::write(
        tmp.path().join(".emery/project.yaml"),
        "name: demo-app\nplatforms:\n  - core\n  - ios\n",
    )
    .unwrap();
    // Workspace design-system baseline satisfies the bootstrap app-icon
    // gate for the declared ios platform.
    fs::create_dir_all(tmp.path().join("design-system/assets")).unwrap();
    fs::write(tmp.path().join("design-system/assets/launcher.svg"), SQUARE).unwrap();
    fs::write(
        tmp.path().join("design-system/assets.yaml"),
        "version: 1\napp-icon: launcher\nassets:\n  launcher:\n    alt: Launcher\n    kind: \
         vector\n    role: app-icon\n    source: assets/launcher.svg\n",
    )
    .unwrap();
    // The slice-local inventory arrives on the lent stage; its master
    // reads relative to the inventory (the stage), its exports must not.
    fs::create_dir_all(stage.path().join("assets")).unwrap();
    fs::write(stage.path().join("assets/check.svg"), SQUARE).unwrap();
    let staged_inventory = "version: 1\nassets:\n  check:\n    alt: Check\n    kind: vector\n    \
                            role: icon\n    source: assets/check.svg\n";
    fs::write(stage.path().join("assets.yaml"), staged_inventory).unwrap();

    let model = Harness::answering([PHASE_DONE, PHASE_DONE, SHELL_SKIPPED, PHASE_REPORT_CLEAN]);
    let report = Adapter::build(
        &model,
        &ctx(tmp.path(), None),
        "demo",
        &[],
        &BuildContext::default(),
        &staged_workspace(tmp.path(), stage.path()),
    )
    .await
    .unwrap();
    assert_eq!(report.outcome, PhaseOutcome::Completed);

    // Exports land under the workspace design-system, where the shell
    // write legs expect them.
    assert!(
        tmp.path().join("design-system/assets/exports/ios/check.imageset/check.pdf").is_file(),
        "materialized export missing from the workspace design-system"
    );

    // The staged tree gains nothing outside the declared grants: the
    // two seeded entries only, and no auto-pin write-back mutates the
    // staged inventory.
    let mut staged_entries: Vec<String> = fs::read_dir(stage.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    staged_entries.sort();
    assert_eq!(
        staged_entries,
        vec!["assets".to_string(), "assets.yaml".to_string()],
        "the stage gains no entries outside the declared grants"
    );
    assert!(
        !stage.path().join("assets/exports").exists(),
        "no exports may land on the artifact stage"
    );
    assert_eq!(
        fs::read_to_string(stage.path().join("assets.yaml")).unwrap(),
        staged_inventory,
        "auto-pin write-back must not touch the staged inventory"
    );
}

// Dirty scripted answer through a check pass: outputs, UI surface,
// continuation, and `tool` finding attributions are sanitized in code,
// and a not-applicable outcome clears `written`.
#[tokio::test]
async fn verify_dirty_answer_sanitized() {
    let tmp = TempDir::new().unwrap();
    let dirty = r#"{"outcome":"not-applicable","source":"tool","findings":[{
        "title":"xcodebuild failed",
        "severity":"important",
        "source":"tool",
        "kind":"violation",
        "artifact":"code",
        "evidence":{"kind":"snippet","value":"exit status 65"},
        "impact":"the ios shell does not build",
        "remediation":"fix the compile error"
    }],
    "outputs":[{"platform":"ios","path":"ios/"}],
    "ui-surface":{"screens":2},
    "next-continuation":null,
    "written":[{"root":"workspace","path":"ios/App.swift"}]}"#;
    let model = Harness::answering([dirty]);

    let report =
        Adapter::verify(&model, &ctx(tmp.path(), None), &workspace(tmp.path())).await.unwrap();

    assert_check_shape(&report);
    assert_eq!(report.source, PhaseSource::ModelAssisted);
    assert_eq!(report.findings.len(), 1);
    assert_eq!(
        report.findings[0].source,
        DiagnosticSource::ModelAssisted,
        "tool attribution is sanitized to model-assisted"
    );
    assert_eq!(
        report.outcome,
        PhaseOutcome::Completed,
        "not-applicable with findings flips to completed"
    );

    // A findings-free not-applicable answer stays not-applicable and
    // must shed its written entries (`target-phase-not-applicable-dirty`).
    let clean_na = r#"{"outcome":"not-applicable","source":"model-assisted",
        "written":[{"root":"workspace","path":"ios/App.swift"}]}"#;
    let model = Harness::answering([clean_na]);
    let report =
        Adapter::verify(&model, &ctx(tmp.path(), None), &workspace(tmp.path())).await.unwrap();
    assert_eq!(report.outcome, PhaseOutcome::NotApplicable);
    assert!(report.written.is_empty(), "a not-applicable report must be clean");
}

#[tokio::test]
async fn verify_single_pass() {
    let tmp = TempDir::new().unwrap();
    let stage = TempDir::new().unwrap();
    // Declared core platform with no `shared/` tree: the deterministic
    // in-guest shell verify gate contributes a blocking finding.
    fs::create_dir_all(tmp.path().join(".emery")).unwrap();
    fs::write(tmp.path().join(".emery/project.yaml"), "name: demo-app\nplatforms:\n  - core\n")
        .unwrap();
    let model = Harness::answering([PHASE_REPORT_CLEAN]);

    let report = Adapter::verify(
        &model,
        &ctx(tmp.path(), None),
        &staged_workspace(tmp.path(), stage.path()),
    )
    .await
    .unwrap();

    assert_eq!(report.outcome, PhaseOutcome::Completed);
    assert_eq!(
        report.source,
        PhaseSource::Hybrid,
        "the in-guest verify gate contributed alongside the model pass"
    );
    assert!(
        report.findings.iter().any(|finding| finding.title.contains("platform-shell-missing")),
        "deterministic gate findings ride the report: {:?}",
        report.findings
    );
    assert_check_shape(&report);

    let requests = model.requests();
    assert_eq!(requests.len(), 1, "verify is one pass — no retry loop");
    let (name, schema) = schema_format(&requests[0]);
    assert_eq!(name, "verify");
    assert_eq!(schema, PHASE_REPORT_ANSWER_SCHEMA);
    let system = requests[0].system.as_deref().unwrap();
    assert!(system.contains("# Vectis target — verify prompt"));
    assert!(system.contains("One pass only"), "single-pass contract in the prompt");
    let user = &requests[0].messages[0].content;
    assert!(!user.contains("demo"), "verify receives no slice identity: {user}");
    assert!(user.contains("no slice identity is supplied"), "workspace-self-contained prose");
    assert!(
        user.contains(&stage.path().display().to_string()),
        "the lent artifact stage is named for candidate slice-artifact reads"
    );
    assert_system_budget(&requests[0], "verify", 5_600); // baseline 5_064
}

#[tokio::test]
async fn verify_model_assisted_without_gates() {
    let tmp = TempDir::new().unwrap();
    // No `.emery/project.yaml` and no lent stage: neither in-guest gate
    // runs, so the report stays model-assisted.
    let model = Harness::answering([PHASE_REPORT_CLEAN]);

    let report =
        Adapter::verify(&model, &ctx(tmp.path(), None), &workspace(tmp.path())).await.unwrap();

    assert_eq!(report.source, PhaseSource::ModelAssisted);
    assert!(report.findings.is_empty());
    assert_check_shape(&report);
}

#[tokio::test]
async fn repair_single_pass() {
    let tmp = TempDir::new().unwrap();
    let stage = TempDir::new().unwrap();
    let model = Harness::answering([PHASE_REPORT_CLEAN]);
    let findings = vec![blocking_finding("cargo clippy failed: unused variable `x`")];

    let report = Adapter::repair(
        &model,
        &ctx(tmp.path(), None),
        "demo",
        RepairOrigin::Verification,
        &findings,
        Some(b"opaque-continuation"),
        &staged_workspace(tmp.path(), stage.path()),
    )
    .await
    .unwrap();

    assert_eq!(report.outcome, PhaseOutcome::Completed);
    assert_eq!(report.source, PhaseSource::ModelAssisted, "repair runs no in-guest gate");
    assert_check_shape(&report);

    let requests = model.requests();
    assert_eq!(requests.len(), 1, "repair is one findings-directed pass");
    let (name, _) = schema_format(&requests[0]);
    assert_eq!(name, "repair");
    let system = requests[0].system.as_deref().unwrap();
    assert!(system.contains("# Vectis target — repair prompt"));
    let user = &requests[0].messages[0].content;
    assert!(user.contains("verification"), "origin keys the brief");
    assert!(
        user.contains("cargo clippy failed: unused variable `x`"),
        "typed findings render into the brief via render_findings: {user}"
    );
    assert!(user.contains("slice `demo`"), "repair is slice-scoped");
    assert!(
        user.contains(&stage.path().display().to_string()),
        "artifact-stage fixes are routed to the stage"
    );
    assert_system_budget(&requests[0], "repair", 4_900); // baseline 4_427
}

// Brief entries arrive with `source: deterministic`; repair runs no
// in-guest gate, so check_pass normalizes them before the RFC-90 D2
// gate.
#[tokio::test]
async fn repair_brief_deterministic_findings_sanitized() {
    let tmp = TempDir::new().unwrap();
    let stage = TempDir::new().unwrap();
    let unrepaired = r#"{"outcome":"completed","source":"model-assisted","findings":[{
        "title":"[core-verify-stamp-missing] shared/.vectis/verify.ok not found",
        "severity":"important",
        "source":"deterministic",
        "kind":"violation",
        "artifact":"code",
        "location":{"path":"shared/.vectis/verify.ok"},
        "evidence":{"kind":"snippet","value":"Repair target forbids stamp writing."},
        "impact":"Core verify stamp absent until the engine runs the core verify gate.",
        "remediation":"Engine: run core verify and write shared/.vectis/verify.ok."
    }],
    "written":[{"root":"workspace","path":"shared/src/lib.rs"}]}"#;
    let model = Harness::answering([unrepaired]);
    let findings =
        vec![blocking_finding("[core-verify-stamp-missing] shared/.vectis/verify.ok not found")];

    let report = Adapter::repair(
        &model,
        &ctx(tmp.path(), None),
        "demo",
        RepairOrigin::Verification,
        &findings,
        None,
        &staged_workspace(tmp.path(), stage.path()),
    )
    .await
    .unwrap();

    assert_eq!(report.source, PhaseSource::ModelAssisted);
    assert_eq!(report.findings.len(), 1);
    assert_eq!(
        report.findings[0].source,
        DiagnosticSource::ModelAssisted,
        "brief-derived deterministic attribution is sanitized to model-assisted"
    );
    assert_check_shape(&report);
}

#[tokio::test]
async fn review_single_pass() {
    let tmp = TempDir::new().unwrap();
    let stage = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join(".emery")).unwrap();
    fs::write(
        tmp.path().join(".emery/project.yaml"),
        "name: demo-app\nplatforms:\n  - core\n  - ios\n",
    )
    .unwrap();
    let model = Harness::answering([PHASE_REPORT_CLEAN]);

    let report = Adapter::review(
        &model,
        &ctx(tmp.path(), None),
        "demo",
        None,
        &staged_workspace(tmp.path(), stage.path()),
    )
    .await
    .unwrap();

    assert_eq!(report.outcome, PhaseOutcome::Completed);
    assert_eq!(report.source, PhaseSource::ModelAssisted, "review is a pure model pass");
    assert_check_shape(&report);

    let requests = model.requests();
    assert_eq!(requests.len(), 1, "review is one pass — remediation routes through repair");
    let (name, _) = schema_format(&requests[0]);
    assert_eq!(name, "review");
    let system = requests[0].system.as_deref().unwrap();
    assert!(system.contains("# Vectis target — review prompt"));
    assert!(system.contains("# Vectis review — core"));
    assert!(system.contains("# Vectis review — iOS"), "declared ios shell review is assembled");
    assert!(
        !system.contains("# Vectis review — Android"),
        "android is not declared, so its review prompt stays out"
    );
    let user = &requests[0].messages[0].content;
    assert!(user.contains("Consolidate review findings"), "consolidation instructed");
    assert!(user.contains("never remediates"), "report-only contract");
    assert_system_budget(&requests[0], "review", 17_000); // baseline 15_433
}

async fn build_with_composition(
    composition: Option<&str>, report_answer: &'static str,
) -> PhaseReport {
    let tmp = TempDir::new().unwrap();
    let stage = TempDir::new().unwrap();
    if let Some(body) = composition {
        fs::write(stage.path().join("composition.yaml"), body).unwrap();
    }
    let model =
        Harness::answering([PHASE_DONE, PHASE_DONE, SHELL_SKIPPED, SHELL_SKIPPED, report_answer]);
    let report = Adapter::build(
        &model,
        &ctx(tmp.path(), None),
        "demo",
        &[],
        &BuildContext::default(),
        &staged_workspace(tmp.path(), stage.path()),
    )
    .await
    .unwrap();
    assert_eq!(model.requests().len(), 5, "coherence warnings never trigger extra legs");
    report
}

#[tokio::test]
async fn ui_surface_coherence() {
    const NON_UI: &str =
        r#"{"outcome":"completed","source":"model-assisted","ui-surface":{"screens":0}}"#;
    const UI: &str =
        r#"{"outcome":"completed","source":"model-assisted","ui-surface":{"screens":2}}"#;
    const SCREENS: &str = "version: 1\nscreens:\n  home:\n    name: Home\n";
    const EMPTY_SCREENS: &str = "version: 1\nscreens: {}\n";

    // screens == 0 against a non-empty `screens:` composition warns
    // unexpected-for-non-ui.
    let report = build_with_composition(Some(SCREENS), NON_UI).await;
    assert_eq!(report.outcome, PhaseOutcome::Completed);
    assert_eq!(report.findings.len(), 1, "expected one warning, got {:?}", report.findings);
    assert_eq!(
        report.findings[0].rule_id.as_deref(),
        Some("composition-unexpected-for-non-ui-slice")
    );
    assert_eq!(report.findings[0].severity, Severity::Suggestion);
    assert_eq!(report.findings[0].kind, FindingKind::Review);
    assert!(!report.findings[0].severity.blocking(), "A4 warnings must never block");

    // screens > 0 against an empty `screens: {}` composition warns
    // empty-for-ui-slice.
    let report = build_with_composition(Some(EMPTY_SCREENS), UI).await;
    assert_eq!(report.findings.len(), 1, "expected one warning, got {:?}", report.findings);
    assert_eq!(report.findings[0].rule_id.as_deref(), Some("composition-empty-for-ui-slice"));

    // An absent composition with a UI-surface claim also flags
    // empty-for-ui (an unreadable file is treated as empty).
    let report = build_with_composition(None, UI).await;
    assert_eq!(report.findings.len(), 1, "absent composition for a UI slice warns");
    assert_eq!(report.findings[0].rule_id.as_deref(), Some("composition-empty-for-ui-slice"));

    // Coherent pairs and a report without a `ui-surface` claim stay silent.
    let report = build_with_composition(Some(SCREENS), UI).await;
    assert!(report.findings.is_empty(), "ui slice + non-empty composition is coherent");
    let report = build_with_composition(Some(EMPTY_SCREENS), NON_UI).await;
    assert!(report.findings.is_empty(), "non-ui slice + empty composition is coherent");
    let report = build_with_composition(Some(SCREENS), PHASE_REPORT_CLEAN).await;
    assert!(report.findings.is_empty(), "absent ui-surface emits no warnings");

    // A non-empty `delta:` envelope counts as a UI surface; an all-empty
    // `delta:` does not.
    let added = "version: 1\ndelta:\n  added:\n    home:\n      name: Home\n  modified: {}\n  removed: {}\n";
    let report = build_with_composition(Some(added), NON_UI).await;
    assert_eq!(report.findings.len(), 1, "non-empty delta is a UI surface");
    assert_eq!(
        report.findings[0].rule_id.as_deref(),
        Some("composition-unexpected-for-non-ui-slice")
    );
    let empty_delta = "version: 1\ndelta:\n  added: {}\n  modified: {}\n  removed: {}\n";
    let report = build_with_composition(Some(empty_delta), NON_UI).await;
    assert!(report.findings.is_empty(), "an all-empty delta carries no UI surface");
}

#[tokio::test]
async fn merge_preflight_deterministic() {
    let tmp = TempDir::new().unwrap();
    let model = Harness::answering::<&str>([]);

    // A clean (absent) staged composition passes without a judgment leg.
    let report = Adapter::merge(
        &model,
        &ctx(tmp.path(), None),
        "demo",
        MergePhase::Preflight,
        &workspace(tmp.path()),
    )
    .await
    .unwrap();
    assert_eq!(report.status, Status::Success);
    assert!(model.requests().is_empty(), "preflight is deterministic: no leg");

    // A broken staged slice composition parks the merge before the fold.
    fs::create_dir_all(tmp.path().join(".emery/slices/demo")).unwrap();
    fs::write(tmp.path().join(".emery/slices/demo/composition.yaml"), "screens: [broken\n")
        .unwrap();
    let report = Adapter::merge(
        &model,
        &ctx(tmp.path(), None),
        "demo",
        MergePhase::Preflight,
        &workspace(tmp.path()),
    )
    .await
    .unwrap();
    assert_eq!(report.status, Status::Failure);
    assert!(report.findings[0].detail.contains("[composition]"));
    assert!(model.requests().is_empty(), "a staged failure still spends no judgment leg");
}

#[tokio::test]
async fn merge_postflight_single_leg() {
    let tmp = TempDir::new().unwrap();
    let model = Harness::answering([SUCCESS_REPORT]);

    let report = Adapter::merge(
        &model,
        &ctx(tmp.path(), None),
        "demo",
        MergePhase::Postflight,
        &workspace(tmp.path()),
    )
    .await
    .unwrap();

    assert_eq!(report.status, Status::Success);
    let requests = model.requests();
    assert_eq!(requests.len(), 1, "a coherent report needs no repair leg");
    assert!(requests[0].system.as_deref().unwrap().contains("# Vectis target — `merge`"));
    assert_system_budget(&requests[0], "merge-postflight", 8_100); // baseline 7_402
    let user = &requests[0].messages[0].content;
    assert!(user.contains("postflight merge gate"), "phase named");
    assert!(user.contains("cap-matrix re-verification"), "agent-run host gates instructed");
}

#[tokio::test]
async fn merge_postflight_gates_composition() {
    let tmp = TempDir::new().unwrap();
    // A broken merged baseline composition is caught by the postlude's
    // in-guest validator; residual findings force failure.
    fs::create_dir_all(tmp.path().join(".emery/specs")).unwrap();
    fs::write(tmp.path().join(".emery/specs/composition.yaml"), "screens: [broken\n").unwrap();
    let model = Harness::answering([SUCCESS_REPORT, SUCCESS_REPORT]);

    let report = Adapter::merge(
        &model,
        &ctx(tmp.path(), None),
        "demo",
        MergePhase::Postflight,
        &workspace(tmp.path()),
    )
    .await
    .unwrap();

    assert_eq!(report.status, Status::Failure);
    assert!(report.findings[0].detail.contains("[composition]"));
    assert_eq!(model.requests().len(), 2, "one merge leg plus one bounded repair leg");
}
