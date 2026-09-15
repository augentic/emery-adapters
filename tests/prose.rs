//! Prompt corpora
//!
//! Every shipped adapter's `prompts/extract.md` stays under the 800
//! non-blank-line cap, and its `## Worked example` parses as the SDK's
//! `Evidence` and passes the claim gate — the one machine-checkable part of
//! a prompt, and where a contract change in the engine pin fails first. An
//! adapter that surveys by model embeds `prompts/survey.md` too, under the
//! same cap, with a worked example that parses as the SDK's `Partition`.
//! Reference presence is the embed-time walker's; the embedded prompt is
//! `source.rs`'s.

#![cfg(not(target_arch = "wasm32"))]

use emery_prose::registry::{Doc, body};
use emery_sdk::survey::Partition;
use emery_sdk::{Evidence, SourceAdapter as _};

// Every `sources/*` component must have a matching test here.
test_programs::foreach_adapter!();

/// The extraction prompt stays under the cap and its worked example passes
/// the gate; a survey prompt, when embedded, stays under the cap and its
/// worked example is a partition.
fn corpus(docs: &[Doc]) {
    let prompt = body(docs, "prompts/extract.md").expect("`prompts/extract.md` is embedded");
    capped("prompts/extract.md", prompt);

    // The example is claims alone: a document-level kind would be refused
    // by the schema, since the kind is the adapter's metadata.
    let evidence: Evidence = serde_json::from_str(worked_example(prompt))
        .expect("the worked example is the SDK's Evidence");
    let findings = evidence.findings();
    assert!(findings.is_empty(), "the worked example fails the gate:\n{}", findings.join("\n"));

    if let Some(prompt) = body(docs, "prompts/survey.md") {
        survey(prompt);
    }
}

/// The survey prompt stays under the cap and its worked example parses as
/// the SDK's `Partition`.
fn survey(prompt: &str) {
    capped("prompts/survey.md", prompt);
    let partition: Partition = serde_json::from_str(worked_example(prompt))
        .expect("the worked example is the SDK's Partition");
    assert!(
        partition.groups.iter().all(|group| !group.files.is_empty()),
        "the worked example names an empty group"
    );
}

fn capped(path: &str, prompt: &str) {
    let lines = prompt.lines().filter(|line| !line.trim().is_empty()).count();
    assert!(lines <= 800, "`{path}` carries {lines} non-blank lines (cap 800)");
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
    corpus(documentation::Adapter::docs());
}

#[test]
fn intent() {
    corpus(intent::Adapter::docs());
}

// The typescript survey asks the model, so its survey prompt must be
// embedded; a missing one is `server_error` on every multi-directory tree.
#[test]
fn typescript() {
    let docs = typescript::Adapter::docs();
    corpus(docs);
    assert!(body(docs, "prompts/survey.md").is_some(), "`prompts/survey.md` is embedded");
}
