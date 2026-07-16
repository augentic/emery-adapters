//! The embedded prose registry: screenshots' own prompts — including the
//! nested pipeline sub-prompt — and its worked example ride inside.

use adapter::Source as _;
use adapter::registry::{body, find};
use screenshots::Screenshots;

#[test]
fn embeds_prompts() {
    assert!(body(Screenshots::docs(), "prompts/survey.md").starts_with("# `screenshots.survey`"));
    assert!(body(Screenshots::docs(), "prompts/extract.md").starts_with("# `screenshots.extract`"));
    assert!(
        find(Screenshots::docs(), "prompts/extract/pipeline.md").is_some(),
        "the nested spatial pipeline sub-prompt is embedded"
    );
}

#[test]
fn embeds_worked_example() {
    assert!(find(Screenshots::docs(), "references/examples/task-list.md").is_some());
}
