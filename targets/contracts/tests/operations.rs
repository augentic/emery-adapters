//! Contracts build-loop / merge operation behavior.

use std::fs;
use std::path::Path;

use adapter::answers::PHASE_REPORT_ANSWER;
use adapter::seam::{
    ArtifactStage, BuildContext, Context, DiagnosticSource, FindingArtifact, FindingEvidence,
    FindingKind, Input, MergePhase, Payload, PhaseFinding, PhaseLocation, PhaseOutcome, PhaseRoot,
    PhaseSource, PhaseWrite, RepairOrigin, Severity, Status, Workspace, WritableArtifact,
};
use adapter::{Format, Request, Target as _};
use contracts::Adapter;
use contracts::validate::RULE_VERSION_IS_SEMVER;
use omnia_testkit::model::{Harness, mcp_grants};
use tempfile::TempDir;

const NOT_APPLICABLE: &str =
    r#"{"outcome":"not-applicable","source":"model-assisted","findings":[],"written":[]}"#;
const SUCCESS_REPORT: &str = r#"{"status":"success","findings":[]}"#;
const CLEAN_PHASE_REPORT: &str = r#"{"outcome":"completed","source":"model-assisted"}"#;
const CLOSEOUT: &str = r#"{"applicable":true,"summary":"ticked tasks","written":["tasks.md"]}"#;

fn ctx<'a>(root: &'a Path, mcp_url: Option<&str>) -> Context<'a> {
    Context {
        adapter_id: "target:contracts",
        project_root: root,
        mcp_url: mcp_url.map(str::to_owned),
        lend: Some(root.display().to_string()),
    }
}

// The degenerate single-checkout shape: workspace root and artifact
// root both point at the test tree; the stage is a sibling directory.
fn workspace(root: &Path, stage: &Path) -> Workspace {
    Workspace {
        id: "ws-1".to_string(),
        root: root.display().to_string(),
        artifacts: root.display().to_string(),
        artifact_stage: Some(ArtifactStage {
            id: "stage-1".to_string(),
            root: stage.display().to_string(),
        }),
    }
}

