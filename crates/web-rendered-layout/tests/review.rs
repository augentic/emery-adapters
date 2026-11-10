//! What a review draws and says, over small hand-built pages.

mod support;

use support::{BLUE, Card, RED, facts};
use web_rendered_layout::{Width, review};

#[test]
fn passing_review_says_nothing() {
    let original = facts(&[RED, BLUE]);
    let passed = review(&original, &original).expect("the ranges overlap");

    assert!(passed.comparison.pass);
    assert_eq!(passed.findings(|width| format!("overlay-{}.svg", width.0)), Vec::<String>::new());
    let widths: Vec<Width> = passed.overlays.iter().map(|(width, _)| *width).collect();
    assert_eq!(widths, [Width(320), Width(360), Width(768), Width(1440)]);
}

#[test]
fn failing_review_names_scores_items_and_overlays() {
    let original = facts(&[RED, BLUE]);
    let moved = facts(&[RED, Card { y: 300.0, ..BLUE }]);
    let failed = review(&original, &moved).expect("the ranges overlap");
    let findings = failed.findings(|width| format!("overlay-{}.svg", width.0));

    assert!(!failed.comparison.pass);
    assert!(
        findings.iter().any(|finding| finding.contains("below the pass mark 0.7")),
        "{findings:#?}"
    );
    assert!(findings.iter().any(|finding| finding.contains("div:nth-of-type(2)")), "{findings:#?}");
    assert!(findings.contains(&"overlay at 768px: overlay-768.svg".to_owned()), "{findings:#?}");
}
