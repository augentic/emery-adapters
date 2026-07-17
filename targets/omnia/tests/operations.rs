//! Omnia build / merge operation behavior.

use std::path::Path;

use adapter::answers::REPORT_ANSWER_SCHEMA;
use adapter::seam::{Context, Input, MergePhase, Severity, Status, WorkingTree};
use adapter::{Format, Request, Target as _};
use omnia::Adapter;
use tempfile::TempDir;
use testkit::{Harness, mcp_grants};

const PHASE_DONE: &str = r#"{"applicable":true,"summary":"phase complete"}"#;
const REPLAY_SKIPPED: &str = r#"{"applicable":false,"summary":"no captures binding"}"#;
const SUCCESS_REPORT: &str = r#"{"status":"success","findings":[]}"#;
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

#[tokio::test]
async fn build_phase_legs() {
    let tmp = TempDir::new().unwrap();
    let model = Harness::answering([PHASE_DONE, PHASE_DONE, REPLAY_SKIPPED, SUCCESS_REPORT]);
    let inputs = vec![
        Input::Proposal("PROPOSAL-BODY".to_string()),
        Input::Spec("SPEC-BODY".to_string()),
        Input::Design("DESIGN-BODY".to_string()),
    ];

    let report = Adapter::build(
        &model,
        &ctx(tmp.path(), Some("http://references/mcp")),
        "demo",
        &inputs,
        &tree(),
    )
    .await
    .unwrap();

    assert_eq!(report.status, Status::Success);
    assert!(report.findings.is_empty());

    let requests = model.requests();
    assert_eq!(requests.len(), 4, "generation, review, replay, then one report call");

    let first = &requests[0];
    let system = first.system.as_deref().unwrap();
    assert!(system.contains("# Omnia target — build prompt"), "build prompt in system");
    assert!(system.contains("# Omnia target — guidance prompt"), "guidance refresher in system");
    assert!(system.contains("# Omnia build — crate writer"), "crate writer prompt in system");
    assert!(system.contains("# Omnia build — test writer"), "test writer prompt in system");
    assert!(system.contains("# Omnia build — guest writer"), "guest writer prompt in system");
    let user = &first.messages[0].content;
    assert!(user.contains("PROPOSAL-BODY") && user.contains("DESIGN-BODY"), "typed inputs");
    assert!(user.contains("slice `demo`"), "slice named");
    assert!(user.contains("Verify-repair loop"), "agent-run cargo verification instructed");
    assert!(user.contains("omnia-references"), "user prompt points at the MCP references");
    let (name, schema) = schema_format(first);
    assert_eq!(name, "generation");
    let compiled = serde_json::from_str::<serde_json::Value>(schema).unwrap();
    assert!(jsonschema::validator_for(&compiled).is_ok(), "internal schema compiles");
    assert!(first.lend_workspace);
    assert_eq!(mcp_grants(first)[0].url, "http://references/mcp");

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
async fn merge_preflight_single_leg() {
    let tmp = TempDir::new().unwrap();
    let model = Harness::answering([SUCCESS_REPORT]);

    let report =
        Adapter::merge(&model, &ctx(tmp.path(), None), "demo", MergePhase::Preflight, &tree())
            .await
            .unwrap();

    assert_eq!(report.status, Status::Success);
    let requests = model.requests();
    assert_eq!(requests.len(), 1, "a coherent report needs no repair leg");
    assert!(requests[0].system.as_deref().unwrap().contains("# Omnia target — merge prompt"));
    let user = &requests[0].messages[0].content;
    assert!(user.contains("preflight merge gate"), "phase named");
    assert!(user.contains("pre-merge gate"), "agent-run cargo verification instructed");
}

#[tokio::test]
async fn merge_postflight_deterministic() {
    let tmp = TempDir::new().unwrap();
    let model = Harness::answering::<&str>([]);

    let report =
        Adapter::merge(&model, &ctx(tmp.path(), None), "demo", MergePhase::Postflight, &tree())
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

    let report =
        Adapter::merge(&model, &ctx(tmp.path(), None), "demo", MergePhase::Preflight, &tree())
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
