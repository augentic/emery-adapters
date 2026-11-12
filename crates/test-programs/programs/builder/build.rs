//! Guest driver exercising a target adapter's `metadata` and `run` exports.

#![cfg(target_arch = "wasm32")]

use std::path::Path;

use emery_sdk::target::capability::{Target, Task};
use emery_sdk::target::{
    Change, Departure, DesignType, MergeRule, MergeStrategy, Outcome, PartReach, Report,
    Requirement, Scenario, SectionChange, Slice, SliceChange, Standing as RequirementStanding,
    TypeChange, Verdict,
};
use emery_sdk::{Standing, Trigger, standing};
use test_programs::{ADAPTER, Caller, WORKSPACE, arguments, check_version};

omnia_sdk::api::command::export!(scenario);

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
        ["serial"] => {
            assert_eq!(
                run(&Task::Init).await,
                Outcome::Initialised,
                "an init answers an initialised tree"
            );

            for slice in [first_serial_slice(), second_serial_slice()] {
                let outcome = run(&Task::Build(slice.clone())).await;
                let Outcome::Built(report) = outcome else {
                    panic!("a build answers a report, not {outcome}");
                };
                check_report(&report, &slice);
            }

            let outcome = run(&Task::Verify).await;
            let Outcome::Verified(verdict) = outcome else {
                panic!("a verify answers a verdict, not {outcome}");
            };
            check_verdict(&verdict);
        }
        ["scope", expected @ ..] => {
            let change = change();
            let outcome = run(&Task::Scope(change.clone())).await;
            let Outcome::Scoped(scoped) = outcome else {
                panic!("a scope answers scoped parts, not {outcome}");
            };
            let findings = change.findings(&scoped);
            assert!(findings.is_empty(), "scope gate findings:\n{}", findings.join("\n"));
            assert_eq!(scoped, expected, "the scoped parts the adapter answered");

            assert_eq!(
                run(&Task::Init).await,
                Outcome::Initialised,
                "an init answers an initialised tree"
            );

            let slice = changed_slice();
            let outcome = run(&Task::Build(slice.clone())).await;
            let Outcome::Built(report) = outcome else {
                panic!("a build answers a report, not {outcome}");
            };
            check_report(&report, &slice);
        }
        ["metadata", expected @ ..] => {
            let declared: Vec<String> = metadata
                .budget
                .map(|budget| format!("budget={budget}"))
                .into_iter()
                .chain(metadata.merge_rules.iter().map(format_rule))
                .collect();
            assert_eq!(
                declared, expected,
                "the budget and merge rules the metadata declares, in order"
            );
        }
        other => {
            panic!(
                "no argument, `serial`, `scope <part>..`, or `metadata [budget=<n>] \
                 <glob>=<strategy>..`; got {other:?}"
            )
        }
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
        id: "SLICE-001".into(),
        part: "PART-001".into(),
        name: "orders".into(),
        index: 1,
        count: 1,
        base: "0123456789abcdef0123456789abcdef01234567".into(),
        requirements: vec![Requirement {
            id: "REQ-001".into(),
            subject: "orders.create".into(),
            statement: vec!["Creates an order from at least one item in the body.".into()],
            scenarios: vec![Scenario {
                name: "An order is created".into(),
                given: vec!["an item".into()],
                when: "an order is placed with it".into(),
                then: "the order is answered".into(),
                and: Vec::new(),
            }],
            trigger: Some(Trigger::http("POST /orders").expect("valid route")),
        }],
        types: vec![DesignType {
            key: "orders.Order".into(),
            signature: "interface Order { id: string; items: string[] }".into(),
        }],
        brief: vec!["Create an order from its items.".into()],
        spec: "# Specification\n\n## Requirement: orders.create\n\nID: REQ-001\n\nPOST /orders \
               creates an order from at least one item in the body.\n"
            .into(),
        design: "# Design\n\n## Type: orders.Order\n\nAn id and its items.\n".into(),
        plan: "## Part: orders\n\nID: PART-001\nRequirements: REQ-001\nTypes: orders.Order\n\
               Depends on: none\n"
            .into(),
        change: None,
    }
}