// Merge's workspace view is read-only and carries no stage.
fn merge_workspace(root: &Path) -> Workspace {
    Workspace {
        id: "ws-1".to_string(),
        root: root.display().to_string(),
        artifacts: root.display().to_string(),
        artifact_stage: None,
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

/// Seed one top-level contract whose `info.version` is not SemVer.
fn seed_bad_contract(dir: &Path) {
    fs::create_dir_all(dir.join("http")).unwrap();
    fs::write(
        dir.join("http/api.yaml"),
        "openapi: '3.1.0'\ninfo:\n  title: API\n  version: 2024-01-15\n",
    )
    .unwrap();
}

/// Seed one well-formed top-level contract.
fn seed_clean_contract(dir: &Path) {
    fs::create_dir_all(dir.join("http")).unwrap();
    fs::write(
        dir.join("http/api.yaml"),
        "openapi: '3.1.0'\ninfo:\n  title: API\n  version: 1.2.3\n",
    )
    .unwrap();
}

/// A blocking verification-shaped finding located under the given
/// stage-relative path.
fn located_finding(path: &str) -> PhaseFinding {
    PhaseFinding {
        id: "F-0001".to_string(),
        rule_id: Some(RULE_VERSION_IS_SEMVER.to_string()),
        related_rule_ids: Vec::new(),
        title: "info.version is not SemVer".to_string(),
        severity: Severity::Important,
        source: DiagnosticSource::Deterministic,
        kind: FindingKind::Violation,
        artifact: FindingArtifact::Contracts,
        location: Some(PhaseLocation {
            path: path.to_string(),
            ..PhaseLocation::default()
        }),
        evidence: FindingEvidence::Snippet {
            value: "version: 2024-01-15".to_string(),
        },
        impact: "the merge gate rejects non-SemVer contract versions".to_string(),
        remediation: "set a SemVer info.version".to_string(),
        confidence: None,
        fingerprint: String::new(),
    }
}

#[test]
fn metadata_grants() {
    let metadata = Adapter::metadata();
    assert_eq!(metadata.emery_floor.as_deref(), Some("0.38.0"));
    assert_eq!(
        metadata.writable_artifacts,
        vec![WritableArtifact::file("tasks.md"), WritableArtifact::tree("contracts")],
        "tasks.md file grant plus the contracts/ tree grant"
    );
}

#[tokio::test]
async fn build_sub_flows() {
    let tmp = TempDir::new().unwrap();
    let stage = TempDir::new().unwrap();
    let model = Harness::answering([
        NOT_APPLICABLE,
        r#"{"outcome":"completed","source":"model-assisted","findings":[],"written":[{"root":"artifacts","path":"contracts/http/user-api.yaml"}]}"#,
        NOT_APPLICABLE,
        CLOSEOUT,
    ]);
    let input = |path: &str| Payload::Path(path.to_string());
    let inputs = vec![
        Input::Proposal(input(".emery/slices/demo/proposal.md")),
        Input::Design(input(".emery/slices/demo/design.md")),
    ];

    let report = Adapter::build(
        &model,
        &ctx(tmp.path(), Some("http://references/mcp")),
        "demo",
        &inputs,
        &BuildContext::default(),
        &workspace(tmp.path(), stage.path()),
    )
    .await
    .unwrap();

    // Generation only: no verification, no repair, no report leg.
    assert_eq!(report.outcome, PhaseOutcome::Completed);
    assert_eq!(report.source, PhaseSource::ModelAssisted);
    assert!(report.findings.is_empty());
    assert!(report.outputs.is_empty(), "contract artifacts declare no per-platform outputs");
    assert!(report.ui_surface.is_none());
    assert!(report.next_continuation.is_none());
    assert_eq!(
        report.written,
        vec![
            PhaseWrite {
                root: PhaseRoot::Artifacts,
                path: "contracts/http/user-api.yaml".to_string(),
            },
            PhaseWrite {
                root: PhaseRoot::Artifacts,
                path: "tasks.md".to_string(),
            },
        ],
        "sub-flow and close-out writes are audited as artifact-stage writes"
    );

    let requests = model.requests();
    assert_eq!(requests.len(), 4, "three sub-flows plus the close-out leg");
    // Budget = measured baseline (per-leg comment, 2026-08-11) + ~10%.
    for (i, (leg, budget)) in [
        ("json-schema-sub-flow", 16_700), // baseline 15_210
        ("openapi-sub-flow", 16_700),     // baseline 15_204
        ("asyncapi-sub-flow", 16_300),    // baseline 14_795
        ("close-out", 9_500),             // baseline 8_626
    ]
    .into_iter()
    .enumerate()
    {
        assert_system_budget(&requests[i], leg, budget);
    }

    let closeout = &requests[3];
    let closeout_user = &closeout.messages[0].content;
    let staged_tasks = format!("{}/tasks.md", stage.path().display());
    assert!(
        closeout_user.contains(&staged_tasks),
        "close-out names the staged tasks.md: {closeout_user}"
    );

    let first = &requests[0];
    let system = first.system.as_deref().unwrap();
    assert!(system.contains("# contracts.build"), "build prompt in system");
    assert!(system.contains("json-schema sub-flow"), "sub-prompt in system");
    let user = &first.messages[0].content;
    assert!(
        user.contains("/.emery/slices/demo/proposal.md")
            && user.contains("/.emery/slices/demo/design.md"),
        "typed inputs render as artifact-rooted path sections: {user}"
    );
    assert!(!user.contains("PROPOSAL-BODY"), "artifact bodies are not inlined");
    assert!(
        user.contains("Read each path"),
        "read-before-writing instruction rides the inputs block"
    );
    let staged = format!("{}/contracts", stage.path().display());
    assert!(user.contains(&staged), "the staged contract delta path is named: {user}");
    let (name, schema) = schema_format(first);
    assert_eq!(name, "json-schema-sub-flow");
    let compiled = serde_json::from_str::<serde_json::Value>(schema).unwrap();
    assert!(jsonschema::validator_for(&compiled).is_ok(), "internal schema compiles");
    assert_eq!(
        first.workspace.as_deref(),
        Some(tmp.path().to_str().unwrap()),
        "the build leg lends the prepared workspace path"
    );
    assert_eq!(mcp_grants(first)[0].url, "http://references/mcp");

    assert_eq!(schema_format(&requests[1]).0, "openapi-sub-flow");
    assert_eq!(schema_format(&requests[2]).0, "asyncapi-sub-flow");
}

// A sub-flow that applies but cannot produce its artifacts reports the
// blockage as a blocking finding, which rides the merged build report
// instead of vanishing into an unconditionally-clean answer.
#[tokio::test]
async fn build_sub_flow_blocked() {
    let tmp = TempDir::new().unwrap();
    let stage = TempDir::new().unwrap();
    let blocked = r#"{"outcome":"completed","source":"model-assisted","findings":[{
        "title":"openapi sub-flow wrote nothing",
        "severity":"important",
        "source":"model-assisted",
        "kind":"violation",
        "artifact":"contracts",
        "evidence":{"kind":"snippet","value":"specs describe HTTP endpoints but design.md names no format"},
        "impact":"the slice's HTTP surface has no staged contract delta",
        "remediation":"resolve the format selection in design.md, then re-run the build"
    }],"written":[]}"#;
    let model = Harness::answering([NOT_APPLICABLE, blocked, NOT_APPLICABLE, CLOSEOUT]);

    let report = Adapter::build(
        &model,
        &ctx(tmp.path(), None),
        "demo",
        &[],
        &BuildContext::default(),
        &workspace(tmp.path(), stage.path()),
    )
    .await
    .unwrap();

    assert_eq!(report.outcome, PhaseOutcome::Completed);
    assert_eq!(report.source, PhaseSource::ModelAssisted);
    assert_eq!(report.findings.len(), 1, "the blocked leg's finding rides the build report");
    assert!(report.findings[0].blocking());
    assert_eq!(report.findings[0].source, DiagnosticSource::ModelAssisted);
}

