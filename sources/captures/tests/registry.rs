//! The embedded prose registry: captures' own prompts and the two
//! references the extract prompt requires ride inside.

use adapter::Source as _;
use adapter::registry::{body, find};
use captures::Adapter;

#[test]
fn embeds_prompts() {
    assert!(body(Adapter::docs(), "prompts/survey.md").starts_with("# Runtime capture survey"));
    assert!(body(Adapter::docs(), "prompts/extract.md").starts_with("# Runtime capture extract"));
}

/// The extract prompt instructs the agent to load both references; the
/// references server must serve them.
#[test]
fn embeds_references() {
    assert!(find(Adapter::docs(), "references/capture-format.md").is_some());
    assert!(find(Adapter::docs(), "references/extraction-mapping.md").is_some());
}
