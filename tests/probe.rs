//! The seam proved over the fixture adapters under
//! `crates/test-programs/programs/probe/`, each in place of a shipped one
//! and driven by the `source_extract` program on the caller's side of
//! `emery:adapter/source`: the WIT `error` arms lifting back to their Omnia
//! classes (`refusing`, `upstream`); every field of the contract's records
//! surviving the bindings' lowering and lift (`echo`); and what the SDK does
//! for every adapter under the runtime — the request it opens, the reference
//! tools, the lend, the claim gate's refusal once the backend's budget is
//! spent (`gated`) — asserted here once rather than per shipped component.
//! Every scenario runs the real components through the omnia runtime.

#![cfg(not(target_arch = "wasm32"))]

mod support;

use omnia_test::host::{ScriptedModel, scratch};
use serde_json::Value;
use support::run;

// Every probe program in `crates/test-programs` must have a matching test
// here; a new probe without one fails to compile.
test_programs::foreach_probe!();

/// An answer the gated probe's claim gate accepts.
const EVIDENCE: &str = r#"{"authority":"documentation","claims":[
    {"kind":"requirement","id":"orders.create","statement":"POST /orders creates an order."}
]}"#;

/// A requirement without its `statement`: the one extra the gate demands.
const UNSTATED: &str =
    r#"{"authority":"documentation","claims":[{"kind":"requirement","id":"orders.create"}]}"#;

/// Runs the driver's `refused` mode against `probe`, requiring the failure
/// to cross the seam as `code`; nothing is scripted because the probe never
/// reaches the model.
async fn refused_by(probe: &str, code: &str) {
    let model = run(probe, &scratch(), &["refused", code], ScriptedModel::default()).await;
    assert!(model.seen().is_empty(), "a probe never reaches the model");
}

// `bad_request!` lowers onto the WIT `invalid-request` arm and lifts back as
// `bad_request`: the class the engine reads as an adapter refusing its input.
#[tokio::test]
async fn probe_refusing() {
    refused_by(test_programs::PROBE_REFUSING, "bad_request").await;
}

// `bad_gateway!` lowers onto the `internal` arm — every class but a refusal
// shares it — and lifts back as `bad_gateway`: an adapter's own failure.
#[tokio::test]
async fn probe_upstream() {
    refused_by(test_programs::PROBE_UPSTREAM, "bad_gateway").await;
}

// Every field of the contract's records — each claim kind, both `backing`
// arms, every anchor form, extras that are objects, numbers, lists, and
// null — crosses the bindings as it left the adapter: the driver compares
// what it lifted against the same `maximal()` the probe answered.
#[tokio::test]
async fn probe_echo() {
    let model =
        run(test_programs::PROBE_ECHO, &scratch(), &["echoed"], ScriptedModel::default()).await;
    assert!(model.seen().is_empty(), "a probe never reaches the model");
}

// What the SDK does for every adapter, proved once under the runtime: each
// `extract` opens one completion (`metadata` none) whose system is the
// embedded `prompts/extract.md`, whose tools are the reference tools, whose
// `check` the backend loops on, and whose turn names the source and key; the
// workspace leg lends the tree and the inline leg lends nothing, its value
// riding the turn; `list_docs` and `read_doc` answer from the embedded corpus
// across the tool streams; and each candidate is offered to the guest's
// `check` before it is accepted.
#[tokio::test]
async fn probe_gated() {
    let model = ScriptedModel::answering([EVIDENCE, EVIDENCE])
        .calling(0, [("list_docs", "{}"), ("read_doc", r#"{"path":"references/greeting.md"}"#)]);
    let model = run(test_programs::PROBE_GATED, &scratch(), &[], model).await;

    let seen = model.seen();
    assert_eq!(seen.len(), 2, "metadata opens no completion; each extract opens one");
    for request in &seen {
        assert_eq!(request.system.as_deref(), Some("SYSTEM"), "the embedded prompt is the system");
        assert_eq!(request.tools, ["list_docs", "read_doc"], "the reference tools are declared");
        assert!(request.check, "each candidate is offered to the guest's check");
        let turn = &request.messages[0];
        assert!(turn.contains("the gated probe source"), "the source noun names the turn: {turn}");
        assert!(turn.contains("(source key `source`)"), "the key names the turn: {turn}");
    }
    assert!(
        seen[1].messages[0].contains("Ship the orders API"),
        "the inline value rides the turn: {:?}",
        seen[1].messages
    );
    let lent = model.lent();
    assert!(lent[0].is_some(), "the workspace leg lends the tree");
    assert!(lent[1].is_none(), "the inline leg lends nothing");

    let exchanges = model.exchanges();
    assert_eq!(exchanges.len(), 4, "two reference calls and a check, then a check");
    assert_eq!(exchanges[0].tool, "list_docs");
    let listed: Value =
        serde_json::from_str(exchanges[0].outcome.as_ref().expect("list_docs answered"))
            .expect("a JSON answer");
    assert_eq!(
        listed["paths"],
        serde_json::json!(["prompts/extract.md", "references/greeting.md"])
    );
    assert_eq!(exchanges[1].tool, "read_doc");
    let read: Value =
        serde_json::from_str(exchanges[1].outcome.as_ref().expect("read_doc answered"))
            .expect("a JSON answer");
    assert_eq!(read["path"], "references/greeting.md");
    assert_eq!(read["body"], "Greet warmly.", "the embedded body crosses the tool stream");
    for check in &exchanges[2..] {
        assert_eq!(check.tool, "check");
        assert_eq!(check.outcome, Ok(String::new()), "the candidate passed the claim gate");
    }
}

// The claim gate rejects the only candidate and the backend's budget is
// spent on it: the correction carries the finding, and the failure crosses
// the seam as `bad_request` — the class the engine reads as a refusal.
#[tokio::test]
async fn gated_spent() {
    let model = ScriptedModel::answering([UNSTATED]);
    let model =
        run(test_programs::PROBE_GATED, &scratch(), &["refused", "bad_request"], model).await;

    assert_eq!(model.seen().len(), 1, "the one candidate consumed the one turn");
    let exchanges = model.exchanges();
    assert_eq!(exchanges.len(), 1, "the one candidate was offered to the check");
    assert_eq!(exchanges[0].tool, "check");
    let correction = exchanges[0].outcome.clone().expect_err("the candidate is rejected");
    assert!(correction.contains("is missing extra `statement`"), "{correction}");
}