// Fix-check dispatch coherence: build-loop operations require a lent
// stage; verify must not pass vacuously without one.
#[tokio::test]
async fn verify_missing_stage_rejected() {
    let tmp = TempDir::new().unwrap();
    let model = Harness::answering::<&str>([]);

    let err = Adapter::verify(&model, &ctx(tmp.path(), None), &merge_workspace(tmp.path()))
        .await
        .unwrap_err();

    assert!(
        matches!(err, adapter::seam::Error::InvalidRequest(_)),
        "a missing stage is a malformed dispatch: {err:?}"
    );
    assert!(model.requests().is_empty(), "no judgment leg is spent on a malformed dispatch");
}

// Dirty scripted answer through the check path: `tool` finding
// attributions are sanitized in code, and the hand-built hybrid report
// never carries outputs, a UI surface, writes, or a continuation.
#[tokio::test]
async fn verify_dirty_answer_sanitized() {
    let tmp = TempDir::new().unwrap();
    let stage = TempDir::new().unwrap();
    seed_clean_contract(&stage.path().join("contracts"));
    let dirty = r#"{"outcome":"not-applicable","source":"tool","findings":[{
        "title":"spectral lint failed",
        "severity":"important",
        "source":"tool",
        "kind":"violation",
        "artifact":"contracts",
        "evidence":{"kind":"snippet","value":"oas3-schema error"},
        "impact":"the staged OpenAPI document is invalid",
        "remediation":"fix the schema error"
    }],
    "outputs":[{"platform":"core","path":"contracts"}],
    "ui-surface":{"screens":1},
    "written":[{"root":"artifacts","path":"contracts/http/api.yaml"}]}"#;
    let model = Harness::answering([dirty]);

    let report =
        Adapter::verify(&model, &ctx(tmp.path(), None), &workspace(tmp.path(), stage.path()))
            .await
            .unwrap();

    assert_eq!(report.outcome, PhaseOutcome::Completed);
    assert_eq!(report.source, PhaseSource::Hybrid);
    assert_eq!(report.findings.len(), 1);
    assert_eq!(
        report.findings[0].source,
        DiagnosticSource::ModelAssisted,
        "tool attribution is sanitized to model-assisted"
    );
    assert!(report.outputs.is_empty(), "verify never declares outputs");
    assert!(report.ui_surface.is_none(), "verify never declares a UI surface");
    assert!(report.written.is_empty(), "verify never declares writes");
    assert!(report.next_continuation.is_none(), "verify never mutates the continuation");
}

#[tokio::test]
async fn verify_empty_delta_deterministic() {
    let tmp = TempDir::new().unwrap();
    let stage = TempDir::new().unwrap();
    let model = Harness::answering::<&str>([]);

    let report =
        Adapter::verify(&model, &ctx(tmp.path(), None), &workspace(tmp.path(), stage.path()))
            .await
            .unwrap();

    // Only the in-guest validator ran: deterministic, clean, no leg.
    assert_eq!(report.outcome, PhaseOutcome::Completed);
    assert_eq!(report.source, PhaseSource::Deterministic);
    assert!(report.findings.is_empty());
    assert!(model.requests().is_empty(), "an empty delta spends no judgment leg");
}

