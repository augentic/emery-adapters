//! Checks every shipped adapter's prompt corpus.
//!
//! Each adapter's `DOCS` is held to its `prose/` tree — every document listed
//! once, every relative link a document in the table — so nothing under
//! `prose/` ships unlisted and nothing a prompt tells the model to read is
//! missing from `read_doc`. Each `prompts/extract.md` stays under the 800
//! non-blank-line cap, and its `## Worked example` parses as the SDK's
//! `Evidence` and passes the claim gate — the one machine-checkable part of a
//! prompt, and where a contract change in the engine pin fails first. An
//! adapter that surveys by model carries `prompts/survey.md` too, under the
//! same cap, with a worked example that parses as the SDK's `Inventory`. The
//! prompt each component embeds is `source.rs`'s.

#![cfg(not(target_arch = "wasm32"))]

use std::path::Path;

use emery_sdk::survey::Inventory;
use emery_sdk::{Doc, Evidence, prose};

// Every `sources/*` component must have a matching test here.
test_programs::foreach_adapter!();

/// Checks an adapter's corpus: listed in step with its tree, its extraction prompt under the cap with a worked example that passes the gate.
fn corpus(docs: &[Doc], name: &str) {
    let tree = Path::new(env!("CARGO_MANIFEST_DIR")).join("sources").join(name).join("prose");
    let findings = prose::check(docs, &tree);
    assert!(
        findings.is_empty(),
        "`{name}`'s DOCS disagree with its tree:\n{}",
        findings.join("\n")
    );

    let prompt = prose::body(docs, "prompts/extract.md").expect("the extraction prompt is listed");
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
fn survey(docs: &[Doc]) {
    let prompt = prose::body(docs, "prompts/survey.md").expect("the survey prompt is listed");
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
    corpus(documentation::DOCS, "documentation");
}

#[test]
fn intent() {
    corpus(intent::DOCS, "intent");
}

// The typescript survey asks the model, so its survey prompt must be listed;
// a missing one would be `server_error` on every tree.
#[test]
fn typescript() {
    corpus(typescript::DOCS, "typescript");
    survey(typescript::DOCS);
}
