//! The embedded prose registry: screenshots' own prompts — including the
//! nested pipeline sub-prompt — and its worked example ride inside.

use screenshots::registry;

#[test]
fn embeds_prompts() {
    assert!(registry::body("prompts/survey.md").starts_with("# `screenshots.survey`"));
    assert!(registry::body("prompts/extract.md").starts_with("# `screenshots.extract`"));
    assert!(
        registry::doc("prompts/extract/pipeline.md").is_some(),
        "the nested spatial pipeline sub-prompt is embedded"
    );
}

#[test]
fn embeds_worked_example() {
    assert!(registry::doc("references/examples/task-list.md").is_some());
}