#[tokio::test]
async fn verify_validator_finding() {
    let tmp = TempDir::new().unwrap();
    let stage = TempDir::new().unwrap();
    seed_bad_contract(&stage.path().join("contracts"));
    let model = Harness::answering([CLEAN_PHASE_REPORT]);

    let report =
        Adapter::verify(&model, &ctx(tmp.path(), None), &workspace(tmp.path(), stage.path()))
            .await
            .unwrap();

    // Validator + model leg both contributed: the report is hybrid and
    // carries the blocking deterministic finding with its location.
    assert_eq!(report.outcome, PhaseOutcome::Completed);
    assert_eq!(report.source, PhaseSource::Hybrid);
    assert_eq!(report.findings.len(), 1);
    let finding = &report.findings[0];
    assert!(finding.blocking());
    assert_eq!(finding.rule_id.as_deref(), Some(RULE_VERSION_IS_SEMVER));
    assert_eq!(finding.source, DiagnosticSource::Deterministic);
    assert_eq!(
        finding.location.as_ref().map(|location| location.path.as_str()),
        Some("contracts/http/api.yaml"),
        "the validator's location is stage-relative"
    );
    assert!(report.outputs.is_empty() && report.ui_surface.is_none());
    assert!(report.next_continuation.is_none(), "verify never mutates the continuation");

    let requests = model.requests();
    assert_eq!(requests.len(), 1, "one check pass, no loop");
    assert!(requests[0].system.as_deref().unwrap().contains("# contracts.verify"));
    let staged = format!("{}/contracts", stage.path().display());
    assert!(requests[0].messages[0].content.contains(&staged));
    let (name, schema) = schema_format(&requests[0]);
    assert_eq!(name, "verify");
    assert_eq!(schema, PHASE_REPORT_ANSWER);
    assert_system_budget(&requests[0], "verify", 3_200); // baseline 2_870
}

#[tokio::test]
async fn verify_clean_delta() {
    let tmp = TempDir::new().unwrap();
    let stage = TempDir::new().unwrap();
    seed_clean_contract(&stage.path().join("contracts"));
    let model = Harness::answering([CLEAN_PHASE_REPORT]);

    let report =
        Adapter::verify(&model, &ctx(tmp.path(), None), &workspace(tmp.path(), stage.path()))
            .await
            .unwrap();

    assert_eq!(report.outcome, PhaseOutcome::Completed);
    assert_eq!(report.source, PhaseSource::Hybrid);
    assert!(report.findings.is_empty(), "a clean candidate passes");
    assert_eq!(model.requests().len(), 1);
}

#[tokio::test]
async fn repair_single_pass() {
    let tmp = TempDir::new().unwrap();
    let stage = TempDir::new().unwrap();
    let model = Harness::answering([
        r#"{"applicable":true,"summary":"corrected info.version","written":["contracts/http/api.yaml"]}"#,
    ]);
    let findings = vec![located_finding("contracts/http/api.yaml")];

    let report = Adapter::repair(
        &model,
        &ctx(tmp.path(), None),
        "demo",
        RepairOrigin::Verification,
        &findings,
        None,
        &workspace(tmp.path(), stage.path()),
    )
    .await
    .unwrap();

    assert_eq!(report.outcome, PhaseOutcome::Completed);
    assert_eq!(report.source, PhaseSource::ModelAssisted);
    assert!(report.findings.is_empty());
    assert!(report.outputs.is_empty(), "repair declares no outputs");
    assert!(report.ui_surface.is_none(), "repair declares no UI surface");
    assert!(report.next_continuation.is_none(), "contracts carries no session state");
    assert_eq!(
        report.written,
        vec![PhaseWrite {
            root: PhaseRoot::Artifacts,
            path: "contracts/http/api.yaml".to_string(),
        }]
    );

    let requests = model.requests();
    assert_eq!(requests.len(), 1, "exactly one findings-directed pass — the engine owns iteration");
    let system = requests[0].system.as_deref().unwrap();
    assert!(system.contains("# contracts.repair"), "repair prompt in system");
    assert!(
        system.contains("# contracts.build — openapi sub-flow"),
        "owning sub-prompt is inlined for the finding under contracts/http/"
    );
    assert!(
        !system.contains("# contracts.build — asyncapi sub-flow"),
        "unaffected sub-prompts stay out of the repair prompt"
    );
    let user = &requests[0].messages[0].content;
    assert!(user.contains("origin: `verification`"), "origin keys the pass: {user}");
    assert!(user.contains(RULE_VERSION_IS_SEMVER), "the rendered brief carries the rule id");
    assert!(user.contains("info.version is not SemVer"), "the rendered brief carries the title");
    assert_system_budget(&requests[0], "repair", 9_400); // baseline 8_571
}

