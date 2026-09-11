//! The grading kernel at its public surface: the mechanical
//! properties over the published wire shapes — the typed specification and
//! its Markdown projection as `emery show --format json` carries them.

use eval::grade::{self, Expect, Spec};
use eval::scorecard::{CaseResult, Outcome, Scorecard};

const EXPECT: Expect = Expect {
    subject_fragment: "order",
};

// A well-formed two-requirement specification: one agreed row, one gap.
const GOOD: &str = r#"{
  "emery": 2,
  "next_id": 3,
  "preamble": [],
  "requirements": [
    {"id": "REQ-001", "subject": "order.placement", "status": "agreed", "covered": true,
     "sources": [{"source": "documentation", "claim": "order.placement"}],
     "body": ["An order carries at least one line item."], "losers": [], "scenarios": []},
    {"id": "REQ-002", "subject": "order.state", "status": "unknown", "covered": false,
     "sources": [{"source": "documentation", "claim": "order.state"}],
     "body": ["An order is open, fulfilled, or cancelled."], "losers": [], "scenarios": []}
  ]
}"#;

// The projection of `GOOD`: the gap is tagged in place.
const GOOD_BODY: &str = "---\nemery: 2\nrevision: cafe\n---\n\n# Specification\n\n\
### Requirement: order.placement\n\n\
ID: REQ-001\nSources: [documentation:order.placement]\nStatus: agreed\n\n\
An order carries at least one line item.\n\n\
### Requirement: order.state [unknown]\n\n\
ID: REQ-002\nSources: [documentation:order.state]\nStatus: unknown\n\n\
An order is open, fulfilled, or cancelled.\n\nNote: acceptance criteria not evidenced.\n";

fn spec_of(json: &str) -> Spec {
    serde_json::from_str(json).expect("a typed spec fixture")
}

// One requirement whose graded fields are chosen by the scenario.
fn requirement(id: &str, subject: &str, status: &str, sources: &serde_json::Value) -> String {
    serde_json::json!({
        "requirements": [{
            "id": id,
            "subject": subject,
            "status": status,
            "sources": sources,
        }],
    })
    .to_string()
}

#[test]
fn well_formed() {
    assert_eq!(grade::spec(&spec_of(GOOD), GOOD_BODY, &EXPECT), Vec::<String>::new());
}

#[test]
fn empty_spec() {
    let findings = grade::spec(&spec_of(r#"{"requirements": []}"#), "# Specification\n", &EXPECT);
    assert_eq!(findings.len(), 1);
    assert!(findings[0].contains("no requirements"), "{findings:?}");
}

#[test]
fn missing_subject() {
    let expect = Expect {
        subject_fragment: "position",
    };
    let findings = grade::spec(&spec_of(GOOD), GOOD_BODY, &expect);
    assert!(
        findings.iter().any(|finding| finding.contains("position")),
        "the missed estate is named: {findings:?}"
    );
}

#[test]
fn no_provenance() {
    let spec = requirement("REQ-001", "order.placement", "agreed", &serde_json::json!([]));
    let findings = grade::spec(&spec_of(&spec), GOOD_BODY, &EXPECT);
    assert!(findings.iter().any(|finding| finding.contains("cites no source")), "{findings:?}");

    let half = requirement(
        "REQ-001",
        "order.placement",
        "agreed",
        &serde_json::json!([{"source": "documentation", "claim": ""}]),
    );
    let findings = grade::spec(&spec_of(&half), GOOD_BODY, &EXPECT);
    assert!(findings.iter().any(|finding| finding.contains("incomplete pair")), "{findings:?}");
}

// Identity is stored: malformed or duplicate `REQ-NNN` ids are findings.
#[test]
fn identity() {
    for malformed in ["", "REQ- 001", "REQ-000", "REQ-0001"] {
        let spec = requirement(malformed, "order.placement", "agreed", &serde_json::json!([]));
        let findings = grade::spec(&spec_of(&spec), GOOD_BODY, &EXPECT);
        assert!(
            findings.iter().any(|finding| finding.contains("not a `REQ-NNN` id")),
            "`{malformed}` was accepted: {findings:?}"
        );
    }

    let duplicated = GOOD.replace("REQ-002", "REQ-001");
    let findings = grade::spec(&spec_of(&duplicated), GOOD_BODY, &EXPECT);
    assert!(
        findings.iter().any(|finding| finding.contains("more than one requirement")),
        "{findings:?}"
    );
}

// A gap or disagreement hidden from the projected heading is a
// finding, in both directions; a requirement the projection never
// heads is one too.
#[test]
fn tag_mismatch() {
    let hidden = GOOD_BODY.replace(" [unknown]", "");
    let findings = grade::spec(&spec_of(GOOD), &hidden, &EXPECT);
    assert!(findings.iter().any(|finding| finding.contains("[unknown]")), "{findings:?}");

    let untagged_status = GOOD.replace(r#""status": "unknown""#, r#""status": "agreed""#);
    let findings = grade::spec(&spec_of(&untagged_status), GOOD_BODY, &EXPECT);
    assert!(findings.iter().any(|finding| finding.contains("[unknown]")), "{findings:?}");

    let unprojected = GOOD_BODY.replace("order.state", "order.status");
    let findings = grade::spec(&spec_of(GOOD), &unprojected, &EXPECT);
    assert!(
        findings.iter().any(|finding| finding.contains("no heading in the projection")),
        "{findings:?}"
    );
}

// The scorecard's green line: every case passed and both measured
// numbers meet their product.md targets; anything else is red.
#[test]
fn green_line() {
    let pass = CaseResult {
        id: "orders-docs".to_string(),
        outcome: Outcome::Pass {
            revision: "cafe".to_string(),
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
            error: "bad_gateway".to_string(),
            exit_code: 4,
        },
        ops_succeeded: 0,
        ops_failed: 1,
        ..pass.clone()
    };
    let red = card(vec![pass, failed]);
    assert!(!red.green(), "a typed failure is a red scorecard, never graded around");
    let rendered = red.to_string();
    assert!(rendered.contains("- status: red"), "{rendered}");
    assert!(rendered.contains("bad_gateway"), "{rendered}");
    assert!(rendered.contains("unconfirmed"), "unmeasured stays unconfirmed: {rendered}");
}
