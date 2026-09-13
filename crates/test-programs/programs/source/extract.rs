//! The `emery:adapter/source` calls the engine makes: `metadata` answers
//! with a pin the version gate parses, then `extract` crosses the seam in
//! the mode the host names as the program's arguments.
//!
//! - none — the adapter answers, over the lent workspace then over an inline
//!   value, with evidence that still passes the contract's claim gate on the
//!   caller's side of the bindings; the host scripts one model answer per
//!   `extract`.
//! - `refused <code>` — `extract` over the lent workspace fails and lifts to
//!   the Omnia error class `<code>`: `bad_request` for an adapter refusing
//!   its input (or evidence the SDK's gate never let through), `bad_gateway`
//!   for its own failure.
//! - `echoed` — `extract` over an inline value answers the maximal evidence
//!   the `echo` probe returns, every field as it left the adapter: what the
//!   WIT bindings' lowering and lift conserve.

#![cfg(target_arch = "wasm32")]

use emery_adapter::source::Source;
use test_programs::{
    ADAPTER, Caller, arguments, check_evidence, check_metadata, check_same, maximal, value,
    workspace,
};

omnia_guest::command!(scenario);

async fn scenario() {
    check_metadata(&Caller.metadata(ADAPTER));

    let arguments = arguments();
    match arguments.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        [] => {
            let evidence = Caller
                .extract(ADAPTER, &workspace())
                .await
                .expect("extract over the lent workspace");
            check_evidence(&evidence);

            let evidence = Caller
                .extract(ADAPTER, &value("Ship the orders API with idempotent retries."))
                .await
                .expect("extract over an inline value");
            check_evidence(&evidence);
        }
        ["refused", expected] => {
            let refusal = Caller
                .extract(ADAPTER, &workspace())
                .await
                .expect_err("extract over the lent workspace is refused");
            assert_eq!(
                refusal.code(),
                *expected,
                "extract refused with the wrong class: {}",
                refusal.description()
            );
        }
        ["echoed"] => {
            let evidence =
                Caller.extract(ADAPTER, &value("")).await.expect("extract over an inline value");
            check_same(&maximal(), &evidence);
        }
        other => panic!("no argument, `refused <code>`, or `echoed`; got {other:?}"),
    }
}
