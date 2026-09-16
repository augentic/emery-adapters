//! Checks every shipped adapter's prompt corpus.
//!
//! Each `prompts/extract.md` stays under the 800 non-blank-line cap, and its
//! `## Worked example` parses as the SDK's `Evidence` and passes the claim
//! gate — the one machine-checkable part of a prompt, and where a contract
//! change in the engine pin fails first. An adapter that surveys by model
//! carries `prompts/survey.md` too, under the same cap, with a worked example
//! that parses as the SDK's `Inventory`. The prompts are read from the
//! `prose/` tree the embed-time walker copies verbatim; reference presence is
//! that walker's, and the prompt each component embeds is `source.rs`'s.

#![cfg(not(target_arch = "wasm32"))]

use emery_sdk::Evidence;
use emery_sdk::survey::Inventory;

// Every `sources/*` component must have a matching test here.
test_programs::foreach_adapter!();

/// Checks an extraction prompt: under the cap, with a worked example that passes the gate.
fn corpus(prompt: &str) {
    capped("prompts/extract.md", prompt);

    // The example is claims alone: a document-level kind would be refused
    // by the schema, since the kind is the adapter's metadata.
    let evidence: Evidence = serde_json::from_str(worked_example(prompt))
        .expect("the worked example is the SDK's Evidence");
    let findings = evidence.findings();
    assert!(findings.is_empty(), "the worked example fails the gate:\n{}", findings.join("\n"));
}

/// Checks a survey prompt: under the cap, with a worked example that is an `Inventory`.
///
/// The example teaches the shape the check accepts: every surface named,
/// once, and entered somewhere.
fn survey(prompt: &str) {
    capped("prompts/survey.md", prompt);
    let inventory: Inventory = serde_json::from_str(worked_example(prompt))
        .expect("the worked example is the SDK's Inventory");
    assert!(!inventory.surfaces.is_empty(), "the worked example exposes no surface");
    let mut names = std::collections::BTreeSet::new();
    for surface in &inventory.surfaces {
        assert!(!surface.name.trim().is_empty(), "a surface at `{}` has no name", surface.entry);
        assert!(!surface.entry.is_empty(), "surface `{}` has no entry", surface.name);
        assert!(names.insert(&surface.name), "surface `{}` is listed twice", surface.name);
    }
}

fn capped(path: &str, prompt: &str) {
    let lines = prompt.lines().filter(|line| !line.trim().is_empty()).count();
    assert!(lines <= 800, "`{path}` carries {lines} non-blank lines (cap 800)");
}

/// Returns the first JSON fence under `## Worked example`.
fn worked_example(prompt: &str) -> &str {
    let (_, section) =
        prompt.split_once("\n## Worked example").expect("the prompt carries a worked example");
    let (_, fenced) = section.split_once("```json\n").expect("the worked example is a JSON fence");
    let (json, _) = fenced.split_once("\n```").expect("the JSON fence closes");
    json
}

#[test]
fn documentation() {
    corpus(include_str!("../sources/documentation/prose/prompts/extract.md"));
}

#[test]
fn intent() {
    corpus(include_str!("../sources/intent/prose/prompts/extract.md"));
}

// The typescript survey asks the model, so its survey prompt must exist —
// `include_str!` fails the build without it; a missing one would be
// `server_error` on every tree.
#[test]
fn typescript() {
    corpus(include_str!("../sources/typescript/prose/prompts/extract.md"));
    survey(include_str!("../sources/typescript/prose/prompts/survey.md"));
}
