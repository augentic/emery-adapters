//! Omnia build-loop and merge operation behavior (RFC-90 split).

mod common;

use std::path::Path;

use adapter::answers::REPORT_ANSWER_SCHEMA;
use adapter::seam::{
    ArtifactStage, BuildContext, Context, DiagnosticSource, FindingArtifact, FindingConfidence,
    FindingEvidence, FindingKind, Input, MergePhase, Payload, PhaseFinding, PhaseLocation,
    PhaseOutcome, PhaseSource, Platform, RepairOrigin, Severity, Status, Workspace,
    WritableArtifactKind,
};
use adapter::{Format, Request, Target as _};
use omnia::Adapter;
use omnia_testkit::model::{Harness, mcp_grants};
use tempfile::TempDir;

const PHASE_DONE: &str = r#"{"applicable":true,"summary":"phase complete"}"#;
const REPORT_DONE: &str = r#"{"outcome":"completed","source":"model-assisted","outputs":[{"platform":"core","path":"crates/demo"}],"written":[{"root":"artifacts","path":"tasks.md"},{"root":"workspace","path":"crates/demo"}]}"#;
const CLEAN_PHASE: &str = r#"{"outcome":"completed","source":"model-assisted"}"#;
const SUCCESS_REPORT: &str = r#"{"status":"success","findings":[]}"#;

fn ctx<'a>(root: &'a Path, mcp_url: Option<&str>) -> Context<'a> {
    Context {
        adapter_id: "target:omnia",
        project_root: root,
        mcp_url: mcp_url.map(str::to_owned),
        lend: root.display().to_string(),
        source_key: None,
    }
}

// The degenerate single-checkout shape: workspace root and artifact
// root both point at the test tree, like the engine's mock sessions;
// the build-loop stage is a sibling directory.
fn workspace(root: &Path) -> Workspace {
    Workspace {
        id: "ws-1".to_string(),
        root: root.display().to_string(),
        artifacts: root.display().to_string(),
        artifact_stage: Some(ArtifactStage {
            id: "stage-1".to_string(),
            root: root.join("stage").display().to_string(),
        }),
    }
}

fn input(path: &str) -> Payload {
    Payload::Path(path.to_string())
}

