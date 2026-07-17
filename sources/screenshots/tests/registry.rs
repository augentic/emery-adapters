//! The embedded prose registry: screenshots' own prompts — including the
//! nested pipeline sub-prompt — and its worked example ride inside.

use adapter::Source as _;
use adapter::registry::{body, find};
use screenshots::Adapter;

#[test]
fn embeds_prompts() {
    assert!(body(Adapter::docs(), "prompts/survey.md").starts_with("# `screenshots.survey`"));
    assert!(body(Adapter::docs(), "prompts/extract.md").starts_with("# `screenshots.extract`"));
    assert!(
        find(Adapter::docs(), "prompts/extract/pipeline.md").is_some(),
        "the nested spatial pipeline sub-prompt is embedded"
    );
}

#[test]
fn embeds_worked_example() {
    assert!(find(Adapter::docs(), "references/examples/task-list.md").is_some());
}
