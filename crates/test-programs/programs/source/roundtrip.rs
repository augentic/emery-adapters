//! The `emery:adapter/source` round trip the engine runs: `metadata` answers
//! with a pin the version gate parses, and `extract` — over the lent
//! workspace, then over an inline value — returns evidence that still passes
//! the contract's claim gate on the caller's side of the bindings. The host
//! scripts one model answer per `extract`.

#![cfg(target_arch = "wasm32")]

use emery_adapter::source::Source;
use test_programs::{ADAPTER, Caller, check_evidence, check_metadata, value, workspace};

omnia_guest::command!(scenario);

async fn scenario() {
    check_metadata(&Caller.metadata(ADAPTER));

    let evidence =
        Caller.extract(ADAPTER, &workspace()).await.expect("extract over the lent workspace");
    check_evidence(&evidence);

    let evidence = Caller
        .extract(ADAPTER, &value("Ship the orders API with idempotent retries."))
        .await
        .expect("extract over an inline value");
    check_evidence(&evidence);
}
