//! The judgment operation template against the scripted [`MockModel`]:
//! the deterministic prepare prelude, the brief-driven phase legs, the
//! in-core composition validator gate with its bounded repair, the
//! declared-platform shell-leg filter, and the deterministic report
//! gate.

use std::fs;
use std::path::Path;

use specify_guest_kit::answers::REPORT_ANSWER_SCHEMA;
use specify_guest_kit::seam::{Changeset, Context, Edit, Input, Severity, Status, WorkingTree};
use specify_guest_kit::{Format, MockModel, Request};
use specify_vectis_core::operations::{build, guidance, merge};
use tempfile::TempDir;

const PHASE_DONE: &str = r#"{"applicable":true,"summary":"phase complete"}"#;
const SHELL_SKIPPED: &str = r#"{"applicable":false,"summary":"no shell work in this slice"}"#;
const SUCCESS_REPORT: &str = r#"{"status":"success","findings":[]}"#;
const SUCCESS_WITH_MISSING_OUTPUT: &str = r#"{"status":"success","findings":[],"outputs":[{"platform":"core","path":"shared/src/app.rs"}]}"#;

const fn ctx<'a>(root: &'a Path, mcp_url: Option<&'a str>) -> Context<'a> {
    Context {
        adapter_id: "target:vectis",
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
    assert!(guidance().starts_with("# Vectis target — `shape`"));
}

#[tokio::test]
async fn build_runs_prelude_then_phase_legs_then_report() {
    let tmp = TempDir::new().unwrap();
    let model = MockModel::answering([
        PHASE_DONE,     // composition
        PHASE_DONE,     // core
        SHELL_SKIPPED,  // ios
        SHELL_SKIPPED,  // android
        PHASE_DONE,     // review
        SUCCESS_REPORT, // report
    ]);
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
    assert_eq!(requests.len(), 6, "composition, core, two shells, review, then one report call");

    // First leg: composition — the orchestrator brief plus the shape
    // refresher and the composition sub-brief, the deterministic prepare
    // prelude's summary, the adapter's own MCP grant, and the workspace
    // lend.
    let first = &requests[0];
    let system = first.system.as_deref().unwrap();
    assert!(system.contains("# Vectis target — build brief"), "build brief in system");
    assert!(system.contains("# Vectis target — `shape`"), "shape refresher in system");
    assert!(system.contains("# Vectis build — composition"), "composition sub-brief in system");
    let user = &first.messages[0].content;
    assert!(user.contains("PROPOSAL-BODY") && user.contains("DESIGN-BODY"), "typed inputs");
    assert!(user.contains("slice `demo`"), "slice named");
    assert!(user.contains("prepare prelude"), "prelude summary feeds the first leg");
    assert!(user.contains("\"skipped\":true"), "nothing to materialize in an empty workspace");
    assert!(
        user.contains("component-identity cluster report"),
        "in-guest infer report feeds the leg"
    );
    assert!(user.contains("component-bindings.yaml"), "bindings file instructed");
    assert!(!user.contains("specify catalog infer"), "no dead CLI verb in the prompt");
    assert!(user.contains("vectis-references"), "user prompt points at the MCP shelf");
    let (name, schema) = schema_format(first);
    assert_eq!(name, "composition");
    let compiled = serde_json::from_str::<serde_json::Value>(schema).unwrap();
    assert!(jsonschema::validator_for(&compiled).is_ok(), "internal schema compiles");
    assert!(first.lend_workspace);
    assert_eq!(first.mcp[0].url, "http://shelf/mcp");

    // The brief's phase order: core (Phases 2-3), the two shell writes
    // (Phases 4-5), review (Phases 6-7), then the report leg gated by
    // the derived answer schema.
    let core = &requests[1];
    assert_eq!(schema_format(core).0, "core");
    assert!(core.system.as_deref().unwrap().contains("# Vectis build — core (write)"));
    assert!(
        core.system.as_deref().unwrap().contains("# Vectis build — tests + core verify-repair")
    );
    assert!(core.messages[0].content.contains("cannot spawn"), "agent-run cargo loop instructed");
    let ios = &requests[2];
    assert_eq!(schema_format(ios).0, "ios");
    assert!(ios.system.as_deref().unwrap().contains("# Vectis build — iOS shell (write + verify)"));
    assert!(ios.messages[0].content.contains("applicable: false"), "shell legs may self-skip");
    let android = &requests[3];
    assert_eq!(schema_format(android).0, "android");
    assert!(
        android
            .system
            .as_deref()
            .unwrap()
            .contains("# Vectis build — Android shell (write + verify)")
    );
    let review = &requests[4];
    assert_eq!(schema_format(review).0, "review");
    assert!(review.system.as_deref().unwrap().contains("# Vectis build — core review"));
    assert!(review.system.as_deref().unwrap().contains("# Vectis build — iOS review"));
    assert!(review.system.as_deref().unwrap().contains("# Vectis build — Android review"));
    let (name, schema) = schema_format(&requests[5]);
    assert_eq!(name, "report");
    assert_eq!(schema, REPORT_ANSWER_SCHEMA);
    let report_user = &requests[5].messages[0].content;
    assert!(report_user.contains("no shell work"), "phase outcomes feed the report leg");
    assert!(report_user.contains("shell verify gate"), "in-guest verify gate feeds the report");
    assert!(!report_user.contains("specify extension run"), "no dead CLI verb in the prompt");
}

#[tokio::test]
async fn declared_core_only_platforms_skip_the_shell_legs() {
    let tmp = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join(".specify")).unwrap();
    fs::write(tmp.path().join(".specify/project.yaml"), "name: demo-app\nplatforms:\n  - core\n")
        .unwrap();
    let model = MockModel::answering([PHASE_DONE, PHASE_DONE, PHASE_DONE, SUCCESS_REPORT]);

    let report = build(&model, &ctx(tmp.path(), None), "demo", &[], &tree()).await.unwrap();

    assert_eq!(report.status, Status::Success);
    let requests = model.requests();
    assert_eq!(requests.len(), 4, "composition, core, review, report — no shell legs");
    assert_eq!(schema_format(&requests[1]).0, "core");
    assert_eq!(schema_format(&requests[2]).0, "review");
    let core_user = &requests[1].messages[0].content;
    assert!(
        core_user.contains("scaffolded `core` for app `DemoApp`"),
        "deterministic scaffold prelude stood the core tree up"
    );
    assert!(
        tmp.path().join("shared/src/app.rs").is_file(),
        "core scaffold rendered from the embedded templates"
    );
    let review_system = requests[2].system.as_deref().unwrap();
    assert!(!review_system.contains("iOS review"), "no iOS review brief for a core-only project");
}

