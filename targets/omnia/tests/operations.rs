//! Omnia build / merge operation behavior.

mod common;

use std::path::Path;

use adapter::answers::REPORT_ANSWER_SCHEMA;
use adapter::seam::{Context, Input, MergePhase, Severity, Status, WorkingTree};
use adapter::{Format, Request, Target as _};
use omnia::Adapter;
use omnia_testkit::model::{Harness, mcp_grants};
use tempfile::TempDir;

const PHASE_DONE: &str = r#"{"applicable":true,"summary":"phase complete"}"#;
const REPLAY_SKIPPED: &str = r#"{"applicable":false,"summary":"no captures binding"}"#;
const SUCCESS_REPORT: &str = r#"{"status":"success","findings":[]}"#;
fn ctx<'a>(root: &'a Path, mcp_url: Option<&str>) -> Context<'a> {
    Context {
        adapter_id: "target:omnia",
        project_root: root,
        mcp_url: mcp_url.map(str::to_owned),
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

/// RFC-78 D7 re-bloat guard: each leg's system assemble is a pure function
/// over the embedded prose registry, so its byte size is locked at the
/// measured baseline plus ~10% headroom. Budgets tighten in WP2.
fn assert_system_budget(request: &Request, leg: &str, budget: usize) {
    let bytes = request.system.as_deref().map_or(0, str::len);
    println!("{leg} system assemble: {bytes} bytes (budget {budget})");
    assert!(
        bytes <= budget,
        "{leg} system assemble is {bytes} bytes, over its {budget}-byte budget — \
         trim the assemble or deliberately raise the budget"
    );
}

#[tokio::test]
async fn build_phase_legs() {
    let tmp = TempDir::new().unwrap();
    // The scripted preparation leg writes nothing, so the checkout the
    // real agent would produce is synthesized up front.
    common::write_checkout(tmp.path());
    let model =
        Harness::answering([PHASE_DONE, PHASE_DONE, PHASE_DONE, REPLAY_SKIPPED, SUCCESS_REPORT]);
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
    assert_eq!(requests.len(), 5, "preparation, generation, review, replay, then one report call");
    // Budget = measured baseline (per-leg comment, 2026-07-31) + ~10%.
    for (i, (leg, budget)) in [
        ("preparation", 19_000), // baseline 17_202
        ("generation", 47_900),  // baseline 43_465
        ("review", 22_500),      // baseline 20_409
        ("replay", 18_900),      // baseline 17_092
        ("report", 16_100),      // baseline 14_561
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
    let system = generation.system.as_deref().unwrap();
    assert!(system.contains("# Omnia target — build prompt"), "build prompt in system");
    assert!(system.contains("# Omnia target — guidance prompt"), "guidance refresher in system");
    assert!(system.contains("# Omnia build — crate writer"), "crate writer prompt in system");
    assert!(system.contains("# Omnia build — test writer"), "test writer prompt in system");
    assert!(system.contains("# Omnia build — guest writer"), "guest writer prompt in system");
    let user = &generation.messages[0].content;
    assert!(user.contains("PROPOSAL-BODY") && user.contains("DESIGN-BODY"), "typed inputs");
    assert!(user.contains("slice `demo`"), "slice named");
    assert!(user.contains("Verify-repair loop"), "agent-run cargo verification instructed");
    assert!(user.contains("omnia-references"), "user prompt points at the MCP references");
    assert!(user.contains("### scaffold prelude"), "scaffold prelude outcome in user prompt");
    assert!(user.contains("- `Makefile.toml`"), "written tooling files listed");
    assert!(
        user.contains("Unfilled placeholders still present"),
        "unfilled publish tokens always surfaced: {user}"
    );
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
    assert!(generation.lend_workspace);
    assert_eq!(mcp_grants(generation)[0].url, "http://references/mcp");

    let review = &requests[2];
    assert_eq!(schema_format(review).0, "review");
    assert!(review.system.as_deref().unwrap().contains("# Omnia build — standards review"));
    let replay = &requests[3];
    assert_eq!(schema_format(replay).0, "replay");
    assert!(replay.system.as_deref().unwrap().contains("# Omnia build — capture replay"));
    assert!(replay.messages[0].content.contains("applicable: false"), "replay may self-skip");
    let (name, schema) = schema_format(&requests[4]);
    assert_eq!(name, "report");
    assert_eq!(schema, REPORT_ANSWER_SCHEMA);
    let report_user = &requests[4].messages[0].content;
    assert!(report_user.contains("no captures binding"), "phase outcomes feed the report leg");
    assert!(report_user.contains("- preparation:"), "preparation outcome feeds the report leg");
}

#[tokio::test]
async fn build_fails_closed_without_checkout() {
    let tmp = TempDir::new().unwrap();
    let model = Harness::answering([PHASE_DONE]);

    let error =
        Adapter::build(&model, &ctx(tmp.path(), None), "demo", &[], &tree()).await.unwrap_err();

    assert!(error.to_string().contains("preparation leg"), "names the missing step: {error}");
    assert_eq!(model.requests().len(), 1, "aborted after the preparation leg, before generation");
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
    assert_system_budget(&requests[0], "merge-preflight", 4_400); // baseline 4_000
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