fn finding(title: &str) -> PhaseFinding {
    PhaseFinding {
        id: "COR-1".to_string(),
        rule_id: Some("OMNIA-002".to_string()),
        related_rule_ids: Vec::new(),
        title: title.to_string(),
        severity: Severity::Important,
        source: DiagnosticSource::ModelAssisted,
        kind: FindingKind::Violation,
        artifact: FindingArtifact::Code,
        location: Some(PhaseLocation {
            path: "crates/demo/src/lib.rs".to_string(),
            line: Some(42),
            ..PhaseLocation::default()
        }),
        evidence: FindingEvidence::Snippet {
            value: "std::fs::read".to_string(),
        },
        impact: "The wasm32 build breaks.".to_string(),
        remediation: "Route through the provider trait.".to_string(),
        confidence: Some(FindingConfidence::High),
        fingerprint: String::new(),
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

/// The generation leg's create-mode assemble and path-form user prompt:
/// guidance dropped, guest writer present, inputs as project-relative
/// path sections with a read-before-writing instruction — and no
/// verify-repair instruction: checks moved to the `verify` operation.
fn assert_generation_leg(generation: &Request) {
    let system = generation.system.as_deref().unwrap();
    assert!(system.contains("# Omnia target — build prompt"), "build prompt in system");
    assert!(
        !system.contains("# Omnia target — guidance prompt"),
        "guidance stays on the guidance operation — never assembled into generation"
    );
    assert!(system.contains("# Omnia build — crate writer"), "crate writer prompt in system");
    assert!(system.contains("# Omnia build — test writer"), "test writer prompt in system");
    assert!(
        !system.contains("| Failure signal |"),
        "the test-failure classification table lives in repair-patterns.md, \
         not the shared preamble"
    );
    assert!(
        system.contains("# Omnia build — guest writer"),
        "guest writer prompt in system — create mode (no workspace-root src/lib.rs)"
    );
    assert!(
        !system.contains("Verify-repair loop"),
        "no verify-repair loop in generation prose — engine policy now (RFC-90)"
    );
    let user = &generation.messages[0].content;
    assert!(
        user.contains("/.emery/slices/demo/proposal.md")
            && user.contains("/.emery/slices/demo/design.md")
            && user.contains("/.emery/slices/demo/specs/core/spec.md"),
        "typed inputs render as artifact-rooted path sections: {user}"
    );
    assert!(!user.contains("PROPOSAL-BODY"), "artifact bodies are not inlined");
    assert!(
        user.contains("Read each path"),
        "read-before-writing instruction rides the inputs block"
    );
    assert!(
        user.contains("folded into the slice artifacts at refine"),
        "guidance pointer replaces the inlined guidance prompt"
    );
    assert!(user.contains("slice `demo`"), "slice named");
    assert!(
        user.contains("do not run the check suite"),
        "generation told checks are engine-dispatched operations: {user}"
    );
    assert!(user.contains("omnia-references"), "user prompt points at the MCP references");
    assert!(user.contains("### scaffold prelude"), "scaffold prelude outcome in user prompt");
    assert!(user.contains("- `Makefile.toml`"), "written tooling files listed");
    assert!(
        user.contains("Unfilled placeholders still present"),
        "unfilled publish tokens always surfaced: {user}"
    );
}

/// The close-out leg's assemble and user prompt: the phase-report
/// schema, the close-out prose (review prose absent), the phase
/// outcomes block, and staged tasks.md routing.
fn assert_closeout_leg(closeout: &Request, stage_root: &str) {
    let (name, schema) = schema_format(closeout);
    assert_eq!(name, "report");
    let compiled = serde_json::from_str::<serde_json::Value>(schema).unwrap();
    assert!(jsonschema::validator_for(&compiled).is_ok(), "phase-report answer schema compiles");
    for field in ["findings", "outputs", "written"] {
        assert!(schema.contains(&format!("\"{field}\"")), "phase-report field: {field}");
    }
    let system = closeout.system.as_deref().unwrap();
    assert!(system.contains("# Omnia build — close-out"), "close-out prompt in system");
    assert!(
        !system.contains("standards review (code reviewer)"),
        "the review prose stays on the review operation"
    );
    let user = &closeout.messages[0].content;
    assert!(user.contains("- preparation:"), "preparation outcome feeds the close-out");
    assert!(user.contains("- generation:"), "generation outcome feeds the close-out");
    assert!(
        user.contains("- replay: skipped in-guest"),
        "the deterministic replay skip is surfaced to the close-out leg: {user}"
    );
    assert!(
        user.contains(&format!("`{stage_root}/tasks.md`")),
        "tasks.md checkbox writes are routed onto the lent artifact stage: {user}"
    );
    assert!(
        user.contains("never in the authoritative slice tree"),
        "authoritative-tree writes forbidden: {user}"
    );
}

/// The tasks.md file grant and the bumped host floor ride the metadata
/// record (RFC-90 D5).
#[test]
fn metadata_declares_writable_artifacts() {
    let metadata = Adapter::metadata();
    assert_eq!(metadata.emery_floor.as_deref(), Some("0.38.0"));
    assert_eq!(metadata.writable_artifacts.len(), 1, "tasks.md is the only staged write");
    let grant = &metadata.writable_artifacts[0];
    assert_eq!(grant.path, "tasks.md");
    assert_eq!(grant.kind, WritableArtifactKind::File);
}

#[tokio::test]
async fn build_phase_legs() {
    let tmp = TempDir::new().unwrap();
    // The scripted preparation leg writes nothing, so the checkout the
    // real agent would produce is synthesized up front — likewise the
    // crate tree the close-out answer declares as an output.
    common::write_checkout(tmp.path());
    std::fs::create_dir_all(tmp.path().join("crates/demo")).unwrap();
    let model = Harness::answering([PHASE_DONE, PHASE_DONE, REPORT_DONE]);
    let inputs = vec![
        Input::Proposal(input(".emery/slices/demo/proposal.md")),
        Input::Spec(input(".emery/slices/demo/specs/core/spec.md")),
        Input::Design(input(".emery/slices/demo/design.md")),
    ];
    let context = BuildContext {
        sources: vec!["intent".to_string(), "typescript".to_string()],
    };
    let workspace = workspace(tmp.path());

    let report = Adapter::build(
        &model,
        &ctx(tmp.path(), Some("http://references/mcp")),
        "demo",
        &inputs,
        &context,
        &workspace,
    )
    .await
    .unwrap();

    assert_eq!(report.outcome, PhaseOutcome::Completed);
    assert_eq!(report.source, PhaseSource::ModelAssisted);
    assert!(report.findings.is_empty());
    assert_eq!(report.outputs.len(), 1, "close-out-declared outputs ride the phase report");
    assert_eq!(report.outputs[0].platform, Platform::Core);
    assert_eq!(report.outputs[0].path, "crates/demo");
    assert!(report.ui_surface.is_none());
    assert_eq!(report.written.len(), 2, "audit writes ride the answer");
    assert!(report.next_continuation.is_none(), "omnia carries no writer-session state");

    let requests = model.requests();
    assert_eq!(
        requests.len(),
        3,
        "preparation, generation, close-out — no replay spawn without a captures \
         binding, no verify/review leg inside build"
    );
    // Budget = measured baseline (per-leg comment, 2026-08-10) + ~10%.
    for (i, (leg, budget)) in [
        ("preparation", 10_900), // baseline 9_880
        ("generation", 28_900),  // baseline 26_184
        ("report", 11_700),      // baseline 10_553
    ]
    .into_iter()
    .enumerate()
    {
        assert_system_budget(&requests[i], leg, budget);
    }

    let preparation = &requests[0];
    assert_eq!(schema_format(preparation).0, "preparation");
    let system = preparation.system.as_deref().unwrap();
    assert!(system.contains("# Omnia target — build prompt"), "build prompt in system");
    assert!(system.contains("# Omnia build — preparation"), "preparation prompt in system");
    let user = &preparation.messages[0].content;
    assert!(user.contains("target/omnia-exemplar"), "checkout location named");
    assert!(user.contains("Stop hint contract"), "stop path instructed");

    let generation = &requests[1];
    assert_generation_leg(generation);
    // Generation only starts after the deterministic scaffold has run.
    for path in
        ["Makefile.toml", "deny.toml", "supply-chain/config.toml", ".github/workflows/ci.yaml"]
    {
        assert!(tmp.path().join(path).is_file(), "prelude wrote {path}");
    }
    let (name, schema) = schema_format(generation);
    assert_eq!(name, "generation");
    let compiled = serde_json::from_str::<serde_json::Value>(schema).unwrap();
    assert!(jsonschema::validator_for(&compiled).is_ok(), "internal schema compiles");
    assert_eq!(
        generation.workspace.as_deref(),
        Some(tmp.path().to_str().unwrap()),
        "the build leg lends the prepared workspace path"
    );
    assert_eq!(mcp_grants(generation)[0].url, "http://references/mcp");

    assert_closeout_leg(&requests[2], &workspace.artifact_stage.as_ref().unwrap().root);
}

// The replay leg spawns only when the engine-forwarded build context
// carries a `captures` source binding; it runs before the close-out
// leg so replay failures reach the findings synthesis.
#[tokio::test]
async fn build_replay_leg_gated_on_captures_binding() {
    let tmp = TempDir::new().unwrap();
    common::write_checkout(tmp.path());
    let model = Harness::answering([
        PHASE_DONE,
        PHASE_DONE,
        r#"{"applicable":true,"summary":"replay suite passed"}"#,
        CLEAN_PHASE,
    ]);
    let context = BuildContext {
        sources: vec!["intent".to_string(), "captures".to_string()],
    };

    let report = Adapter::build(
        &model,
        &ctx(tmp.path(), None),
        "demo",
        &[],
        &context,
        &workspace(tmp.path()),
    )
    .await
    .unwrap();

    assert_eq!(report.outcome, PhaseOutcome::Completed);
    let requests = model.requests();
    assert_eq!(requests.len(), 4, "preparation, generation, replay, close-out");
    let replay = &requests[2];
    assert_eq!(schema_format(replay).0, "replay");
    assert!(replay.system.as_deref().unwrap().contains("# Omnia build — capture replay"));
    assert!(
        replay.messages[0].content.contains("binds the `captures` source"),
        "replay dispatch is deterministic — the leg is never asked to self-skip"
    );
    assert_system_budget(replay, "replay", 11_200); // baseline 10_176
    let closeout_user = &requests[3].messages[0].content;
    assert!(
        closeout_user.contains("- replay: applicable=true"),
        "replay outcome feeds the close-out leg: {closeout_user}"
    );
}

// A build dispatched without a lent stage (the Option is part of the
// shared workspace record) skips the checkbox close-out instead of
// writing the authoritative slice tree.
#[tokio::test]
async fn build_without_stage_skips_checkbox_closeout() {
    let tmp = TempDir::new().unwrap();
    common::write_checkout(tmp.path());
    let model = Harness::answering([PHASE_DONE, PHASE_DONE, CLEAN_PHASE]);
    let mut workspace = workspace(tmp.path());
    workspace.artifact_stage = None;

    Adapter::build(
        &model,
        &ctx(tmp.path(), None),
        "demo",
        &[],
        &BuildContext::default(),
        &workspace,
    )
    .await
    .unwrap();

    let closeout_user = &model.requests()[2].messages[0].content;
    assert!(
        closeout_user.contains("skip the tasks.md checkbox close-out"),
        "no stage means no tasks.md write: {closeout_user}"
    );
}

// Update mode (workspace-root `src/lib.rs` present) drops the guest
// writer from the generation assemble; guest wiring updates fold into
// the crate writer per the build prompt's mode detection.
#[tokio::test]
async fn build_update_mode_skips_guest_writer() {
    let tmp = TempDir::new().unwrap();
    common::write_checkout(tmp.path());
    std::fs::create_dir_all(tmp.path().join("src")).unwrap();
    std::fs::write(tmp.path().join("src/lib.rs"), "// existing guest\n").unwrap();
    let model = Harness::answering([PHASE_DONE, PHASE_DONE, CLEAN_PHASE]);

    Adapter::build(
        &model,
        &ctx(tmp.path(), None),
        "demo",
        &[],
        &BuildContext::default(),
        &workspace(tmp.path()),
    )
    .await
    .unwrap();

    let generation = &model.requests()[1];
    let system = generation.system.as_deref().unwrap();
    assert!(system.contains("# Omnia build — crate writer"), "crate writer prompt in system");
    assert!(
        !system.contains("# Omnia build — guest writer"),
        "guest writer prompt absent in update mode"
    );
}

#[tokio::test]
async fn build_fails_closed_without_checkout() {
    let tmp = TempDir::new().unwrap();
    let model = Harness::answering([PHASE_DONE]);

    let error = Adapter::build(
        &model,
        &ctx(tmp.path(), None),
        "demo",
        &[],
        &BuildContext::default(),
        &workspace(tmp.path()),
    )
    .await
    .unwrap_err();

    assert!(error.to_string().contains("preparation leg"), "names the missing step: {error}");
    assert_eq!(model.requests().len(), 1, "aborted after the preparation leg, before generation");
}

// One check pass, one leg, no slice identity: the verify prompt owns
// the command list; the answer is the phase report.
#[tokio::test]
async fn verify_single_pass() {
    let tmp = TempDir::new().unwrap();
    let model = Harness::answering([CLEAN_PHASE]);

    let report =
        Adapter::verify(&model, &ctx(tmp.path(), None), &workspace(tmp.path())).await.unwrap();

    assert_eq!(report.outcome, PhaseOutcome::Completed);
    assert_eq!(report.source, PhaseSource::ModelAssisted);
    assert!(report.findings.is_empty());
    assert!(report.outputs.is_empty(), "only build declares outputs");
    assert!(report.ui_surface.is_none());
    assert!(report.next_continuation.is_none(), "verify never mutates the continuation");

    let requests = model.requests();
    assert_eq!(requests.len(), 1, "one pass, no retry leg");
    assert_eq!(schema_format(&requests[0]).0, "verify");
    let system = requests[0].system.as_deref().unwrap();
    assert!(system.contains("# Omnia target — verify prompt"));
    assert!(system.contains("cargo clippy --all-targets -- -D warnings"), "check pass listed");
    assert!(system.contains("One pass only"), "no in-prompt retry loop");
    assert_system_budget(&requests[0], "verify", 3_100); // baseline 2_765
    let user = &requests[0].messages[0].content;
    assert!(user.contains("fix nothing"), "verify observes, never repairs: {user}");
    assert!(!user.contains("slice"), "verify receives no slice identity: {user}");
}

// A verification failure comes back as typed findings with locations —
// the engine, not the adapter, decides whether repair runs.
#[tokio::test]
async fn verify_findings() {
    let tmp = TempDir::new().unwrap();
    let model = Harness::answering([
        r#"{"outcome":"completed","source":"model-assisted","findings":[{"title":"cargo clippy: needless clone","severity":"important","source":"model-assisted","artifact":"code","location":{"path":"crates/demo/src/handler.rs","line":12},"evidence":{"kind":"snippet","value":"error: redundant clone"},"impact":"Fails -D warnings.","remediation":"Remove the clone."}]}"#,
    ]);

    let report =
        Adapter::verify(&model, &ctx(tmp.path(), None), &workspace(tmp.path())).await.unwrap();

    assert_eq!(report.findings.len(), 1);
    let finding = &report.findings[0];
    assert!(finding.blocking(), "an important violation blocks");
    assert_eq!(
        finding.location.as_ref().unwrap().path,
        "crates/demo/src/handler.rs",
        "cargo locations ride the finding"
    );
    assert_eq!(model.requests().len(), 1, "a failing pass still returns after one leg");
}

// Dirty scripted answer through a check pass: the in-code postlude
// clears outputs / UI surface / continuation, forces `model-assisted`
// attribution (report and findings alike), flips a findings-bearing
// not-applicable outcome to completed, and clears `written` on a
// findings-free not-applicable answer.
#[tokio::test]
async fn verify_dirty_answer_sanitized() {
    let tmp = TempDir::new().unwrap();
    let dirty = r#"{"outcome":"not-applicable","source":"tool","findings":[{
        "title":"cargo test failed",
        "severity":"important",
        "source":"tool",
        "kind":"violation",
        "artifact":"code",
        "evidence":{"kind":"snippet","value":"test result: FAILED"},
        "impact":"the check suite fails",
        "remediation":"fix the failing test"
    }],
    "outputs":[{"platform":"core","path":"crates/demo"}],
    "ui-surface":{"screens":1},
    "written":[{"root":"workspace","path":"crates/demo/src/lib.rs"}]}"#;
    let model = Harness::answering([dirty]);

    let report =
        Adapter::verify(&model, &ctx(tmp.path(), None), &workspace(tmp.path())).await.unwrap();

    assert!(report.outputs.is_empty(), "check passes declare no outputs");
    assert!(report.ui_surface.is_none(), "check passes carry no UI surface");
    assert!(report.next_continuation.is_none(), "verify never mutates the continuation");
    assert_eq!(report.source, PhaseSource::ModelAssisted, "report source forced coherent");
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

    // Findings-free not-applicable: the outcome stands and the written
    // entries are cleared (`target-phase-not-applicable-dirty`).
    let clean_na = r#"{"outcome":"not-applicable","source":"model-assisted",
        "written":[{"root":"workspace","path":"crates/demo/src/lib.rs"}]}"#;
    let model = Harness::answering([clean_na]);
    let report =
        Adapter::verify(&model, &ctx(tmp.path(), None), &workspace(tmp.path())).await.unwrap();
    assert_eq!(report.outcome, PhaseOutcome::NotApplicable);
    assert!(report.written.is_empty(), "a not-applicable report must be clean");
}

