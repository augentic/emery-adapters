//! Verifies every shipped adapter's embedded prompt corpus.
//!
//! Every Markdown file must be listed once, every relative link must resolve
//! to a listed document or one of the SDK's runtime references, and every
//! reference must be reachable from a prompt. Extraction prompts and worked
//! examples are kept within their size limit and validated against the
//! evidence schema and claim gate.
//!
//! Model-assisted survey prompts receive the same checks against their
//! inventory schema. Runtime use of each embedded prompt is covered by
//! `source.rs`.

#![cfg(not(target_arch = "wasm32"))]

use std::path::Path;

use emery_sdk::survey::Inventory;
use emery_sdk::{Doc, Evidence, RUNTIME, body, check};

// Every `sources/*` component must have a matching test here.
test_programs::foreach_adapter!();

/// Checks an adapter's corpus against its tree, prompts, and examples.
///
/// `prompts` are the documents the SDK puts to the model by path:
/// `extract.md` for every adapter, and `survey.md` for one that surveys by
/// model. The worked examples are the prompt's own, under
/// `## Worked example`, and every document under `references/examples/`
/// other than `README.md`, under `## Evidence`.
fn corpus(docs: &[Doc], name: &str, prompts: &[&str]) {
    let tree = Path::new(env!("CARGO_MANIFEST_DIR")).join("sources").join(name).join("prose");
    let findings = check(docs, &tree, prompts, RUNTIME);
    assert!(
        findings.is_empty(),
        "`{name}`'s PROSE disagree with its tree:\n{}",
        findings.join("\n")
    );

    let prompt = body(docs, "extract.md").expect("the extraction prompt is listed");
    capped("extract.md", prompt);
    gated("extract.md", fenced_json(prompt, "## Worked example"));

    let examples = docs.iter().filter(|doc| {
        doc.path.strip_prefix("references/examples/").is_some_and(|file| file != "README.md")
    });
    for example in examples {
        gated(example.path, fenced_json(example.body, "## Evidence"));
    }
}

/// Checks that a worked example is nonempty evidence accepted by the claim gate.
fn gated(path: &str, json: &str) {
    // The example is claims alone: a document-level kind would be refused
    // by the schema, since the kind is the adapter's metadata.
    let evidence: Evidence = serde_json::from_str(json).unwrap_or_else(|err| {
        panic!("`{path}`: the worked example is not the SDK's Evidence: {err}")
    });
    assert!(!evidence.claims.is_empty(), "`{path}`: the worked example carries no claims");
    let findings = evidence.findings();
    assert!(
        findings.is_empty(),
        "`{path}`: the worked example fails the gate:\n{}",
        findings.join("\n")
    );
}

/// Checks a survey prompt's size and worked inventory.
///
/// The example teaches the shape the check accepts: every surface named,
/// once, and entered somewhere.
fn survey(docs: &[Doc]) {
    let prompt = body(docs, "survey.md").expect("the survey prompt is listed");
    capped("survey.md", prompt);
    let inventory: Inventory = serde_json::from_str(fenced_json(prompt, "## Worked example"))
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

/// Returns the first JSON fence under `heading` in `doc`.
fn fenced_json<'d>(doc: &'d str, heading: &str) -> &'d str {
    let (_, section) = doc
        .split_once(&format!("\n{heading}"))
        .unwrap_or_else(|| panic!("the document carries no `{heading}` section"));
    let (_, fenced) = section.split_once("```json\n").expect("the section carries a JSON fence");
    let (json, _) = fenced.split_once("\n```").expect("the JSON fence closes");
    json
}

#[test]
fn documentation() {
    corpus(documentation::PROSE, "documentation", &["extract.md"]);
}

#[test]
fn intent() {
    corpus(intent::PROSE, "intent", &["extract.md"]);
}

// The typescript survey asks the model, so its survey prompt must be listed;
// a missing one would be `server_error` on every tree.
#[test]
fn typescript() {
    corpus(typescript::PROSE, "typescript", &["extract.md", "survey.md"]);
    survey(typescript::PROSE);
}
