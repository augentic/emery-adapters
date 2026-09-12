//! A refusal crosses the seam typed: `extract` over the lent workspace fails,
//! and the failure lifts on the caller's side to the error code the host
//! names as the program's one argument — `bad_request` for an adapter that
//! refuses its input (or evidence the SDK's gate never let through),
//! `bad_gateway` for an adapter's own failure.

#![cfg(target_arch = "wasm32")]

use emery_adapter::source::Source;
use test_programs::{ADAPTER, Caller, arguments, workspace};

omnia_guest::command!(scenario);

async fn scenario() {
    let [expected]: [String; 1] =
        arguments().try_into().expect("one argument: the expected error code");

    let refusal = Caller
        .extract(ADAPTER, &workspace())
        .await
        .expect_err("extract over the lent workspace is refused");
    assert_eq!(
        refusal.code(),
        expected,
        "extract refused with the wrong class: {}",
        refusal.description()
    );
}
