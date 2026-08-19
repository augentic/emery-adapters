//! The grading kernel at its public surface: the CC-05 / CC-06
//! mechanical properties over published spec-format text.

use eval::grade::{self, Expect};
use eval::scorecard::{CaseResult, Outcome, Scorecard};

const EXPECT: Expect = Expect {
    subject_fragment: "order",
};

/// A well-formed two-block spec: one agreed row, one inline gap.
const GOOD: &str = "# Specification\n\n\
### Requirement: order.placement\n\n\
ID: REQ-001\nSources: [documentation]\nStatus: agreed\n\n\
An order carries at least one line item.\n\n\
### Requirement: order.placement acceptance criteria [unknown]\n\n\
ID: REQ-002\nSources: []\nStatus: unknown\n\n\
No source contributed an acceptance criterion.\n";

#[test]
fn well_formed_spec_passes() {
    assert_eq!(grade::spec(GOOD, &EXPECT), Vec::<String>::new());
}

#[test]
fn empty_spec_is_unreviewable() {
    let findings = grade::spec("# Specification\n", &EXPECT);
    assert_eq!(findings.len(), 1);
    assert!(findings[0].contains("no requirement blocks"), "{findings:?}");
}

#[test]
fn missing_subject_is_a_finding() {
    let expect = Expect {
        subject_fragment: "position",
    };
    let findings = grade::spec(GOOD, &expect);
    assert!(
        findings.iter().any(|finding| finding.contains("position")),
        "the missed estate is named: {findings:?}"
    );
}

#[test]
fn missing_provenance_is_a_finding() {
    let spec = "### Requirement: order.placement\n\nStatus: agreed\n\nBody.\n";
    let findings = grade::spec(spec, &EXPECT);
    assert!(findings.iter().any(|finding| finding.contains("`ID:`")), "{findings:?}");
    assert!(findings.iter().any(|finding| finding.contains("`Sources:`")), "{findings:?}");
}

/// CC-05: a gap or disagreement hidden from the heading is a finding,
/// in both directions.
#[test]
fn tag_status_mismatch_is_a_finding() {
    let hidden = "### Requirement: order.state\n\n\
                  ID: REQ-001\nSources: []\nStatus: unknown\n\nBody.\n";
    let findings = grade::spec(hidden, &EXPECT);
    assert!(findings.iter().any(|finding| finding.contains("[unknown]")), "{findings:?}");

    let untagged_status = "### Requirement: order.state [conflict]\n\n\
                           ID: REQ-001\nSources: [documentation]\nStatus: agreed\n\nBody.\n";
    let findings = grade::spec(untagged_status, &EXPECT);
    assert!(findings.iter().any(|finding| finding.contains("[conflict]")), "{findings:?}");
}

/// The scorecard's green line: every case passed and both measured
/// numbers meet their product.md targets; anything else is red.
#[test]
fn scorecard_green_line() {
    let pass = CaseResult {
        id: "orders-docs".to_string(),
        outcome: Outcome::Pass {
            generation: "cafe".to_string(),
        },
        secs: 120.0,
        ops_succeeded: 3,
        ops_failed: 0,
        fixture_sha: None,
    };
    let card = |cases: Vec<CaseResult>| Scorecard {
        date: "2026-08-19".to_string(),
        emery_sha: "e".to_string(),
        adapters_sha: "a".to_string(),
        cases,
        complete: true,
    };

    assert!(card(vec![pass.clone()]).green());
    assert!(!card(Vec::new()).green(), "an empty run proves nothing");

    let filtered = Scorecard {
        complete: false,
        ..card(vec![pass.clone()])
    };
    assert!(!filtered.green(), "a filtered run can never produce a green record");

    let slow = CaseResult {
        secs: eval::scorecard::TIME_TARGET_SECS + 1.0,
        ..pass.clone()
    };
    assert!(!card(vec![slow]).green(), "over the time target is red");

    let failed = CaseResult {
        outcome: Outcome::TypedFailure {
            error: "source-extract-failed".to_string(),
            exit_code: 1,
        },
        ops_succeeded: 0,
        ops_failed: 1,
        ..pass.clone()
    };
    let red = card(vec![pass, failed]);
    assert!(!red.green(), "a typed failure is a red scorecard, never graded around");
    let rendered = red.render();
    assert!(rendered.contains("- status: red"), "{rendered}");
    assert!(rendered.contains("source-extract-failed"), "{rendered}");
    assert!(rendered.contains("unconfirmed"), "unmeasured stays unconfirmed: {rendered}");
}
