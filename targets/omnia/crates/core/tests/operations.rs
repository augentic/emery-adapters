//! The judgment operation template against the scripted [`MockModel`]:
//! prompt assembly, schema-gated formats, the phase-leg decomposition,
//! and the deterministic report-coherence gate with its bounded repair.

use std::fs;
use std::path::Path;

use specify_guest_kit::answers::REPORT_ANSWER_SCHEMA;
use specify_guest_kit::seam::{
    Changeset, Context, Edit, Error, Input, Severity, Status, WorkingTree,
};
use specify_guest_kit::{Error as ModelError, Format, MockModel, Request};
use specify_omnia_core::operations::{build, guidance, merge};
use tempfile::TempDir;

const PHASE_DONE: &str = r#"{"applicable":true,"summary":"phase complete"}"#;
const REPLAY_SKIPPED: &str = r#"{"applicable":false,"summary":"no captures binding"}"#;
const SUCCESS_REPORT: &str = r#"{"status":"success","findings":[]}"#;
const SUCCESS_WITH_MISSING_OUTPUT: &str =
    r#"{"status":"success","findings":[],"outputs":[{"platform":"core","path":"crates/demo"}]}"#;

const fn ctx<'a>(root: &'a Path, mcp_url: Option<&'a str>) -> Context<'a> {
    Context {
        adapter_id: "target:omnia",
        project_root: root,
        mcp_url,
    }
}

fn tree() -> WorkingTree {
    WorkingTree {
        base: "rev-1".to_string(),
        subpath: None,
    }
}

fn schema_format(request: &Request) -> (&str, &str) {
    match &request.format {
        Format::Schema(schema) => (&schema.name, &schema.schema),
        other => panic!("expected schema format, got {other:?}"),
    }
}

#[test]
fn guidance_returns_embedded_shape_brief() {
    assert!(guidance().starts_with("# Omnia target — shape brief"));
}

#[tokio::test]
async fn build_runs_phase_legs_then_report() {
    let tmp = TempDir::new().unwrap();
    let model = MockModel::answering([PHASE_DONE, PHASE_DONE, REPLAY_SKIPPED, SUCCESS_REPORT]);
    let inputs = vec![
        Input::Proposal("PROPOSAL-BODY".to_string()),
        Input::Spec("SPEC-BODY".to_string()),
        Input::Design("DESIGN-BODY".to_string()),
    ];

    let report =
        build(&model, &ctx(tmp.path(), Some("http://shelf/mcp")), "demo", &inputs, &tree())
            .await
            .unwrap();

    assert_eq!(report.status, Status::Success);
    assert!(report.findings.is_empty());

    let requests = model.requests();
    assert_eq!(requests.len(), 4, "generation, review, replay, then one report call");

    // First leg: generation — the orchestrator brief plus the shape
    // refresher and all three writer sub-briefs (the verify-repair loop
    // crosses them), the adapter's own MCP grant, and the workspace lend.
    let first = &requests[0];
    let system = first.system.as_deref().unwrap();
    assert!(system.contains("# Omnia target — build brief"), "build brief in system");
    assert!(system.contains("# Omnia target — shape brief"), "shape refresher in system");
    assert!(system.contains("# Omnia build — crate writer"), "crate sub-brief in system");
    assert!(system.contains("# Omnia build — test writer"), "test sub-brief in system");
    assert!(system.contains("# Omnia build — guest writer"), "guest sub-brief in system");
    let user = &first.messages[0].content;
    assert!(user.contains("PROPOSAL-BODY") && user.contains("DESIGN-BODY"), "typed inputs");
    assert!(user.contains("slice `demo`"), "slice named");
    assert!(user.contains("Verify-repair loop"), "agent-run cargo verification instructed");
    assert!(user.contains("omnia-references"), "user prompt points at the MCP shelf");
    let (name, schema) = schema_format(first);
    assert_eq!(name, "generation");
    let compiled = serde_json::from_str::<serde_json::Value>(schema).unwrap();
    assert!(jsonschema::validator_for(&compiled).is_ok(), "internal schema compiles");
    assert!(first.lend_workspace);
    assert_eq!(first.mcp[0].url, "http://shelf/mcp");

    // Fixed phase order: review carries the review sub-brief, replay the
    // replay sub-brief, then the report leg gated by the derived answer
    // schema.
    let review = &requests[1];
    assert_eq!(schema_format(review).0, "review");
    assert!(review.system.as_deref().unwrap().contains("# Omnia build — standards review"));
    let replay = &requests[2];
    assert_eq!(schema_format(replay).0, "replay");
    assert!(replay.system.as_deref().unwrap().contains("# Omnia build — capture replay"));
    assert!(replay.messages[0].content.contains("applicable: false"), "replay may self-skip");
    let (name, schema) = schema_format(&requests[3]);
    assert_eq!(name, "report");
    assert_eq!(schema, REPORT_ANSWER_SCHEMA);
    let report_user = &requests[3].messages[0].content;
    assert!(report_user.contains("no captures binding"), "phase outcomes feed the report leg");
}

