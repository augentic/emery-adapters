//! The embedded prose registry: screenshots' own briefs — including the
//! nested pipeline sub-brief — and its worked example ride inside.

use specify_screenshots_core::registry;

#[test]
fn registry_embeds_the_briefs() {
    assert!(registry::body("briefs/survey.md").starts_with("# `screenshots.survey`"));
    assert!(registry::body("briefs/extract.md").starts_with("# `screenshots.extract`"));
    assert!(
        registry::doc("briefs/extract/pipeline.md").is_some(),
        "the nested spatial pipeline sub-brief is embedded"
    );
}

#[test]
fn registry_embeds_the_worked_example() {
    assert!(registry::doc("references/examples/task-list.md").is_some());
}
