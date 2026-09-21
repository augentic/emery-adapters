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
            assert_eq!(metadata.kind, SourceKind::Behaviour, "metadata carries the probe's kind");
            let evidence =
                Caller.extract(ADAPTER, &value("")).await.expect("extract over an inline value");
            check_same(&maximal(), &evidence);
        }
        ["together"] => {
            let (one, other) = (workspace(), workspace());
            let (first, second) =
                futures::join!(Caller.extract(ADAPTER, &one), Caller.extract(ADAPTER, &other));
            check_evidence(&first.expect("the first extract, dispatched beside the second"));
            check_evidence(&second.expect("the second extract, dispatched beside the first"));
        }
        ["tracing", level] => {
            omnia_wasi_otel::set_baggage([(omnia_wasi_otel::LEVEL, *level)]);
            let evidence = Caller
                .extract(ADAPTER, &value("Ship the orders API with idempotent retries."))
                .await
                .expect("extract over an inline value");
            check_evidence(&evidence);
        }
        other => panic!(
            "no argument, `refused <code> [<value>]`, `echoed`, `together`, or `tracing \
             <level>`; got {other:?}"
        ),
    }
}
