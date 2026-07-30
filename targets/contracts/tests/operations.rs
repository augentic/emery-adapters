//! Contracts build / merge operation behavior.

use std::fs;
use std::path::Path;

use adapter::answers::REPORT_ANSWER_SCHEMA;
use adapter::seam::{
    BuildContext, Context, Input, MergePhase, Payload, Severity, Status, WorkingTree,
};
use adapter::{Format, Request, Target as _};
use contracts::Adapter;
use contracts::validate::RULE_VERSION_IS_SEMVER;
use omnia_testkit::model::{Harness, mcp_grants};
use tempfile::TempDir;

const NOT_APPLICABLE: &str = r#"{"applicable":false,"summary":"no surface this format owns"}"#;
const SUCCESS_REPORT: &str = r#"{"status":"success","findings":[]}"#;

fn ctx<'a>(root: &'a Path, mcp_url: Option<&str>) -> Context<'a> {
    Context {
        adapter_id: "target:contracts",
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
/// measured baseline plus ~10% headroom (re-measured after the RFC-78 D3
/// build.md thinning).
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

#[tokio::test]
async fn build_sub_flows() {
    let tmp = TempDir::new().unwrap();
    let model =
        Harness::answering([NOT_APPLICABLE, NOT_APPLICABLE, NOT_APPLICABLE, SUCCESS_REPORT]);
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
        &tree(),
    )
    .await
    .unwrap();

    assert_eq!(report.status, Status::Success);
    assert!(report.findings.is_empty());

    let requests = model.requests();
    assert_eq!(requests.len(), 4, "three sub-flows plus one report call");
    // Budget = measured baseline (per-leg comment, 2026-07-31, after the
    // RFC-78 D3 build.md thinning) + ~10%.
    for (i, (leg, budget)) in [
        ("json-schema-sub-flow", 18_900), // baseline 17_178
        ("openapi-sub-flow", 19_000),     // baseline 17_233
        ("asyncapi-sub-flow", 18_400),    // baseline 16_668
        ("report", 11_700),               // baseline 10_559
    ]
    .into_iter()
    .enumerate()
    {
        assert_system_budget(&requests[i], leg, budget);
    }

    let first = &requests[0];
    let system = first.system.as_deref().unwrap();
    assert!(system.contains("# contracts.build"), "build prompt in system");
    assert!(system.contains("json-schema sub-flow"), "sub-prompt in system");
    let user = &first.messages[0].content;
    assert!(
        user.contains("### input: proposal → .emery/slices/demo/proposal.md")
            && user.contains("### input: design → .emery/slices/demo/design.md"),
        "typed inputs render as path-form sections: {user}"
    );
    assert!(!user.contains("PROPOSAL-BODY"), "artifact bodies are not inlined");
    assert!(
        user.contains("Read each path from the working tree"),
        "read-before-writing instruction rides the inputs block"
    );
    assert!(user.contains(".emery/slices/demo/contracts"), "slice delta dir named");
    let (name, schema) = schema_format(first);
    assert_eq!(name, "json-schema-sub-flow");
    let compiled = serde_json::from_str::<serde_json::Value>(schema).unwrap();
    assert!(jsonschema::validator_for(&compiled).is_ok(), "internal schema compiles");
    assert!(first.lend_workspace);
    assert_eq!(mcp_grants(first)[0].url, "http://references/mcp");

    assert_eq!(schema_format(&requests[1]).0, "openapi-sub-flow");
    assert_eq!(schema_format(&requests[2]).0, "asyncapi-sub-flow");
    let (name, schema) = schema_format(&requests[3]);
    assert_eq!(name, "report");
    assert_eq!(schema, REPORT_ANSWER_SCHEMA);
}

#[tokio::test]
async fn build_repair_bounded() {
    let tmp = TempDir::new().unwrap();
    seed_bad_contract(&tmp.path().join(".emery/slices/demo/contracts"));
    let model = Harness::answering([
        NOT_APPLICABLE,
        NOT_APPLICABLE,
        NOT_APPLICABLE,
        r#"{"applicable":true,"summary":"repaired"}"#,
        r#"{"applicable":true,"summary":"repaired again"}"#,
        SUCCESS_REPORT,
    ]);

    let report = Adapter::build(
        &model,
        &ctx(tmp.path(), None),
        "demo",
        &[],
        &BuildContext::default(),
        &tree(),
    )
    .await
    .unwrap();

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
    assert_system_budget(&requests[3], "repair", 19_000); // baseline 17_233
}

#[tokio::test]
async fn merge_preflight_deterministic() {
    let tmp = TempDir::new().unwrap();
    let model = Harness::answering::<&str>([]);

    // A clean (absent) staged delta passes without a judgment leg.
    let report =
        Adapter::merge(&model, &ctx(tmp.path(), None), "demo", MergePhase::Preflight, &tree())
            .await
            .unwrap();
    assert_eq!(report.status, Status::Success);
    assert!(model.requests().is_empty(), "preflight is deterministic: no leg");

    // A broken staged delta parks the merge before the engine promotes it.
    seed_bad_contract(&tmp.path().join(".emery/slices/demo/contracts"));
    let report =
        Adapter::merge(&model, &ctx(tmp.path(), None), "demo", MergePhase::Preflight, &tree())
            .await
            .unwrap();
    assert_eq!(report.status, Status::Failure);
    assert_eq!(report.findings[0].rule_id.as_deref(), Some(RULE_VERSION_IS_SEMVER));
    assert!(model.requests().is_empty(), "a staged failure still spends no judgment leg");
}

#[tokio::test]
async fn merge_postflight_gate() {
    let tmp = TempDir::new().unwrap();
    // Baseline under a working-tree subpath, mirroring a scoped mount.
    seed_bad_contract(&tmp.path().join("proj/contracts"));
    let model = Harness::answering([SUCCESS_REPORT]);
    let subpath_tree = WorkingTree {
        base: "rev-1".to_string(),
        subpath: Some("proj".to_string()),
    };

    let report = Adapter::merge(
        &model,
        &ctx(tmp.path(), None),
        "demo",
        MergePhase::Postflight,
        &subpath_tree,
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

    let report =
        Adapter::merge(&model, &ctx(tmp.path(), None), "demo", MergePhase::Postflight, &tree())
            .await
            .unwrap();

    assert_eq!(report.status, Status::Success);
    assert!(model.requests().is_empty(), "a clean baseline spends no judgment leg");
}
