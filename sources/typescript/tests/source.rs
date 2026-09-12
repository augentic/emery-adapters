//! End-to-end tests for the TypeScript component over
//! `emery:adapter/source`: every scenario runs the built component through
//! the omnia runtime, driven by a guest program from `crates/test-programs`
//! over the same seam the engine uses, against a scripted host-side model.
//! The driver asserts what crosses the boundary (and traps on failure); this
//! side asserts wire fidelity: the completions the component opened and the
//! exchanges it drove.

#![cfg(not(target_arch = "wasm32"))]

use omnia::ExitStatus;
use omnia_test::host::{Backends, Deployment, Scratch, ScriptedModel, scratch};
use omnia_wasi_model::WasiModel;
use serde_json::Value;

// Every source-seam program in `crates/test-programs` must have a matching
// test here; a new program without one fails to compile.
test_programs::foreach_source!();

/// The extraction prompt as compiled into the component.
const PROMPT: &str = include_str!("../prose/prompts/extract.md");

/// A behavioural evidence answer the claim gate accepts.
const EVIDENCE: &str = r#"{"authority":"behaviour","claims":[
    {"kind":"requirement","id":"greeting.greet","path":"src/index.ts#L1","statement":"greet answers the string hello."},
    {"kind":"type","path":"src/index.ts#L1","signature":"function greet(): string"}
]}"#;

/// A requirement without its `statement`: the one extra the gate demands.
const UNSTATED: &str =
    r#"{"authority":"behaviour","claims":[{"kind":"requirement","id":"greeting.greet"}]}"#;

/// The source tree the component is lent.
fn project() -> Scratch {
    let project = scratch();
    project.write("src/index.ts", "export function greet(): string { return 'hello'; }\n");
    project
}

/// Runs `program` as the caller against the built component, `project`
/// mounted read-only as `.`, requiring a clean exit and the script exactly
/// consumed; returns the model for its recordings.
async fn run_guest(
    program: &str, project: &Scratch, args: &[&str], model: ScriptedModel,
) -> ScriptedModel {
    let backends = Backends::defaults().await.model(model.clone());
    let status = Deployment::new()
        .link(["emery:adapter/source@0.1.0"])
        .guest("caller", program)
        .guest(test_programs::ADAPTER, test_programs::ADAPTER_TYPESCRIPT)
        .command("caller")
        .mount(project.mount(false))
        .args(args.iter().copied())
        .run_host::<WasiModel, _>(backends)
        .await
        .expect("the caller runs");
    assert_eq!(status, ExitStatus::SUCCESS, "the driver `{program}` failed");
    model.assert_exhausted();
    model
}

// `metadata` is effect-free and each `extract` is one completion — the
// workspace leg lends the tree, the inline leg lends nothing — whose system
// prompt is the embedded `prompts/extract.md`, whose declared tools are the
// reference tools, whose `read_doc` answers with that same embedded body
// across the tool streams, and whose candidate the component's `check`
// accepts over the same streams.
#[tokio::test]
async fn source_roundtrip() {
    let model = ScriptedModel::answering([EVIDENCE, EVIDENCE])
        .calling(0, [("read_doc", r#"{"path":"prompts/extract.md"}"#)]);
    let model = run_guest(test_programs::SOURCE_ROUNDTRIP, &project(), &[], model).await;

    let seen = model.seen();
    assert_eq!(seen.len(), 2, "metadata opens no completion; each extract opens one");
    for request in &seen {
        assert_eq!(request.system.as_deref(), Some(PROMPT), "the compiled-in prompt is the system");
        assert_eq!(request.tools, ["list_docs", "read_doc"], "the reference tools are declared");
        assert!(request.check, "the component judges each candidate over the check tool");
        assert!(request.messages[0].contains("source key `source`"), "{:?}", request.messages);
    }
    let lent = model.lent();
    assert!(lent[0].is_some(), "the workspace leg lends the tree");
    assert!(lent[1].is_none(), "the inline leg lends nothing");

    let exchanges = model.exchanges();
    assert_eq!(exchanges.len(), 3, "one driven tool call and a check, then a check");
    assert_eq!(exchanges[0].tool, "read_doc");
    let answer: Value =
        serde_json::from_str(exchanges[0].outcome.as_ref().expect("read_doc answered"))
            .expect("a JSON answer");
    assert_eq!(answer["path"], "prompts/extract.md");
    assert_eq!(answer["body"], PROMPT, "the embedded document body crosses the seam");
    for check in &exchanges[1..] {
        assert_eq!(check.tool, "check");
        assert_eq!(check.outcome, Ok(String::new()), "the candidate passed the claim gate");
    }
}

// The SDK's gate rejects the only candidate and the backend's budget is
// spent on it: the correction carries the finding, and the failure crosses
// the seam as `bad_request` — the class the engine reads as a refusal.
#[tokio::test]
async fn source_refused() {
    let model = ScriptedModel::answering([UNSTATED]);
    let model = run_guest(test_programs::SOURCE_REFUSED, &project(), &["bad_request"], model).await;

    assert_eq!(model.seen().len(), 1);
    let exchanges = model.exchanges();
    assert_eq!(exchanges.len(), 1, "the one candidate was offered to the check");
    assert_eq!(exchanges[0].tool, "check");
    let correction = exchanges[0].outcome.clone().expect_err("the candidate is rejected");
    assert!(correction.contains("is missing extra `statement`"), "{correction}");
}