#[tokio::test]
async fn missing_output_triggers_bounded_repair_then_enforcement() {
    let tmp = TempDir::new().unwrap();
    // The success report declares `crates/demo`, which never appears in
    // the tree; the single bounded repair leg fires and the residual
    // discrepancy overrides the repeated success answer.
    let model = MockModel::answering([
        PHASE_DONE,
        PHASE_DONE,
        REPLAY_SKIPPED,
        SUCCESS_WITH_MISSING_OUTPUT,
        SUCCESS_WITH_MISSING_OUTPUT,
    ]);

    let report = build(&model, &ctx(tmp.path(), None), "demo", &[], &tree()).await.unwrap();

    assert_eq!(report.status, Status::Failure, "residual discrepancy forces failure");
    let finding = &report.findings[0];
    assert_eq!(finding.rule_id, None);
    assert_eq!(finding.severity, Severity::Important);
    assert!(finding.detail.contains("crates/demo"), "finding names the missing output");

    let requests = model.requests();
    assert_eq!(requests.len(), 5, "three phases, one report, one bounded repair");
    let repair = &requests[4].messages[0].content;
    assert!(repair.contains("crates/demo"), "repair prompt names the missing output");
    assert!(repair.contains("does not exist"), "repair prompt carries the discrepancy");
}

#[tokio::test]
async fn declared_outputs_that_exist_pass_the_gate() {
    let tmp = TempDir::new().unwrap();
    // Outputs resolve beneath the working-tree subpath, mirroring how a
    // deployment scopes the shared mount.
    fs::create_dir_all(tmp.path().join("proj/crates/demo")).unwrap();
    let model =
        MockModel::answering([PHASE_DONE, PHASE_DONE, REPLAY_SKIPPED, SUCCESS_WITH_MISSING_OUTPUT]);
    let subpath_tree = WorkingTree {
        base: "rev-1".to_string(),
        subpath: Some("proj".to_string()),
    };

    let report = build(&model, &ctx(tmp.path(), None), "demo", &[], &subpath_tree).await.unwrap();

    assert_eq!(report.status, Status::Success);
    assert_eq!(model.requests().len(), 4, "no repair leg when the declared outputs exist");
}

#[tokio::test]
async fn failure_report_is_terminal_without_repair() {
    let tmp = TempDir::new().unwrap();
    // A failure report parks the slice per the brief's stop contract; the
    // gate must not spend a repair leg re-litigating its output claims.
    let model = MockModel::answering([
        PHASE_DONE,
        PHASE_DONE,
        REPLAY_SKIPPED,
        r#"{"status":"failure","findings":[],"outputs":[{"platform":"core","path":"crates/never-written"}]}"#,
    ]);

    let report = build(&model, &ctx(tmp.path(), None), "demo", &[], &tree()).await.unwrap();

    assert_eq!(report.status, Status::Failure);
    assert_eq!(model.requests().len(), 4, "failure reports take no repair leg");
}

#[tokio::test]
async fn malformed_answer_fails_internal() {
    let tmp = TempDir::new().unwrap();
    let model = MockModel::answering(["this is not json"]);

    let err = build(&model, &ctx(tmp.path(), None), "demo", &[], &tree()).await.unwrap_err();

    match err {
        Error::Internal(detail) => assert!(detail.contains("generation answer")),
        other => panic!("expected internal error, got {other:?}"),
    }
}

