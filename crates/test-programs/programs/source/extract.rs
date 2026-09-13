//! The `emery:adapter/source` calls the engine makes: `metadata` answers
//! with a pin the version gate parses, then `extract` crosses the seam. With
//! no argument the adapter is expected to answer — over the lent workspace,
//! then over an inline value — with evidence that still passes the
//! contract's claim gate on the caller's side of the bindings, the host
//! scripting one model answer per `extract`. Given one argument, an Omnia
//! error code, `extract` over the lent workspace is expected to fail and
//! lift to that class — `bad_request` for an adapter refusing its input,
//! `bad_gateway` for its own failure.

#![cfg(target_arch = "wasm32")]

use emery_adapter::source::Source;
use test_programs::{ADAPTER, Caller, arguments, check_evidence, check_metadata, value, workspace};

omnia_guest::command!(scenario);

async fn scenario() {
    check_metadata(&Caller.metadata(ADAPTER));

    match arguments().as_slice() {
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
        [expected] => {
            let refusal = Caller
                .extract(ADAPTER, &workspace())
                .await
                .expect_err("extract over the lent workspace is refused");
            assert_eq!(
                refusal.code(),
                expected.as_str(),
                "extract refused with the wrong class: {}",
                refusal.description()
            );
        }
        arguments => panic!("at most one argument, the expected error code; got {arguments:?}"),
    }
}
