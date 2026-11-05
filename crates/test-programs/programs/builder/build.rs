//! Guest driver exercising a target adapter's `metadata` and `run` exports.

#![cfg(target_arch = "wasm32")]

use std::path::Path;

use emery_sdk::target::capability::{Target, Task};
use emery_sdk::target::{
    DesignType, MergeRule, MergeStrategy, Outcome, Report, Requirement, Scenario, Slice, Verdict,
};
use emery_sdk::{Standing, Trigger, standing};
use test_programs::{ADAPTER, Caller, WORKSPACE, arguments, check_version};

omnia_sdk::command!(scenario);

async fn scenario() {
    let metadata = Caller.metadata(ADAPTER);
    check_version(metadata.emery_version.as_deref());

    let args = arguments();
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        [] => {
            assert_eq!(
                run(&Task::Init).await,
                Outcome::Initialised,
                "an init answers an initialised tree"
            );

            let slice = slice();
            let outcome = run(&Task::Build(slice.clone())).await;
            let Outcome::Built(report) = outcome else {
                panic!("a build answers a report, not {outcome}");
            };
            check_report(&report, &slice);

            let outcome = run(&Task::Verify).await;
            let Outcome::Verified(verdict) = outcome else {
                panic!("a verify answers a verdict, not {outcome}");
            };
            check_verdict(&verdict);
        }
        ["metadata", rules @ ..] => {
            let declared: Vec<String> = metadata.merge_rules.iter().map(format_rule).collect();
            assert_eq!(declared, rules, "the merge rules the metadata declares, in order");
        }
        ["init"] => {
            assert_eq!(
                run(&Task::Init).await,
                Outcome::Initialised,
                "an init answers an initialised tree"
            );
        }
        ["refused", expected, fragment] => {
            let refusal = Caller
                .run(ADAPTER, WORKSPACE, &Task::Build(slice()))
                .await
                .expect_err("the build of the slice is refused");
            assert_eq!(
                refusal.code(),
                *expected,
                "the build refused with the wrong class: {}",
                refusal.description()
            );
            assert!(
                refusal.description().contains(fragment),
                "the refusal names `{fragment}`: {}",
                refusal.description()
            );
        }
        other => panic!(
            "no argument, `metadata <glob>=<strategy>..`, `init`, or `refused <code> <fragment>`; got {other:?}"
        ),
    }
}

async fn run(task: &Task) -> Outcome {
    Caller
        .run(ADAPTER, WORKSPACE, task)
        .await
        .unwrap_or_else(|err| panic!("{task} over the lent workspace failed: {err}"))
}

fn check_report(report: &Report, slice: &Slice) {
    let findings = report.findings(slice);
    assert!(findings.is_empty(), "report gate findings:\n{}", findings.join("\n"));
    for path in &report.written {
        assert_eq!(
            standing(Path::new(WORKSPACE), path).ok(),
            Some(Standing::File),
            "`{path}` is reported written and the tree holds it"
        );
    }
}

fn check_verdict(verdict: &Verdict) {
    let findings = verdict.findings();
    assert!(findings.is_empty(), "verdict gate findings:\n{}", findings.join("\n"));
    assert!(verdict.passed, "the scripted verdict passes");
}

fn format_rule(rule: &MergeRule) -> String {
    let strategy = match rule.strategy {
        MergeStrategy::Union => "union",
        MergeStrategy::Ours => "ours",
        MergeStrategy::Theirs => "theirs",
    };
    format!("{}={strategy}", rule.paths)
}

// One requirement under the `orders` stem, served at a typed route its
// statement does not spell.
fn slice() -> Slice {
    Slice {
        id: "SLICE-001".to_owned(),
        name: "orders".to_owned(),
        base: "0123456789abcdef0123456789abcdef01234567".to_owned(),
        requirements: vec![Requirement {
            id: "REQ-001".to_owned(),
            subject: "orders.create".to_owned(),
            statement: vec!["Creates an order from at least one item.".to_owned()],
            scenarios: vec![Scenario {
                name: "An order is created".to_owned(),
                given: vec!["an item".to_owned()],
                when: "an order is placed with it".to_owned(),
                then: "the order is answered".to_owned(),
                and: Vec::new(),
            }],
            trigger: Some(Trigger::http("POST /orders").expect("valid route")),
        }],
        types: vec![DesignType {
            key: "orders.Order".to_owned(),
            signature: "interface Order { id: string; items: string[] }".to_owned(),
        }],
        brief: vec!["Create an order from its items.".to_owned()],
        spec: "# Specification\n\n## Requirement: orders.create\n\nID: REQ-001\n\nPOST /orders \
               creates an order from at least one item.\n"
            .to_owned(),
        design: "# Design\n\n## Type: orders.Order\n\nAn id and its items.\n".to_owned(),
        plan: "## Slice: orders\n\nID: SLICE-001\nRequirements: REQ-001\nTypes: orders.Order\n\
               Depends on: none\n"
            .to_owned(),
        layouts: Vec::new(),
    }
}
