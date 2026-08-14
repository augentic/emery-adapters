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
        body(Adapter::docs(), "prompts/extract.md").contains("`$PROJECT_DIR` are unreachable"),
        "extract names the prepared-tree grant boundary"
    );
    let pipeline = body(Adapter::docs(), "prompts/extract/pipeline.md");
    assert!(
        find(Adapter::docs(), "prompts/extract/pipeline.md").is_some(),
        "the nested spatial pipeline sub-prompt is embedded"
    );
    assert!(
        !pipeline.contains(".emery/.cache/component-candidates"),
        "stage-6 candidates ride Evidence notes; the prompt must not write a project-tree sidecar"
    );
    assert!(
        !pipeline
            .contains("then containers (in pre-order tree walk under each region), then leaves"),
        "kind-grouped emission puts nested groups before sibling leaves"
    );
    assert!(
        pipeline.contains("bbox") && pipeline.contains("visual sibling order"),
        "candidate reconstruction needs bbox-sorted visual sibling order"
    );
}

#[test]
fn embeds_worked_example() {
    assert!(find(Adapter::docs(), "references/examples/task-list.md").is_some());
}
