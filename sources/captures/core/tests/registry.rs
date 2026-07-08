//! The embedded prose registry: captures' own prompts and the two
//! references the extract prompt requires ride inside.

use captures_core as core;
use core::registry;

#[test]
fn registry_embeds_the_prompts() {
    assert!(registry::body("prompts/survey.md").starts_with("# Runtime capture survey"));
    assert!(registry::body("prompts/extract.md").starts_with("# Runtime capture extract"));
}

/// The extract prompt instructs the agent to load both references; the
/// references server must serve them.
#[test]
fn registry_embeds_the_capture_references() {
    assert!(registry::doc("references/capture-format.md").is_some());
    assert!(registry::doc("references/extraction-mapping.md").is_some());
}
