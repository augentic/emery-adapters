//! The judgment operation template against the scripted [`MockModel`]:
//! prompt assembly, schema-gated formats, answer projection, the
//! bounded verify-repair loop, and validate-before-visible enforcement.

use std::fs;
use std::path::Path;

use adapter::answers::REPORT_ANSWER_SCHEMA;
use adapter::seam::{Changeset, Context, Edit, Error, Input, Severity, Status, WorkingTree};
use adapter::{Error as ModelError, Format, MockModel, Request};
use contracts_core::operations::{build, describe, guidance, merge};
use contracts_core::validate::RULE_VERSION_IS_SEMVER;
use tempfile::TempDir;

const NOT_APPLICABLE: &str = r#"{"applicable":false,"summary":"no surface this format owns"}"#;
const SUCCESS_REPORT: &str = r#"{"status":"success","findings":[]}"#;

const fn ctx<'a>(root: &'a Path, mcp_url: Option<&'a str>) -> Context<'a> {
    Context {
        adapter_id: "target:contracts",
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

/// Seed one top-level contract whose `info.version` is not SemVer.
fn seed_bad_contract(dir: &Path) {
    fs::create_dir_all(dir.join("http")).unwrap();
    fs::write(
        dir.join("http/api.yaml"),
        "openapi: '3.1.0'\ninfo:\n  title: API\n  version: 2024-01-15\n",
    )
    .unwrap();
}

#[test]
fn guidance_returns_embedded_prompt() {
    assert!(guidance().starts_with("# contracts.guidance"));
}

#[tokio::test]
async fn build_runs_sub_flows_then_report() {
    let tmp = TempDir::new().unwrap();
    let model =
        MockModel::answering([NOT_APPLICABLE, NOT_APPLICABLE, NOT_APPLICABLE, SUCCESS_REPORT]);
    let inputs = vec![
        Input::Proposal("PROPOSAL-BODY".to_string()),
        Input::Design("DESIGN-BODY".to_string()),
    ];

    let report =
        build(&model, &ctx(tmp.path(), Some("http://shelf/mcp")), "demo", &inputs, &tree())
            .await
            .unwrap();

    assert_eq!(report.status, Status::Success);
    assert!(report.findings.is_empty());

    let requests = model.requests();
    assert_eq!(requests.len(), 4, "three sub-flows plus one report call");

    // First leg: the json-schema sub-flow, the assembled system prompt,
    // the adapter's own MCP grant, and the workspace lend.
    let first = &requests[0];
    let system = first.system.as_deref().unwrap();
    assert!(system.contains("# contracts.build"), "build prompt in system");
    assert!(system.contains("json-schema sub-flow"), "sub-prompt in system");
    let user = &first.messages[0].content;
    assert!(user.contains("PROPOSAL-BODY") && user.contains("DESIGN-BODY"), "typed inputs");
    assert!(user.contains(".specify/slices/demo/contracts"), "slice delta dir named");
    let (name, schema) = schema_format(first);
    assert_eq!(name, "json-schema-sub-flow");
    let compiled = serde_json::from_str::<serde_json::Value>(schema).unwrap();
    assert!(jsonschema::validator_for(&compiled).is_ok(), "internal schema compiles");
    assert!(first.lend_workspace);
    assert_eq!(first.mcp[0].url, "http://shelf/mcp");

    // Fixed sub-flow order, then the report leg gated by the derived
    // answer schema.
    assert_eq!(schema_format(&requests[1]).0, "openapi-sub-flow");
    assert_eq!(schema_format(&requests[2]).0, "asyncapi-sub-flow");
    let (name, schema) = schema_format(&requests[3]);
    assert_eq!(name, "report");
    assert_eq!(schema, REPORT_ANSWER_SCHEMA);
}

#[tokio::test]
async fn build_repair_loop_is_bounded() {
    let tmp = TempDir::new().unwrap();
    seed_bad_contract(&tmp.path().join(".specify/slices/demo/contracts"));
    // The mock never fixes the file, so both repair iterations fire and
    // the residual finding overrides the success answer.
    let model = MockModel::answering([
        NOT_APPLICABLE,
        NOT_APPLICABLE,
        NOT_APPLICABLE,
        r#"{"applicable":true,"summary":"repaired"}"#,
        r#"{"applicable":true,"summary":"repaired again"}"#,
        SUCCESS_REPORT,
    ]);

    let report = build(&model, &ctx(tmp.path(), None), "demo", &[], &tree()).await.unwrap();

    assert_eq!(report.status, Status::Failure, "residual validator finding forces failure");
    let finding = &report.findings[0];
    assert_eq!(finding.rule_id.as_deref(), Some(RULE_VERSION_IS_SEMVER));
    assert_eq!(finding.severity, Severity::Important);

    let requests = model.requests();
    assert_eq!(requests.len(), 6, "three sub-flows, two repairs, one report");
    let repair = &requests[3].messages[0].content;
    assert!(repair.contains(RULE_VERSION_IS_SEMVER), "repair prompt carries the finding");
    assert!(repair.contains("http/api.yaml"), "repair prompt names the file");
    let repair_system = requests[3].system.as_deref().unwrap();
    assert!(
        repair_system.contains("# contracts.build — openapi sub-flow"),
        "owning sub-prompt is inlined for the finding under http/"
    );
    assert!(
        !repair_system.contains("# contracts.build — asyncapi sub-flow"),
        "unaffected sub-prompts stay out of the repair prompt"
    );
}

#[tokio::test]
async fn malformed_answer_fails_internal() {
    let tmp = TempDir::new().unwrap();
    let model = MockModel::answering(["this is not json"]);

    let err = build(&model, &ctx(tmp.path(), None), "demo", &[], &tree()).await.unwrap_err();

    match err {
        Error::Internal(detail) => assert!(detail.contains("sub-flow answer")),
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
async fn merge_projects_diagnostic_onto_seam() {
    let tmp = TempDir::new().unwrap();
    let model = MockModel::answering([
        r#"{"status":"failure","findings":[{"rule-id":"UNI-014","title":"Duplicate id","severity":"critical","impact":"Baseline is ambiguous.","remediation":"Rename one contract."}]}"#,
    ]);
    let delta = Changeset {
        base: "rev-1".to_string(),
        edits: vec![Edit {
            path: "contracts/http/api.yaml".to_string(),
            content: None,
        }],
    };

    let report = merge(&model, &ctx(tmp.path(), None), "demo", &delta, &tree()).await.unwrap();

    assert_eq!(report.status, Status::Failure);
    let finding = &report.findings[0];
    assert_eq!(finding.rule_id.as_deref(), Some("UNI-014"));
    assert_eq!(finding.severity, Severity::Critical);
    assert_eq!(
        finding.detail,
        "Duplicate id — Baseline is ambiguous.; remediation: Rename one contract."
    );

    let requests = model.requests();
    assert_eq!(requests.len(), 1, "clean baseline needs no repair leg");
    assert!(requests[0].system.as_deref().unwrap().contains("# contracts.merge"));
    assert!(requests[0].messages[0].content.contains("contracts/http/api.yaml (deleted)"));
}

#[tokio::test]
async fn success_with_blocking_finding_downgrades() {
    let tmp = TempDir::new().unwrap();
    // A `success` answer carrying a blocking finding violates the report
    // contract; the deterministic guard downgrades rather than trusting it.
    let model = MockModel::answering([
        r#"{"status":"success","findings":[{"title":"Broken ref","severity":"important","impact":"Consumers break.","remediation":"Fix the $ref."}]}"#,
    ]);
    let delta = Changeset {
        base: "rev-1".to_string(),
        edits: vec![],
    };

    let report = merge(&model, &ctx(tmp.path(), None), "demo", &delta, &tree()).await.unwrap();

    assert_eq!(report.status, Status::Failure);
}

#[tokio::test]
async fn merge_post_gate_repairs_then_enforces() {
    let tmp = TempDir::new().unwrap();
    // Baseline under a working-tree subpath; the mock's repair leg never
    // fixes the file, so enforcement appends the validator finding.
    seed_bad_contract(&tmp.path().join("proj/contracts"));
    let model = MockModel::answering([SUCCESS_REPORT, SUCCESS_REPORT]);
    let delta = Changeset {
        base: "rev-1".to_string(),
        edits: vec![],
    };
    let subpath_tree = WorkingTree {
        base: "rev-1".to_string(),
        subpath: Some("proj".to_string()),
    };

    let report =
        merge(&model, &ctx(tmp.path(), None), "demo", &delta, &subpath_tree).await.unwrap();

    assert_eq!(report.status, Status::Failure);
    assert_eq!(report.findings[0].rule_id.as_deref(), Some(RULE_VERSION_IS_SEMVER));

    let requests = model.requests();
    assert_eq!(requests.len(), 2, "one merge leg plus one bounded repair leg");
    assert!(requests[1].messages[0].content.contains("post-merge"));
}

// The RFC-64 self-description is answerable without a model or a
// filesystem: no floor, one optional `contracts` build input, no
// platform capability.
#[test]
fn describe_declares_the_contracts_input() {
    let manifest = describe();
    assert_eq!(manifest.specify_floor, None);
    let declared: Vec<(&str, bool)> =
        manifest.inputs.iter().map(|input| (input.path.as_str(), input.required)).collect();
    assert_eq!(declared, [("contracts", false)]);
    assert_eq!(manifest.platforms, None);
}
