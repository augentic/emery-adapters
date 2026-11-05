//! Guest driver exercising a source adapter's `metadata` and `extract` exports.

#![cfg(target_arch = "wasm32")]

use emery_sdk::{Evidence, Source, SourceInput, SourceKind};
use test_programs::{ADAPTER, Caller, WORKSPACE, arguments, check_version, maximal};

const SOURCE: &str = "source";

omnia_sdk::command!(scenario);

async fn scenario() {
    let metadata = Caller.metadata(ADAPTER);
    check_version(metadata.emery_version.as_deref());

    let args = arguments();
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        [] => {
            extract(&workspace()).await;
            extract(&value("Ship the orders API with idempotent retries.")).await;
        }
        ["workspace"] => {
            extract(&workspace()).await;
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
            let evidence = extract(&value("")).await;
            assert_eq!(evidence, maximal(), "evidence survives the WIT round-trip intact");
        }
        ["together"] => {
            let input = workspace();
            futures::join!(extract(&input), extract(&input));
        }
        other => {
            panic!(
                "no argument, `workspace`, `refused <code> [<value>]`, `echoed`, or `together`; got \
                 {other:?}"
            )
        }
    }
}

fn workspace() -> SourceInput {
    SourceInput::workspace(SOURCE, WORKSPACE)
}

fn value(text: &str) -> SourceInput {
    SourceInput::value(SOURCE, text)
}

async fn extract(input: &SourceInput) -> Evidence {
    let evidence = Caller
        .extract(ADAPTER, input)
        .await
        .unwrap_or_else(|err| panic!("extract for `{}` failed: {err}", input.name));
    check_evidence(&evidence);
    evidence
}

fn check_evidence(evidence: &Evidence) {
    assert!(!evidence.claims.is_empty(), "evidence carries no claims");
    let findings = evidence.findings();
    assert!(findings.is_empty(), "claim gate findings:\n{}", findings.join("\n"));
}
