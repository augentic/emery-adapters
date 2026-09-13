//! The embedded corpus of every shipped adapter: `prompts/extract.md` is
//! present (the SDK's `prompt` answers `server_error` otherwise) and under
//! the 800 non-blank-line cap, and its worked example — the JSON fence under
//! `## Worked example` — is an Evidence document the claim gate accepts
//! under the adapter's own authority class, so a prompt edit cannot teach
//! the model a shape the adapter itself would refuse. Each adapter then
//! asserts the registry facts of its own: the rule overlay or the deep
//! references its prompt links. Reference presence in general is the
//! embed-time walker's — a dangling link or symlink fails the build.
//!
//! `foreach_adapter!` is the orphan guard for the adapter set: a new
//! `sources/<name>` component fails to compile here until a test of that
//! name exists.

#![cfg(not(target_arch = "wasm32"))]

use emery_prose::registry::{Doc, body, find};
use emery_sdk::{Authority, Evidence, SourceAdapter as _};

// Every `sources/*` component `crates/test-programs` builds must have a
// matching test here; a new adapter without one fails to compile.
test_programs::foreach_adapter!();

/// What every adapter's corpus satisfies: the extraction prompt is embedded,
/// stays under the cap, and its worked example passes the claim gate under
/// `authority`.
fn corpus(docs: &[Doc], authority: Authority) {
    let prompt = body(docs, "prompts/extract.md").expect("`prompts/extract.md` is embedded");

    let lines = prompt.lines().filter(|line| !line.trim().is_empty()).count();
    assert!(lines <= 800, "`prompts/extract.md` carries {lines} non-blank lines (cap 800)");

    let evidence: Evidence = serde_json::from_str(worked_example(prompt))
        .expect("the worked example is an Evidence document");
    let findings = evidence.findings();
    assert!(findings.is_empty(), "the worked example fails the gate:\n{}", findings.join("\n"));
    assert_eq!(evidence.authority, authority, "the worked example carries the adapter's authority");
}

/// The first JSON fence under the prompt's `## Worked example` heading.
fn worked_example(prompt: &str) -> &str {
    let (_, section) =
        prompt.split_once("\n## Worked example").expect("the prompt carries a worked example");
    let (_, fenced) = section.split_once("```json\n").expect("the worked example is a JSON fence");
    let (json, _) = fenced.split_once("\n```").expect("the JSON fence closes");
    json
}

#[test]
fn intent() {
    corpus(intent::Adapter::docs(), Authority::Intent);
}

// Documentation embeds its SRC-001 rule overlay beside the prompt.
#[test]
fn documentation() {
    let docs = documentation::Adapter::docs();
    corpus(docs, Authority::Documentation);
    let rule = find(docs, "rules/documentation-verbatim-preservation.md")
        .expect("SRC-001 rule overlay is embedded");
    assert!(rule.body.contains("id: SRC-001"), "rule frontmatter carries its id");
}

// TypeScript's prompt links its deep references; they ride inside.
#[test]
fn typescript() {
    let docs = typescript::Adapter::docs();
    corpus(docs, Authority::Behaviour);
    for path in ["references/business-logic.md", "references/language-mapping.md"] {
        assert!(find(docs, path).is_some(), "registry embeds `{path}`");
    }
}
