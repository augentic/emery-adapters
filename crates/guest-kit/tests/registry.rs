//! The prose-registry lookup helpers over a sorted doc table.

use specify_guest_kit::registry::{Doc, body, find};

/// A sorted table, as the `specify-prose-registry` codegen emits.
static DOCS: &[Doc] = &[
    Doc {
        path: "prompts/build.md",
        body: "# build",
    },
    Doc {
        path: "prompts/guidance.md",
        body: "# guidance",
    },
    Doc {
        path: "references/verifier.md",
        body: "# verifier",
    },
];

#[test]
fn find_binary_searches_by_path() {
    assert_eq!(find(DOCS, "prompts/guidance.md").map(|doc| doc.body), Some("# guidance"));
    assert_eq!(find(DOCS, "references/verifier.md").map(|doc| doc.body), Some("# verifier"));
    assert!(find(DOCS, "prompts/missing.md").is_none());
}

#[test]
fn body_returns_guaranteed_documents() {
    assert_eq!(body(DOCS, "prompts/build.md"), "# build");
}

#[test]
#[should_panic(expected = "document `prompts/missing.md` is not embedded")]
fn body_panics_on_a_registry_miss() {
    let _ = body(DOCS, "prompts/missing.md");
}
