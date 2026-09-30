#![cfg(target_arch = "wasm32")]

use emery_sdk::{ClaimKind, Evidence, Source, SourceKind};
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
        ["types", names @ ..] => {
            let evidence = Caller
                .extract(ADAPTER, &workspace())
                .await
                .expect("extract over the lent workspace");
            check_evidence(&evidence);
            let mut expected: Vec<&str> = names.to_vec();
            expected.sort_unstable();
            assert_eq!(types(&evidence), expected, "the workspace's declarations, and no other");
            for claim in evidence.claims.iter().filter(|claim| claim.kind == ClaimKind::Type) {
                let path = claim.path.as_deref().expect("a declaration of the tree is anchored");
                assert!(path.contains("#L"), "anchored at its lines: {path}");
            }

            let evidence = Caller
                .extract(
                    ADAPTER,
                    &value(
                        "export interface Inline { id: string }\ninterface Local { n: number }\n",
                    ),
                )
                .await
                .expect("extract over an inline value");
            check_evidence(&evidence);
            assert_eq!(
                types(&evidence),
                ["Inline"],
                "the value's exported declaration, and no other"
            );
            let inline = evidence.claims.iter().find(|claim| claim.kind == ClaimKind::Type);
            assert_eq!(inline.and_then(|claim| claim.path.clone()), None, "a value has no file");
        }
        other => {
            panic!(
                "no argument, `refused <code> [<value>]`, `echoed`, `together`, or `types \
                 <name>..`; got {other:?}"
            )
        }
    }
}

// The `type` claims' declared names, sorted; every one carries a signature.
fn types(evidence: &Evidence) -> Vec<&str> {
    let mut names: Vec<&str> = evidence
        .claims
        .iter()
        .filter(|claim| claim.kind == ClaimKind::Type)
        .map(|claim| {
            let signature = claim.extras.get("signature").and_then(|s| s.as_str());
            assert!(signature.is_some_and(|s| !s.is_empty()), "a declaration carries its text");
            claim.extras.get("name").and_then(|n| n.as_str()).expect("a declaration is named")
        })
        .collect();
    names.sort_unstable();
    names
}
