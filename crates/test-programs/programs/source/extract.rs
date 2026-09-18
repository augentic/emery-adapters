//! Drives one adapter through its metadata and extraction interface.
//!
//! Command-line arguments select the verification mode:
//!
//! - No arguments extract workspace and inline inputs, then recheck each
//!   response against the claim gate.
//! - `refused <code>` expects workspace extraction to return that Omnia error
//!   class. Adding `<value>` tests the same refusal for inline input.
//! - `echoed` expects `maximal()` field for field and verifies the adapter's
//!   declared source kind.
//! - `together` dispatches two workspace extractions at once, as the engine
//!   dispatches one per source, and rechecks both responses.

#![cfg(target_arch = "wasm32")]

use emery_sdk::{Source, SourceKind};
use test_programs::{
    ADAPTER, Caller, arguments, check_evidence, check_metadata, check_same, maximal, value,
    workspace,
};

omnia_sdk::command!(scenario);

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
        ["refused", expected, inline @ ..] => {
            let input = match inline {
                [] => workspace(),
                [text] => value(text),
                more => panic!("`refused <code>` takes one inline value at most; got {more:?}"),
            };
            let refusal = Caller.extract(ADAPTER, &input).await.expect_err("extract is refused");
            assert_eq!(
                refusal.code(),
                *expected,
                "extract refused with the wrong class: {}",
                refusal.description()
            );
        }
        ["echoed"] => {
            // The kind crosses the bindings once, on `metadata`: the echo
            // probe declares `Behaviour`.
            assert_eq!(metadata.kind, SourceKind::Behaviour, "metadata carries the probe's kind");
            let evidence =
                Caller.extract(ADAPTER, &value("")).await.expect("extract over an inline value");
            check_same(&maximal(), &evidence);
        }
        ["together"] => {
            // Two link dispatches from one caller, in flight at once, as the
            // engine issues one per source of a run.
            let (one, other) = (workspace(), workspace());
            let (first, second) =
                futures::join!(Caller.extract(ADAPTER, &one), Caller.extract(ADAPTER, &other));
            check_evidence(&first.expect("the first extract, dispatched beside the second"));
            check_evidence(&second.expect("the second extract, dispatched beside the first"));
        }
        other => panic!(
            "no argument, `refused <code> [<value>]`, `echoed`, or `together`; got {other:?}"
        ),
    }
}
