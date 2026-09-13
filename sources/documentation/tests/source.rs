//! End-to-end test of the documentation component over
//! `emery:adapter/source`: the built component runs through the omnia
//! runtime, driven by the `source_extract` program from
//! `crates/test-programs` over the same seam the engine uses, against a
//! scripted host-side model. The driver asserts what crosses the boundary
//! (and traps on failure); this side asserts what only the host sees: the
//! completions the component opened and the exchanges it drove. The
//! adapter's own behaviour is `tests/extract.rs`'s.

#![cfg(not(target_arch = "wasm32"))]

use omnia::ExitStatus;
use omnia_test::host::{Backends, Deployment, ScriptedModel, scratch};
use omnia_wasi_model::WasiModel;
use serde_json::Value;

// Every source-seam program in `crates/test-programs` must have a matching
// test here; a new program without one fails to compile.
test_programs::foreach_source!();

/// The extraction prompt as compiled into the component.
const PROMPT: &str = include_str!("../prose/prompts/extract.md");

/// A documentation evidence answer the claim gate accepts.
const EVIDENCE: &str = r#"{"authority":"documentation","claims":[
    {"kind":"requirement","id":"orders.create","path":"docs/orders.md#L3","statement":"POST /orders creates an order."},
    {"kind":"criterion","id":"orders.create.status","path":"docs/orders.md#L3","criterion":"A created order answers 201."}
]}"#;

// `metadata` opens no completion and each `extract` opens one whose system
// prompt is the compiled-in `prompts/extract.md`. The workspace leg lends the
// documentation tree for the prompt to walk; the inline leg carries the
// driver's value across the `Value` arm and lends nothing. `read_doc`
// answers with the embedded body across the tool streams, and the
// component's `check` accepts each candidate over the same streams.
#[tokio::test]
async fn source_extract() {
    let project = scratch();
    project.write("docs/orders.md", "# Orders\n\nPOST /orders creates an order.\n");
    let model = ScriptedModel::answering([EVIDENCE, EVIDENCE])
        .calling(0, [("read_doc", r#"{"path":"prompts/extract.md"}"#)]);
    let backends = Backends::defaults().await.model(model.clone());
    let status = Deployment::new()
        .link(["emery:adapter/source@0.1.0"])
        .guest("caller", test_programs::SOURCE_EXTRACT)
        .guest(test_programs::ADAPTER, test_programs::ADAPTER_DOCUMENTATION)
        .command("caller")
        .mount(project.mount(false))
        .run_host::<WasiModel, _>(backends)
        .await
        .expect("the caller runs");
    assert_eq!(status, ExitStatus::SUCCESS, "the driver's checks failed");
    model.assert_exhausted();

    let seen = model.seen();
    assert_eq!(seen.len(), 2, "metadata opens no completion; each extract opens one");
    for request in &seen {
        assert_eq!(request.system.as_deref(), Some(PROMPT), "the compiled-in prompt is the system");
    }
    assert!(
        seen[1].messages[0].contains("Ship the orders API"),
        "the inline value crossed the seam: {:?}",
        seen[1].messages
    );
    let lent = model.lent();
    assert!(lent[0].is_some(), "the workspace leg lends the tree");
    assert!(lent[1].is_none(), "the inline leg lends nothing");

    let exchanges = model.exchanges();
    assert_eq!(exchanges.len(), 3, "one driven tool call and a check, then a check");
    assert_eq!(exchanges[0].tool, "read_doc");
    let answer: Value =
        serde_json::from_str(exchanges[0].outcome.as_ref().expect("read_doc answered"))
            .expect("a JSON answer");
    assert_eq!(answer["body"], PROMPT, "the embedded document body crosses the seam");
    for check in &exchanges[1..] {
        assert_eq!(check.tool, "check");
        assert_eq!(check.outcome, Ok(String::new()), "the candidate passed the claim gate");
    }
}
