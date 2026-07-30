//! Omnia build / merge operation behavior.

mod common;

use std::path::Path;

use adapter::answers::REPORT_ANSWER_SCHEMA;
use adapter::seam::{
    BuildContext, Context, Input, MergePhase, Payload, Platform, Severity, Status, WorkingTree,
};
use adapter::{Format, Request, Target as _};
use omnia::Adapter;
use omnia_testkit::model::{Harness, mcp_grants};
use tempfile::TempDir;

const PHASE_DONE: &str = r#"{"applicable":true,"summary":"phase complete"}"#;
const REVIEW_DONE: &str = r#"{"applicable":true,"summary":"review complete","written":["crates/demo/REVIEW.md"],"findings":[],"outputs":[{"platform":"core","path":"crates/demo"}]}"#;
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

fn input(path: &str) -> Payload {
    Payload::Path(path.to_string())
}

fn schema_format(request: &Request) -> (&str, &str) {
    match &request.format {
        Format::Schema(schema) => (&schema.name, &schema.schema),
        other => panic!("expected schema format, got {other:?}"),
    }
}

/// RFC-78 D7 re-bloat guard: each leg's system assemble is a pure function
/// over the embedded prose registry, so its byte size is locked at the
/// measured baseline plus ~10% headroom (re-measured after WP2's path-first inputs).
fn assert_system_budget(request: &Request, leg: &str, budget: usize) {
    let bytes = request.system.as_deref().map_or(0, str::len);
    println!("{leg} system assemble: {bytes} bytes (budget {budget})");
    assert!(
        bytes <= budget,
        "{leg} system assemble is {bytes} bytes, over its {budget}-byte budget — \
         trim the assemble or deliberately raise the budget"
    );
}

/// The generation leg's create-mode assemble and path-form user prompt
/// (RFC-78 D1/D2): guidance dropped, guest writer present, inputs as
/// project-relative path sections with a read-before-writing instruction.
fn assert_generation_leg(generation: &Request) {
    let system = generation.system.as_deref().unwrap();
    assert!(system.contains("# Omnia target — build prompt"), "build prompt in system");
    assert!(
        !system.contains("# Omnia target — guidance prompt"),
        "guidance stays on the guidance operation — dropped from generation (RFC-78 D2)"
    );
    assert!(system.contains("# Omnia build — crate writer"), "crate writer prompt in system");
    assert!(system.contains("# Omnia build — test writer"), "test writer prompt in system");
    assert!(
        system.contains("# Omnia build — guest writer"),
        "guest writer prompt in system — create mode (no workspace-root src/lib.rs)"
    );
    let user = &generation.messages[0].content;
    assert!(
        user.contains("### input: proposal → .emery/slices/demo/proposal.md")
            && user.contains("### input: design → .emery/slices/demo/design.md")
            && user.contains("### input: spec → .emery/slices/demo/specs/core/spec.md"),
        "typed inputs render as path-form sections: {user}"
    );
    assert!(!user.contains("PROPOSAL-BODY"), "artifact bodies are not inlined");
    assert!(
        user.contains("Read each path from the working tree"),
        "read-before-writing instruction rides the inputs block"
    );
    assert!(
        user.contains("folded into the slice artifacts at refine"),
        "guidance pointer replaces the inlined guidance prompt"
    );
    assert!(user.contains("slice `demo`"), "slice named");
    assert!(user.contains("Verify-repair loop"), "agent-run cargo verification instructed");
    assert!(user.contains("omnia-references"), "user prompt points at the MCP references");
    assert!(user.contains("### scaffold prelude"), "scaffold prelude outcome in user prompt");
    assert!(user.contains("- `Makefile.toml`"), "written tooling files listed");
    assert!(
        user.contains("Unfilled placeholders still present"),
        "unfilled publish tokens always surfaced: {user}"
    );
}