fn change() -> Change {
    Change {
        built: "rev-001".into(),
        commit: "0123456789abcdef0123456789abcdef01234567".into(),
        parts: vec![
            PartReach {
                id: "PART-001".into(),
                name: "orders".into(),
                requirements: vec!["REQ-001".into()],
                reasons: vec!["requirements".into()],
            },
            PartReach {
                id: "PART-002".into(),
                name: "customers".into(),
                requirements: vec!["REQ-002".into()],
                reasons: Vec::new(),
            },
        ],
        retired: vec![Departure {
            requirement: Requirement {
                id: "REQ-003".into(),
                subject: "orders.cancel".into(),
                statement: vec!["Cancels an order.".into()],
                scenarios: Vec::new(),
                trigger: None,
            },
            moved_to: None,
        }],
        types: vec![TypeChange {
            key: "orders.Order".into(),
            before: Some("interface Order { id: string }".into()),
            after: Some("interface Order { id: string; items: string[] }".into()),
        }],
        sections: vec![SectionChange {
            kind: "apis".into(),
            before: Some("Inbound HTTP: POST /orders.".into()),
            after: Some("Inbound HTTP: POST /orders (v2).".into()),
        }],
    }
}

fn changed_slice() -> Slice {
    let mut slice = slice();
    slice.change = Some(SliceChange {
        built: "rev-001".into(),
        commit: "0123456789abcdef0123456789abcdef01234567".into(),
        standing: vec![RequirementStanding::Changed],
        prior: vec![Requirement {
            id: "REQ-001".into(),
            subject: "orders.create".into(),
            statement: vec!["Creates an order.".into()],
            scenarios: Vec::new(),
            trigger: Some(Trigger::http("POST /orders").expect("valid route")),
        }],
        departed: vec![Departure {
            requirement: Requirement {
                id: "REQ-003".into(),
                subject: "orders.cancel".into(),
                statement: vec!["Cancels an order.".into()],
                scenarios: Vec::new(),
                trigger: None,
            },
            moved_to: Some("PART-002".into()),
        }],
        built_before: Vec::new(),
        vanished: vec!["legacy".into()],
        types: vec![TypeChange {
            key: "orders.Order".into(),
            before: Some("interface Order { id: string }".into()),
            after: Some("interface Order { id: string; items: string[] }".into()),
        }],
        sections: vec![SectionChange {
            kind: "apis".into(),
            before: Some("Inbound HTTP: POST /orders.".into()),
            after: Some("Inbound HTTP: POST /orders (v2).".into()),
        }],
    });
    slice
}

fn first_serial_slice() -> Slice {
    let mut slice = slice();
    slice.count = 2;
    slice
}

fn second_serial_slice() -> Slice {
    Slice {
        id: "SLICE-002".into(),
        part: "PART-001".into(),
        name: "orders".into(),
        index: 2,
        count: 2,
        base: "0123456789abcdef0123456789abcdef01234567".into(),
        requirements: vec![Requirement {
            id: "REQ-002".into(),
            subject: "orders.read".into(),
            statement: vec!["Reads an order by its id.".into()],
            scenarios: vec![Scenario {
                name: "An order is read".into(),
                given: vec!["an order 42".into()],
                when: "GET /orders/42 is called".into(),
                then: "the order is answered".into(),
                and: Vec::new(),
            }],
            trigger: Some(Trigger::http("GET /orders/{id}").expect("valid route")),
        }],
        types: Vec::new(),
        brief: vec!["Create and read an order.".into()],
        spec: "# Specification\n\n## Requirement: orders.read\n\nID: REQ-002\n\nGET /orders/{id} \
               reads an order by its id.\n"
            .into(),
        design: "# Design\n\n## Type: orders.Order\n\nAn id and its items.\n".into(),
        plan: "## Part: orders\n\nID: PART-001\nRequirements: REQ-001, REQ-002\nTypes: \
               orders.Order\nDepends on: none\n"
            .into(),
        change: None,
    }
}
