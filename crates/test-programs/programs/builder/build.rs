//! Drives one target adapter through the `target-adapter` world under the
//! omnia runtime and asserts what the host sees: the metadata's merge rules,
//! or the report of one build over the lent workspace held to the report
//! gate and to the tree, and the verdict of one verification held to its gate.

#![cfg(target_arch = "wasm32")]

use emery_sdk::target::{MergeStrategy, Target};
use test_programs::{ADAPTER, Caller, arguments, check_version, slice};

omnia_sdk::command!(scenario);

async fn scenario() {
    let metadata = Caller.metadata(ADAPTER);
    check_version(metadata.emery_version.as_deref());

    let arguments = arguments();
    match arguments.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        [] => {
            let slice = slice();
            let report =
                Caller.build(ADAPTER, &slice, ".").await.expect("build over the lent workspace");
            let findings = report.findings(&slice);
            assert!(findings.is_empty(), "report gate findings:\n{}", findings.join("\n"));
            for path in &report.written {
                assert!(
                    std::fs::metadata(path).is_ok_and(|meta| meta.is_file()),
                    "`{path}` is reported written and the tree holds it"
                );
            }

            let verdict =
                Caller.verify(ADAPTER, ".").await.expect("verify over the lent workspace");
            let findings = verdict.findings();
            assert!(findings.is_empty(), "verdict gate findings:\n{}", findings.join("\n"));
            assert!(verdict.passed, "the scripted verdict passes");
        }
        ["rules", rules @ ..] => {
            let declared: Vec<String> = metadata
                .merge_rules
                .iter()
                .map(|rule| {
                    let strategy = match rule.strategy {
                        MergeStrategy::Union => "union",
                        MergeStrategy::Ours => "ours",
                        MergeStrategy::Theirs => "theirs",
                    };
                    format!("{}={strategy}", rule.paths)
                })
                .collect();
            assert_eq!(declared, rules, "the merge rules the metadata declares, in order");
        }
        other => panic!("no argument or `rules <glob>=<strategy>..`; got {other:?}"),
    }
}