#[tokio::test]
async fn build_phase_legs() {
    let tmp = TempDir::new().unwrap();
    // The scripted preparation leg writes nothing, so the checkout the
    // real agent would produce is synthesized up front — likewise the
    // crate tree the review answer declares as an output.
    common::write_checkout(tmp.path());
    std::fs::create_dir_all(tmp.path().join("crates/demo")).unwrap();
    let model = Harness::answering([PHASE_DONE, PHASE_DONE, REVIEW_DONE]);
    let inputs = vec![
        Input::Proposal(input(".emery/slices/demo/proposal.md")),
        Input::Spec(input(".emery/slices/demo/specs/core/spec.md")),
        Input::Design(input(".emery/slices/demo/design.md")),
    ];
    let context = BuildContext {
        sources: vec!["intent".to_string(), "typescript".to_string()],
    };

    let report = Adapter::build(
        &model,
        &ctx(tmp.path(), Some("http://references/mcp")),
        "demo",
        &inputs,
        &context,
        &tree(),
    )
    .await
    .unwrap();

    assert_eq!(report.status, Status::Success);
    assert!(report.findings.is_empty());
    assert_eq!(report.outputs.len(), 1, "review-declared outputs ride the assembled report");
    assert_eq!(report.outputs[0].platform, Platform::Core);
    assert_eq!(report.outputs[0].path, "crates/demo");

    let requests = model.requests();
    assert_eq!(
        requests.len(),
        3,
        "preparation, generation, review — no replay spawn without a captures binding, \
         the report is assembled in-guest (RFC-78 D6)"
    );
    // Budget = measured baseline (per-leg comment, 2026-07-31, post-WP3) + ~10%.
    // Generation dropped from 43_465 with RFC-78 D2: guidance.md left the
    // assemble and guest.md ships only in create mode (this test's tree).
    for (i, (leg, budget)) in [
        ("preparation", 19_300), // baseline 17_579
        ("generation", 37_000),  // baseline 33_610
        ("review", 24_800),      // baseline 22_550
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
    assert!(generation.lend_workspace);
    assert_eq!(mcp_grants(generation)[0].url, "http://references/mcp");

    let review = &requests[2];
    let (name, schema) = schema_format(review);
    assert_eq!(name, "review");
    let compiled = serde_json::from_str::<serde_json::Value>(schema).unwrap();
    assert!(jsonschema::validator_for(&compiled).is_ok(), "review answer schema compiles");
    for field in ["findings", "outputs"] {
        assert!(schema.contains(&format!("\"{field}\"")), "absorbed report residue: {field}");
    }
    assert!(review.system.as_deref().unwrap().contains("# Omnia build — standards review"));
    let review_user = &review.messages[0].content;
    assert!(review_user.contains("- preparation:"), "preparation outcome feeds the review leg");
    assert!(review_user.contains("- generation:"), "generation outcome feeds the review leg");
    assert!(
        review_user.contains("- replay: skipped in-guest"),
        "the deterministic replay skip is surfaced to the review leg: {review_user}"
    );
    assert!(review_user.contains("tasks.md"), "tasks close-out absorbed into review");
    assert!(
        review_user.contains("no separate report leg"),
        "review told it closes the build: {review_user}"
    );
}

// The replay leg spawns only when the engine-forwarded build context
// carries a `captures` source binding (RFC-78 D6); it runs before the
// review leg so replay failures reach the findings synthesis.
#[tokio::test]
async fn build_replay_leg_gated_on_captures_binding() {
    let tmp = TempDir::new().unwrap();
    common::write_checkout(tmp.path());
    let model = Harness::answering([
        PHASE_DONE,
        PHASE_DONE,
        r#"{"applicable":true,"summary":"replay suite passed"}"#,
        r#"{"applicable":true,"summary":"ok"}"#,
    ]);
    let context = BuildContext {
        sources: vec!["intent".to_string(), "captures".to_string()],
    };

    let report = Adapter::build(&model, &ctx(tmp.path(), None), "demo", &[], &context, &tree())
        .await
        .unwrap();

    assert_eq!(report.status, Status::Success);
    let requests = model.requests();
    assert_eq!(requests.len(), 4, "preparation, generation, replay, review");
    let replay = &requests[2];
    assert_eq!(schema_format(replay).0, "replay");
    assert!(replay.system.as_deref().unwrap().contains("# Omnia build — capture replay"));
    assert!(
        replay.messages[0].content.contains("binds the `captures` source"),
        "replay dispatch is deterministic — the leg is never asked to self-skip"
    );
    assert_system_budget(replay, "replay", 19_600); // baseline 17_790
    let review_user = &requests[3].messages[0].content;
    assert!(
        review_user.contains("- replay: applicable=true"),
        "replay outcome feeds the review leg: {review_user}"
    );
}

// The in-guest report assembly folds the review answer's diagnostics
// into seam findings; a blocking finding forces `failure` with no
// further model call.
#[tokio::test]
async fn build_report_from_review_findings() {
    let tmp = TempDir::new().unwrap();
    common::write_checkout(tmp.path());
    let model = Harness::answering([
        PHASE_DONE,
        PHASE_DONE,
        r#"{"applicable":true,"summary":"unresolved blocking findings","findings":[{"rule-id":"OMNIA-002","title":"Forbidden std API","severity":"critical","impact":"The wasm32 build breaks.","remediation":"Route through the provider trait."}]}"#,
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

    assert_eq!(report.status, Status::Failure, "a blocking review finding fails the build");
    let finding = &report.findings[0];
    assert_eq!(finding.rule_id.as_deref(), Some("OMNIA-002"));
    assert_eq!(finding.severity, Severity::Critical);
    assert_eq!(
        finding.detail,
        "Forbidden std API — The wasm32 build breaks.; remediation: Route through the provider trait."
    );
    assert_eq!(model.requests().len(), 3, "no report or repair leg after review");
}

// A declared-but-missing output fails the assembled report through the
// deterministic gate — no repair re-prompt replaces the old report leg's.
#[tokio::test]
async fn build_report_missing_output_fails() {
    let tmp = TempDir::new().unwrap();
    common::write_checkout(tmp.path());
    let model = Harness::answering([
        PHASE_DONE,
        PHASE_DONE,
        r#"{"applicable":true,"summary":"done","outputs":[{"platform":"core","path":"crates/ghost"}]}"#,
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

    assert_eq!(report.status, Status::Failure);
    assert_eq!(report.findings.len(), 1);
    assert_eq!(report.findings[0].severity, Severity::Important);
    assert!(report.findings[0].detail.contains("crates/ghost"), "missing output named");
    assert_eq!(model.requests().len(), 3, "the output gate is deterministic — no re-prompt");
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
    let model =
        Harness::answering([PHASE_DONE, PHASE_DONE, r#"{"applicable":true,"summary":"ok"}"#]);

    Adapter::build(&model, &ctx(tmp.path(), None), "demo", &[], &BuildContext::default(), &tree())
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
        &tree(),
    )
    .await
    .unwrap_err();

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
    let (name, schema) = schema_format(&requests[0]);
    assert_eq!(name, "report");
    assert_eq!(schema, REPORT_ANSWER_SCHEMA, "merge still answers with the vendored report schema");
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
