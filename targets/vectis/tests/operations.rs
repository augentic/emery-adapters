use std::fs;
use std::path::Path;

use adapter::answers::REPORT_ANSWER_SCHEMA;
use adapter::seam::{Context, Input, MergePhase, Report, Severity, Status, WorkingTree};
use adapter::{Format, Request, Target as _};
use tempfile::TempDir;
use testkit::{Harness, mcp_grants};
use vectis::Vectis;

const PHASE_DONE: &str = r#"{"applicable":true,"summary":"phase complete"}"#;
const SHELL_SKIPPED: &str = r#"{"applicable":false,"summary":"no shell work in this slice"}"#;
const SUCCESS_REPORT: &str = r#"{"status":"success","findings":[]}"#;
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

#[tokio::test]
async fn build_phase_legs() {
    let tmp = TempDir::new().unwrap();
    let model = Harness::answering([
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

    let report = Vectis::build(
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
    assert_eq!(requests.len(), 6, "composition, core, two shells, review, then one report call");

    // First leg: composition.
    let first = &requests[0];
    let system = first.system.as_deref().unwrap();
    assert!(system.contains("# Vectis target — build prompt"), "build prompt in system");
    assert!(system.contains("# Vectis target — `guidance`"), "guidance refresher in system");
    assert!(system.contains("# Vectis build — composition"), "composition prompt in system");
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
    assert!(user.contains("vectis-references"), "user prompt points at the MCP references");
    let (name, schema) = schema_format(first);
    assert_eq!(name, "composition");
    let compiled = serde_json::from_str::<serde_json::Value>(schema).unwrap();
    assert!(jsonschema::validator_for(&compiled).is_ok(), "internal schema compiles");
    assert!(first.lend_workspace);
    assert_eq!(mcp_grants(first)[0].url, "http://references/mcp");

    // Phase order: core, the two shell writes, review, then the report
    // leg gated by the derived answer schema.
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
async fn core_only_skips_shells() {
    let tmp = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join(".specify")).unwrap();
    fs::write(tmp.path().join(".specify/project.yaml"), "name: demo-app\nplatforms:\n  - core\n")
        .unwrap();
    let model = Harness::answering([PHASE_DONE, PHASE_DONE, PHASE_DONE, SUCCESS_REPORT]);

    let report = Vectis::build(&model, &ctx(tmp.path(), None), "demo", &[], &tree()).await.unwrap();

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
    assert!(!review_system.contains("iOS review"), "no iOS review prompt for a core-only project");
}

#[tokio::test]
async fn composition_repair() {
    let tmp = TempDir::new().unwrap();
    // The mock never fixes the unparseable composition, so both bounded
    // repair iterations fire and no downstream leg is spent against the
    // knowingly-broken composition.
    let slice_dir = tmp.path().join(".specify/slices/demo");
    fs::create_dir_all(&slice_dir).unwrap();
    fs::write(slice_dir.join("composition.yaml"), "screens: [broken\n").unwrap();
    let model = Harness::answering([
        PHASE_DONE, // composition
        PHASE_DONE, // composition-repair 1
        PHASE_DONE, // composition-repair 2
    ]);

    let report = Vectis::build(&model, &ctx(tmp.path(), None), "demo", &[], &tree()).await.unwrap();

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

async fn build_with_composition(composition: Option<&str>, report_answer: &'static str) -> Report {
    let tmp = TempDir::new().unwrap();
    if let Some(body) = composition {
        let slice_dir = tmp.path().join(".specify/slices/demo");
        fs::create_dir_all(&slice_dir).unwrap();
        fs::write(slice_dir.join("composition.yaml"), body).unwrap();
    }
    let model = Harness::answering([
        PHASE_DONE,
        PHASE_DONE,
        SHELL_SKIPPED,
        SHELL_SKIPPED,
        PHASE_DONE,
        report_answer,
    ]);
    let report = Vectis::build(&model, &ctx(tmp.path(), None), "demo", &[], &tree()).await.unwrap();
    assert_eq!(model.requests().len(), 6, "coherence warnings never trigger the repair leg");
    report
}

#[tokio::test]
async fn ui_surface_coherence() {
    const NON_UI: &str = r#"{"status":"success","findings":[],"ui-surface":{"screens":0}}"#;
    const UI: &str = r#"{"status":"success","findings":[],"ui-surface":{"screens":2}}"#;
    const SCREENS: &str = "version: 1\nscreens:\n  home:\n    name: Home\n";
    const EMPTY_SCREENS: &str = "version: 1\nscreens: {}\n";

    // screens == 0 against a non-empty `screens:` composition warns
    // unexpected-for-non-ui.
    let report = build_with_composition(Some(SCREENS), NON_UI).await;
    assert_eq!(report.status, Status::Success, "coherence warnings never fail the report");
    assert_eq!(report.findings.len(), 1, "expected one warning, got {:?}", report.findings);
    assert_eq!(
        report.findings[0].rule_id.as_deref(),
        Some("composition-unexpected-for-non-ui-slice")
    );
    assert_eq!(report.findings[0].severity, Severity::Suggestion);
    assert!(!report.findings[0].severity.blocking(), "A4 warnings must never block");

    // screens > 0 against an empty `screens: {}` composition warns
    // empty-for-ui-slice.
    let report = build_with_composition(Some(EMPTY_SCREENS), UI).await;
    assert_eq!(report.status, Status::Success);
    assert_eq!(report.findings.len(), 1, "expected one warning, got {:?}", report.findings);
    assert_eq!(report.findings[0].rule_id.as_deref(), Some("composition-empty-for-ui-slice"));
    assert_eq!(report.findings[0].severity, Severity::Suggestion);

    // An absent composition with a UI-surface claim also flags
    // empty-for-ui (an unreadable file is treated as empty).
    let report = build_with_composition(None, UI).await;
    assert_eq!(report.status, Status::Success);
    assert_eq!(report.findings.len(), 1, "absent composition for a UI slice warns");
    assert_eq!(report.findings[0].rule_id.as_deref(), Some("composition-empty-for-ui-slice"));

    // Coherent pairs and a report without a `ui-surface` claim stay silent.
    let report = build_with_composition(Some(SCREENS), UI).await;
    assert!(report.findings.is_empty(), "ui slice + non-empty composition is coherent");
    let report = build_with_composition(Some(EMPTY_SCREENS), NON_UI).await;
    assert!(report.findings.is_empty(), "non-ui slice + empty composition is coherent");
    let report = build_with_composition(Some(SCREENS), SUCCESS_REPORT).await;
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
    let report =
        Vectis::merge(&model, &ctx(tmp.path(), None), "demo", MergePhase::Preflight, &tree())
            .await
            .unwrap();
    assert_eq!(report.status, Status::Success);
    assert!(model.requests().is_empty(), "preflight is deterministic: no leg");

    // A broken staged slice composition parks the merge before the fold.
    fs::create_dir_all(tmp.path().join(".specify/slices/demo")).unwrap();
    fs::write(tmp.path().join(".specify/slices/demo/composition.yaml"), "screens: [broken\n")
        .unwrap();
    let report =
        Vectis::merge(&model, &ctx(tmp.path(), None), "demo", MergePhase::Preflight, &tree())
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

    let report =
        Vectis::merge(&model, &ctx(tmp.path(), None), "demo", MergePhase::Postflight, &tree())
            .await
            .unwrap();

    assert_eq!(report.status, Status::Success);
    let requests = model.requests();
    assert_eq!(requests.len(), 1, "a coherent report needs no repair leg");
    assert!(requests[0].system.as_deref().unwrap().contains("# Vectis target — `merge`"));
    let user = &requests[0].messages[0].content;
    assert!(user.contains("postflight merge gate"), "phase named");
    assert!(user.contains("cap-matrix re-verification"), "agent-run host gates instructed");
}

#[tokio::test]
async fn merge_postflight_gates_composition() {
    let tmp = TempDir::new().unwrap();
    // A broken merged baseline composition is caught by the postlude's
    // in-guest validator; residual findings force failure.
    fs::create_dir_all(tmp.path().join(".specify/specs")).unwrap();
    fs::write(tmp.path().join(".specify/specs/composition.yaml"), "screens: [broken\n").unwrap();
    let model = Harness::answering([SUCCESS_REPORT, SUCCESS_REPORT]);

    let report =
        Vectis::merge(&model, &ctx(tmp.path(), None), "demo", MergePhase::Postflight, &tree())
            .await
            .unwrap();

    assert_eq!(report.status, Status::Failure);
    assert!(report.findings[0].detail.contains("[composition]"));
    assert_eq!(model.requests().len(), 2, "one merge leg plus one bounded repair leg");
}
