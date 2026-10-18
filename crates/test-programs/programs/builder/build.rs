//! Guest driver exercising a target adapter's `metadata` and `run` exports.

#![cfg(target_arch = "wasm32")]

use emery_sdk::target::capability::{Target, Task};
use emery_sdk::target::{MergeStrategy, Outcome};
use test_programs::{ADAPTER, Caller, arguments, check_version, slice};

omnia_sdk::command!(scenario);

async fn scenario() {
    let metadata = Caller.metadata(ADAPTER);
    check_version(metadata.emery_version.as_deref());

    let arguments = arguments();
    match arguments.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        [] => {
            let outcome =
                Caller.run(ADAPTER, ".", &Task::Init).await.expect("init over the lent workspace");
            assert_eq!(outcome, Outcome::Initialised, "an init answers an initialised tree");

            let slice = slice();
            let task = Task::Build(slice.clone());
            let outcome =
                Caller.run(ADAPTER, ".", &task).await.expect("build over the lent workspace");
            let Outcome::Built(report) = outcome else {
                panic!("a build answers a report, not {outcome}");
            };
            let findings = report.findings(&slice);
            assert!(findings.is_empty(), "report gate findings:\n{}", findings.join("\n"));
            for path in &report.written {
                assert!(
                    std::fs::metadata(path).is_ok_and(|meta| meta.is_file()),
                    "`{path}` is reported written and the tree holds it"
                );
            }

            let outcome = Caller
                .run(ADAPTER, ".", &Task::Verify)
                .await
                .expect("verify over the lent workspace");
            let Outcome::Verified(verdict) = outcome else {
                panic!("a verify answers a verdict, not {outcome}");
            };
            let findings = verdict.findings();
            assert!(findings.is_empty(), "verdict gate findings:\n{}", findings.join("\n"));
            assert!(verdict.passed, "the scripted verdict passes");
        }
        ["metadata", rules @ ..] => {
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
        other => panic!("no argument or `metadata <glob>=<strategy>..`; got {other:?}"),
    }
}
