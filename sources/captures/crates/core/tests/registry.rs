//! The embedded prose registry: captures' own briefs and the two
//! references the extract brief requires ride inside.

use specify_captures_core::registry;

#[test]
fn registry_embeds_the_briefs() {
    assert!(registry::body("briefs/survey.md").starts_with("# Runtime capture survey"));
    assert!(registry::body("briefs/extract.md").starts_with("# Runtime capture extract"));
}

/// The extract brief instructs the agent to load both references; the
/// shelf must serve them.
#[test]
fn registry_embeds_the_capture_references() {
    assert!(registry::doc("references/capture-format.md").is_some());
    assert!(registry::doc("references/extraction-mapping.md").is_some());
}