// Repair receives the engine's bounded brief; the rendered findings and
// the origin ride the user prompt, and the report declares no outputs.
#[tokio::test]
async fn repair_verification_origin() {
    let tmp = TempDir::new().unwrap();
    let model = Harness::answering([CLEAN_PHASE]);
    let findings = vec![finding("Forbidden std API")];

    let report = Adapter::repair(
        &model,
        &ctx(tmp.path(), None),
        "demo",
        RepairOrigin::Verification,
        &findings,
        Some(b"opaque-session"),
        &workspace(tmp.path()),
    )
    .await
    .unwrap();

    assert_eq!(report.outcome, PhaseOutcome::Completed);
    assert!(report.outputs.is_empty() && report.ui_surface.is_none());
    assert!(report.next_continuation.is_none(), "omnia preserves the continuation untouched");

    let requests = model.requests();
    assert_eq!(requests.len(), 1, "one findings-directed pass");
    assert_eq!(schema_format(&requests[0]).0, "repair");
    let system = requests[0].system.as_deref().unwrap();
    assert!(system.contains("# Omnia target — repair prompt"));
    assert!(system.contains("repair-origin: verification"), "verification branch in prose");
    assert!(system.contains("repair-origin: review"), "review branch in prose");
    assert_system_budget(&requests[0], "repair", 3_700); // baseline 3_358
    let user = &requests[0].messages[0].content;
    assert!(user.contains("`repair-origin: verification`"), "origin named: {user}");
    assert!(user.contains("1. [Important] OMNIA-002 — Forbidden std API"), "brief rendered");
    assert!(user.contains("at: crates/demo/src/lib.rs:42"), "finding location rendered");
    assert!(
        user.contains("remediation: Route through the provider trait."),
        "remediation rendered"
    );
}

