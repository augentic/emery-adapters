//! Verifies shared SDK behaviour and component conversion with fixture adapters.
//!
//! Each probe isolates one contract property:
//!
//! - `refusing` and `upstream` verify WIT error classification.
//! - `echo` verifies every record field survives conversion.
//! - `gated` verifies model requests, reference tools, workspace grants, and
//!   exhausted correction rounds.
//! - `fanout` verifies concurrent guest requests are pending together.

#![cfg(not(target_arch = "wasm32"))]

mod support;

use omnia_test::host::{ScriptedModel, scratch};
use serde_json::Value;
use support::{Barrier, Strict as _, run};

// Every probe program must have a matching test here.
test_programs::foreach_probe!();

/// An answer the claim gate accepts.
const EVIDENCE: &str = r#"{"claims":[
    {"kind":"requirement","id":"orders.create","statement":"POST /orders creates an order."}
]}"#;

/// A requirement without its `statement`.
const UNSTATED: &str = r#"{"claims":[{"kind":"requirement","id":"orders.create"}]}"#;

/// Runs the driver's `refused` mode against `probe`, expecting `code`.
///
/// The probe never reaches the model.
async fn refused_by(probe: &str, code: &str) {
    let model = run(probe, &scratch(), &["refused", code], ScriptedModel::default()).await;
    assert!(model.seen().is_empty(), "a probe never reaches the model");
}

// `bad_request!` lowers onto `invalid-request` and lifts back as `bad_request`.
#[tokio::test]
async fn probe_refusing() {
    refused_by(test_programs::PROBE_REFUSING, "bad_request").await;
}

// `bad_gateway!` lowers onto `internal` — every class but a refusal — and
// lifts back as `bad_gateway`.
#[tokio::test]
async fn probe_upstream() {
    refused_by(test_programs::PROBE_UPSTREAM, "bad_gateway").await;
}

// The driver compares what it lifted against the `maximal()` the probe
// answered, field for field.
#[tokio::test]
async fn probe_echo() {
    let model =
        run(test_programs::PROBE_ECHO, &scratch(), &["echoed"], ScriptedModel::default()).await;
    assert!(model.seen().is_empty(), "a probe never reaches the model");
}

// The SDK's side of the boundary, once under the runtime: the request, the
// reference tools answered from the corpus, the lend following the input,
// each candidate offered to the guest's `check`.
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
        assert!(
            turn.contains("bound to adapter `adapter`"),
            "the adapter id names the turn: {turn}"
        );
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

// The property the SDK's fan-out rests on, under the runtime: the two
// completions one `extract` issues together are both pending before either
// is answered. The barrier holds each until the other arrives, so a host
// that serialised a guest's completions fails inside its hold; the guard is
// permanent, and pins neither the brief nor which turn served which
// seam.
#[tokio::test]
async fn probe_fanout() {
    let model = Barrier::new(ScriptedModel::answering([EVIDENCE; 4]), 2);
    let model = run(test_programs::PROBE_FANOUT, &scratch(), &[], model).await;

    assert_eq!(
        model.script().seen().len(),
        4,
        "each extract opens one completion per seam, both pending at once"
    );
}

// The gate rejects the only candidate and the budget is spent: the
// correction names the finding and the failure crosses as `bad_request`.
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