#[tokio::test]
async fn composition_validator_findings_feed_the_bounded_repair_loop() {
    let tmp = TempDir::new().unwrap();
    // An unparseable composition triggers the in-core validator gate;
    // the mock never fixes the file, so both bounded repair iterations
    // fire, the exhausted gate parks the slice with a deterministic
    // failure report, and no downstream core / shell / review / report
    // leg is spent against the knowingly-broken composition.
    let slice_dir = tmp.path().join(".specify/slices/demo");
    fs::create_dir_all(&slice_dir).unwrap();
    fs::write(slice_dir.join("composition.yaml"), "screens: [broken\n").unwrap();
    let model = MockModel::answering([
        PHASE_DONE, // composition
        PHASE_DONE, // composition-repair 1
        PHASE_DONE, // composition-repair 2
    ]);

    let report = build(&model, &ctx(tmp.path(), None), "demo", &[], &tree()).await.unwrap();

    assert_eq!(report.status, Status::Failure, "an exhausted gate parks the slice");
    assert_eq!(report.findings[0].severity, Severity::Important);
    assert!(report.findings[0].detail.contains("[composition]"), "finding names the validator");
    assert!(report.outputs.is_empty(), "no platform phase ran, so no outputs are declared");

    let requests = model.requests();
    assert_eq!(requests.len(), 3, "the composition leg plus both bounded repairs, nothing more");
    let repair = &requests[1];
    assert_eq!(schema_format(repair).0, "composition-repair");
    assert!(repair.messages[0].content.contains("composition validator found blocking issues"));
}

#[tokio::test]
async fn missing_output_triggers_bounded_repair_then_enforcement() {
    let tmp = TempDir::new().unwrap();
    // The success report declares `shared/src/app.rs`, which never
    // appears in the tree; the single bounded repair leg fires and the
    // residual discrepancy overrides the repeated success answer.
    let model = MockModel::answering([
        PHASE_DONE,
        PHASE_DONE,
        SHELL_SKIPPED,
        SHELL_SKIPPED,
        PHASE_DONE,
        SUCCESS_WITH_MISSING_OUTPUT,
        SUCCESS_WITH_MISSING_OUTPUT,
    ]);

    let report = build(&model, &ctx(tmp.path(), None), "demo", &[], &tree()).await.unwrap();

    assert_eq!(report.status, Status::Failure, "residual discrepancy forces failure");
    assert!(report.findings[0].detail.contains("shared/src/app.rs"));

    let requests = model.requests();
    assert_eq!(requests.len(), 7, "five phases, one report, one bounded repair");
    assert!(requests[6].messages[0].content.contains("does not exist"));
}