#[tokio::test]
async fn repair_review_origin() {
    let tmp = TempDir::new().unwrap();
    let model = Harness::answering([CLEAN_PHASE]);
    let findings = vec![finding("Missing input validation")];

    Adapter::repair(
        &model,
        &ctx(tmp.path(), None),
        "demo",
        RepairOrigin::Review,
        &findings,
        None,
        &workspace(tmp.path()),
    )
    .await
    .unwrap();

    let user = &model.requests()[0].messages[0].content;
    assert!(user.contains("`repair-origin: review`"), "review origin named: {user}");
    assert!(user.contains("Missing input validation"), "brief rendered");
}

// One standards pass: the review team prose rides the system prompt,
// the answer's findings ride the phase report, and no remediation or
// auto-fix leg follows — the engine routes blocking findings to repair.
#[tokio::test]
async fn review_single_pass() {
    let tmp = TempDir::new().unwrap();
    let model = Harness::answering([
        r#"{"outcome":"completed","source":"model-assisted","findings":[{"id":"SEC-1","rule-id":"OMNIA-002","title":"Forbidden std API","severity":"critical","source":"model-assisted","artifact":"code","evidence":{"kind":"snippet","value":"std::env::var"},"impact":"The wasm32 build breaks.","remediation":"Route through the provider trait."}],"written":[{"root":"workspace","path":"crates/demo/REVIEW.md"}]}"#,
    ]);

    let report = Adapter::review(
        &model,
        &ctx(tmp.path(), None),
        "demo",
        Some(b"opaque-session"),
        &workspace(tmp.path()),
    )
    .await
    .unwrap();

    assert_eq!(report.outcome, PhaseOutcome::Completed);
    assert_eq!(report.source, PhaseSource::ModelAssisted);
    let finding = &report.findings[0];
    assert_eq!(finding.rule_id.as_deref(), Some("OMNIA-002"));
    assert_eq!(finding.severity, Severity::Critical);
    assert!(finding.blocking());
    assert!(report.outputs.is_empty(), "only build declares outputs");
    assert!(report.next_continuation.is_none(), "omnia carries no reviewer-session state");

    let requests = model.requests();
    assert_eq!(requests.len(), 1, "one pass — remediation is the engine's repair routing");
    assert_eq!(schema_format(&requests[0]).0, "review");
    let system = requests[0].system.as_deref().unwrap();
    assert!(system.contains("# Omnia target — review prompt"));
    assert!(system.contains("REVIEW.md"), "synthesis output named");
    assert!(!system.contains("Remediation cycle"), "no in-prompt remediation cycle");
    assert_system_budget(&requests[0], "review", 6_700); // baseline 6_025
    let user = &requests[0].messages[0].content;
    assert!(user.contains("slice `demo`"), "slice named");
    assert!(user.contains("no remediation cycle and no auto-fix"), "one pass instructed: {user}");
}