#[tokio::test]
async fn review_not_applicable() {
    let tmp = TempDir::new().unwrap();
    let stage = TempDir::new().unwrap();
    let model = Harness::answering::<&str>([]);

    let report = Adapter::review(
        &model,
        &ctx(tmp.path(), None),
        "demo",
        None,
        &workspace(tmp.path(), stage.path()),
    )
    .await
    .unwrap();

    // Contracts has no standards-review team: a typed non-applicable
    // report with an honest deterministic source and no leg spent.
    assert_eq!(report.outcome, PhaseOutcome::NotApplicable);
    assert_eq!(report.source, PhaseSource::Deterministic);
    assert!(report.findings.is_empty());
    assert!(report.outputs.is_empty());
    assert!(report.ui_surface.is_none());
    assert!(report.written.is_empty());
    assert!(report.next_continuation.is_none());
    assert!(model.requests().is_empty(), "review spends no judgment leg");
}

#[tokio::test]
async fn merge_preflight_deterministic() {
    let tmp = TempDir::new().unwrap();
    let model = Harness::answering::<&str>([]);

    // A clean (absent) staged delta passes without a judgment leg.
    let report = Adapter::merge(
        &model,
        &ctx(tmp.path(), None),
        "demo",
        MergePhase::Preflight,
        &merge_workspace(tmp.path()),
    )
    .await
    .unwrap();
    assert_eq!(report.status, Status::Success);
    assert!(model.requests().is_empty(), "preflight is deterministic: no leg");

    // A broken staged delta parks the merge before the engine promotes it.
    seed_bad_contract(&tmp.path().join(".emery/change/slices/demo/contracts"));
    let report = Adapter::merge(
        &model,
        &ctx(tmp.path(), None),
        "demo",
        MergePhase::Preflight,
        &merge_workspace(tmp.path()),
    )
    .await
    .unwrap();
    assert_eq!(report.status, Status::Failure);
    assert_eq!(report.findings[0].rule_id.as_deref(), Some(RULE_VERSION_IS_SEMVER));
    assert!(model.requests().is_empty(), "a staged failure still spends no judgment leg");
}

#[tokio::test]
async fn merge_postflight_gate() {
    let tmp = TempDir::new().unwrap();
    // The merged baseline lives in the lent workspace — postflight
    // validates it there (the test tree is both project root and workspace).
    seed_bad_contract(&tmp.path().join("contracts"));
    let model = Harness::answering([SUCCESS_REPORT]);

    let report = Adapter::merge(
        &model,
        &ctx(tmp.path(), None),
        "demo",
        MergePhase::Postflight,
        &merge_workspace(tmp.path()),
    )
    .await
    .unwrap();

    assert_eq!(report.status, Status::Failure);
    assert_eq!(report.findings[0].rule_id.as_deref(), Some(RULE_VERSION_IS_SEMVER));

    let requests = model.requests();
    assert_eq!(requests.len(), 1, "one bounded repair leg on validator findings");
    assert!(requests[0].system.as_deref().unwrap().contains("# contracts.merge"));
    assert!(requests[0].messages[0].content.contains("postflight"));
    assert_system_budget(&requests[0], "merge-postflight", 6_000); // baseline 5_444
}

#[tokio::test]
async fn merge_postflight_clean_baseline() {
    let tmp = TempDir::new().unwrap();
    let model = Harness::answering::<&str>([]);

    let report = Adapter::merge(
        &model,
        &ctx(tmp.path(), None),
        "demo",
        MergePhase::Postflight,
        &merge_workspace(tmp.path()),
    )
    .await
    .unwrap();

    assert_eq!(report.status, Status::Success);
    assert!(model.requests().is_empty(), "a clean baseline spends no judgment leg");
}