#[tokio::test]
async fn declared_outputs_that_exist_pass_the_gate() {
    let tmp = TempDir::new().unwrap();
    // Outputs resolve beneath the working-tree subpath, mirroring how a
    // deployment scopes the shared mount.
    fs::create_dir_all(tmp.path().join("proj/shared/src")).unwrap();
    fs::write(tmp.path().join("proj/shared/src/app.rs"), "pub struct App;").unwrap();
    let model = MockModel::answering([
        PHASE_DONE,
        PHASE_DONE,
        SHELL_SKIPPED,
        SHELL_SKIPPED,
        PHASE_DONE,
        SUCCESS_WITH_MISSING_OUTPUT,
    ]);
    let subpath_tree = WorkingTree {
        base: "rev-1".to_string(),
        subpath: Some("proj".to_string()),
    };

    let report = build(&model, &ctx(tmp.path(), None), "demo", &[], &subpath_tree).await.unwrap();

    assert_eq!(report.status, Status::Success);
    assert_eq!(model.requests().len(), 6, "no repair leg when the declared outputs exist");
}

#[tokio::test]
async fn merge_is_one_report_leg_with_postlude_gate() {
    let tmp = TempDir::new().unwrap();
    let model = MockModel::answering([SUCCESS_REPORT]);
    let delta = Changeset {
        base: "rev-1".to_string(),
        edits: vec![
            Edit {
                path: "shared/src/app.rs".to_string(),
                content: Some("pub struct App;".to_string()),
            },
            Edit {
                path: "shared/src/old.rs".to_string(),
                content: None,
            },
        ],
    };

    let report = merge(&model, &ctx(tmp.path(), None), "demo", &delta, &tree()).await.unwrap();

    assert_eq!(report.status, Status::Success);
    let requests = model.requests();
    assert_eq!(requests.len(), 1, "a coherent report needs no repair leg");
    assert!(requests[0].system.as_deref().unwrap().contains("# Vectis target — `merge`"));
    let user = &requests[0].messages[0].content;
    assert!(user.contains("cap-matrix re-verification"), "agent-run host gates instructed");
    assert!(user.contains("shared/src/old.rs (deleted)"), "delta rendered");
    assert!(user.contains("base `rev-1`"), "delta base named");
}

#[tokio::test]
async fn merge_gates_the_merged_baseline_composition() {
    let tmp = TempDir::new().unwrap();
    // A broken merged baseline composition is caught by the postlude's
    // in-core validator; the bounded repair leg fires once and the
    // residual findings force failure.
    fs::create_dir_all(tmp.path().join(".specify/specs")).unwrap();
    fs::write(tmp.path().join(".specify/specs/composition.yaml"), "screens: [broken\n").unwrap();
    let model = MockModel::answering([SUCCESS_REPORT, SUCCESS_REPORT]);
    let delta = Changeset {
        base: "rev-1".to_string(),
        edits: vec![],
    };

    let report = merge(&model, &ctx(tmp.path(), None), "demo", &delta, &tree()).await.unwrap();

    assert_eq!(report.status, Status::Failure);
    assert!(report.findings[0].detail.contains("[composition]"));
    assert_eq!(model.requests().len(), 2, "one merge leg plus one bounded repair leg");
}

#[tokio::test]
async fn merge_success_with_blocking_finding_downgrades() {
    let tmp = TempDir::new().unwrap();
    // A `success` answer carrying a blocking finding violates the report
    // contract; the deterministic guard downgrades rather than trusting it.
    let model = MockModel::answering([
        r#"{"status":"success","findings":[{"rule-id":"VECTIS-006","title":"Glyph substituted for vector asset","severity":"critical","impact":"The committed export is ignored.","remediation":"Render by assets.yaml kind."}]}"#,
    ]);
    let delta = Changeset {
        base: "rev-1".to_string(),
        edits: vec![],
    };

    let report = merge(&model, &ctx(tmp.path(), None), "demo", &delta, &tree()).await.unwrap();

    assert_eq!(report.status, Status::Failure);
    assert_eq!(report.findings[0].rule_id.as_deref(), Some("VECTIS-006"));
}