#[tokio::test]
async fn merge_preflight_single_leg() {
    let tmp = TempDir::new().unwrap();
    let model = Harness::answering([SUCCESS_REPORT]);

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
    let requests = model.requests();
    assert_eq!(requests.len(), 1, "a coherent report needs no repair leg");
    let (name, schema) = schema_format(&requests[0]);
    assert_eq!(name, "report");
    assert_eq!(schema, REPORT_ANSWER_SCHEMA, "merge still answers with the vendored report schema");
    assert!(requests[0].system.as_deref().unwrap().contains("# Omnia target — merge prompt"));
    assert_system_budget(&requests[0], "merge-preflight", 4_800); // baseline 4_331
    let user = &requests[0].messages[0].content;
    assert!(user.contains("preflight merge gate"), "phase named");
    assert!(user.contains("pre-merge gate"), "agent-run cargo verification instructed");
}

#[tokio::test]
async fn merge_postflight_deterministic() {
    let tmp = TempDir::new().unwrap();
    let model = Harness::answering::<&str>([]);

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
    assert!(report.findings.is_empty());
    assert!(model.requests().is_empty(), "omnia declares no postflight validator: no leg");
}

#[tokio::test]
async fn merge_diagnostics() {
    let tmp = TempDir::new().unwrap();
    let model = Harness::answering([
        r#"{"status":"failure","findings":[{"rule-id":"OMNIA-002","title":"Forbidden std API","severity":"critical","impact":"The wasm32 build breaks.","remediation":"Route through the provider trait."}]}"#,
    ]);

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
    let finding = &report.findings[0];
    assert_eq!(finding.rule_id.as_deref(), Some("OMNIA-002"));
    assert_eq!(finding.severity, Severity::Critical);
    assert_eq!(
        finding.detail,
        "Forbidden std API — The wasm32 build breaks.; remediation: Route through the provider trait."
    );
}