#[tokio::test]
async fn model_invalid_request_maps_through() {
    let tmp = TempDir::new().unwrap();
    let model =
        MockModel::scripted([Err(ModelError::InvalidRequest("messages must not be empty".into()))]);

    let err = build(&model, &ctx(tmp.path(), None), "demo", &[], &tree()).await.unwrap_err();

    assert!(matches!(err, Error::InvalidRequest(_)));
}

#[tokio::test]
async fn merge_is_one_report_leg_with_pre_merge_gate_instructions() {
    let tmp = TempDir::new().unwrap();
    let model = MockModel::answering([SUCCESS_REPORT]);
    let delta = Changeset {
        base: "rev-1".to_string(),
        edits: vec![
            Edit {
                path: "crates/demo/src/lib.rs".to_string(),
                content: Some("pub fn demo() {}".to_string()),
            },
            Edit {
                path: "crates/demo/old.rs".to_string(),
                content: None,
            },
        ],
    };

    let report = merge(&model, &ctx(tmp.path(), None), "demo", &delta, &tree()).await.unwrap();

    assert_eq!(report.status, Status::Success);
    let requests = model.requests();
    assert_eq!(requests.len(), 1, "a coherent report needs no repair leg");
    assert!(requests[0].system.as_deref().unwrap().contains("# Omnia target — merge brief"));
    let user = &requests[0].messages[0].content;
    assert!(user.contains("pre-merge gate"), "agent-run cargo verification instructed");
    assert!(user.contains("crates/demo/old.rs (deleted)"), "delta rendered");
    assert!(user.contains("base `rev-1`"), "delta base named");
}

#[tokio::test]
async fn merge_projects_diagnostic_onto_seam() {
    let tmp = TempDir::new().unwrap();
    let model = MockModel::answering([
        r#"{"status":"failure","findings":[{"rule-id":"OMNIA-002","title":"Forbidden std API","severity":"critical","impact":"The wasm32 build breaks.","remediation":"Route through the provider trait."}]}"#,
    ]);
    let delta = Changeset {
        base: "rev-1".to_string(),
        edits: vec![],
    };

    let report = merge(&model, &ctx(tmp.path(), None), "demo", &delta, &tree()).await.unwrap();

    assert_eq!(report.status, Status::Failure);
    let finding = &report.findings[0];
    assert_eq!(finding.rule_id.as_deref(), Some("OMNIA-002"));
    assert_eq!(finding.severity, Severity::Critical);
    assert_eq!(
        finding.detail,
        "Forbidden std API — The wasm32 build breaks.; remediation: Route through the provider trait."
    );
}

#[tokio::test]
async fn merge_success_with_blocking_finding_downgrades() {
    let tmp = TempDir::new().unwrap();
    // A `success` answer carrying a blocking finding violates the report
    // contract; the deterministic guard downgrades rather than trusting it.
    let model = MockModel::answering([
        r#"{"status":"success","findings":[{"title":"Clippy regression","severity":"important","impact":"CI fails.","remediation":"Fix the lint."}]}"#,
    ]);
    let delta = Changeset {
        base: "rev-1".to_string(),
        edits: vec![],
    };

    let report = merge(&model, &ctx(tmp.path(), None), "demo", &delta, &tree()).await.unwrap();

    assert_eq!(report.status, Status::Failure);
}

#[tokio::test]
async fn merge_missing_output_repairs_then_enforces() {
    let tmp = TempDir::new().unwrap();
    // The mock never writes the declared output, so the bounded repair
    // leg fires once and enforcement appends the residual discrepancy.
    let model = MockModel::answering([SUCCESS_WITH_MISSING_OUTPUT, SUCCESS_WITH_MISSING_OUTPUT]);
    let delta = Changeset {
        base: "rev-1".to_string(),
        edits: vec![],
    };

    let report = merge(&model, &ctx(tmp.path(), None), "demo", &delta, &tree()).await.unwrap();

    assert_eq!(report.status, Status::Failure);
    assert!(report.findings[0].detail.contains("crates/demo"));
    assert_eq!(model.requests().len(), 2, "one merge leg plus one bounded repair leg");
}
