//! The one machine-checkable part of every shipped prompt: the worked
//! example. `prompts/extract.md` is the behaviour the model executes, and
//! the JSON fence under its `## Worked example` is the shape it teaches —
//! here it must parse as an Evidence document the claim gate accepts under
//! the adapter's own authority class, so a prompt edit cannot teach the
//! model a shape the adapter itself would refuse, and an engine pin that
//! moves the contract (the required extras, the id grammar, an authority)
//! fails here rather than in the live eval's repair rounds. The prompt is
//! also held under the 800 non-blank-line cap, which nothing else enforces.
//!
//! Nothing else about the corpus is asserted: the walker fails the build on
//! a dangling link, and `tests/source.rs` proves under the runtime that the
//! prompt each component embeds is the one on disk. `foreach_adapter!`
//! keeps the set complete: a new `sources/<name>` fails to compile here
//! until its corpus is checked.

#![cfg(not(target_arch = "wasm32"))]

use emery_prose::registry::{Doc, body};
use emery_sdk::{Authority, Evidence, SourceAdapter as _};

// Every `sources/*` component `crates/test-programs` builds must have a
// matching test here; a new adapter without one fails to compile.
test_programs::foreach_adapter!();

/// The prompt stays under the cap and its worked example passes the claim
/// gate under `authority`.
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
fn documentation() {
    corpus(documentation::Adapter::docs(), Authority::Documentation);
}

#[test]
fn intent() {
    corpus(intent::Adapter::docs(), Authority::Intent);
}

#[test]
fn typescript() {
    corpus(typescript::Adapter::docs(), Authority::Behaviour);
}
