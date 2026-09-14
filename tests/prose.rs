//! Prompt corpora
//!
//! Every shipped adapter's `prompts/extract.md` stays under the 800
//! non-blank-line cap, and its `## Worked example` parses as the SDK's
//! `Answer` and passes the claim gate under the adapter's constant — the
//! one machine-checkable part of a prompt, and where a contract change in
//! the engine pin fails first. Reference presence is the embed-time
//! walker's; the embedded prompt is `source.rs`'s.

#![cfg(not(target_arch = "wasm32"))]

use emery_prose::registry::{Doc, body};
use emery_sdk::{Answer, SourceAdapter};

// Every `sources/*` component must have a matching test here.
test_programs::foreach_adapter!();

/// The prompt stays under the cap and its worked example passes the gate
/// under the adapter's constant.
fn corpus<A: SourceAdapter>(docs: &[Doc]) {
    let prompt = body(docs, "prompts/extract.md").expect("`prompts/extract.md` is embedded");

    let lines = prompt.lines().filter(|line| !line.trim().is_empty()).count();
    assert!(lines <= 800, "`prompts/extract.md` carries {lines} non-blank lines (cap 800)");

    let answer: Answer = serde_json::from_str(worked_example(prompt))
        .expect("the worked example is the SDK's Answer");
    let evidence = A::stamp(answer.claims);
    let findings = evidence.findings();
    assert!(findings.is_empty(), "the worked example fails the gate:\n{}", findings.join("\n"));
    assert_eq!(evidence.kind, A::KIND, "the worked example is stamped with the constant");
}

/// The first JSON fence under `## Worked example`.
fn worked_example(prompt: &str) -> &str {
    let (_, section) =
        prompt.split_once("\n## Worked example").expect("the prompt carries a worked example");
    let (_, fenced) = section.split_once("```json\n").expect("the worked example is a JSON fence");
    let (json, _) = fenced.split_once("\n```").expect("the JSON fence closes");
    json
}

#[test]
fn documentation() {
    corpus::<documentation::Adapter>(documentation::Adapter::docs());
}

#[test]
fn intent() {
    corpus::<intent::Adapter>(intent::Adapter::docs());
}

#[test]
fn typescript() {
    corpus::<typescript::Adapter>(typescript::Adapter::docs());
}
