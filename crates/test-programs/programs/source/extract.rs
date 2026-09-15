//! Drives one adapter over `emery:adapter/source`: `metadata`, then `extract`.
//!
//! The host names the mode as arguments:
//!
//! - none: `extract` over both `SourceInput` arms, each answer re-checked
//!   against the claim gate;
//! - `refused <code>`: the failure lifts to that omnia class;
//! - `echoed`: the answer is `maximal()` field for field, and the metadata
//!   carries the probe's kind.

#![cfg(target_arch = "wasm32")]

use emery_sdk::{Source, SourceKind};
use test_programs::{
    ADAPTER, Caller, arguments, check_evidence, check_metadata, check_same, maximal, value,
    workspace,
};

omnia_guest::command!(scenario);

async fn scenario() {
    let metadata = Caller.metadata(ADAPTER);
    check_metadata(&metadata);

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
            // The kind crosses the bindings once, on `metadata`: the echo
            // probe's constant is `Behaviour`.
            assert_eq!(metadata.kind, SourceKind::Behaviour, "metadata carries the probe's kind");
            let evidence =
                Caller.extract(ADAPTER, &value("")).await.expect("extract over an inline value");
            check_same(&maximal(), &evidence);
        }
        other => panic!("no argument, `refused <code>`, or `echoed`; got {other:?}"),
    }
}
