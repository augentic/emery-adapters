//! Verifies every shipped adapter's embedded prompt corpus.
//!
//! The checks run natively over each adapter's `PROSE` and its `prose/` tree.
//! Runtime use of each embedded prompt is covered by `source.rs`.

#![cfg(not(target_arch = "wasm32"))]

use std::path::Path;

use emery_sdk::{Doc, Evidence, RUNTIME, body, check};

// Every `sources/*` component must have a matching test here.
test_programs::foreach_adapter!();

// `prompts` are the documents the SDK puts to the model: `extract.md` for every
// adapter.
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

fn gated(path: &str, json: &str) {
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

fn capped(path: &str, prompt: &str) {
    let lines = prompt.lines().filter(|line| !line.trim().is_empty()).count();
    assert!(lines <= 800, "`{path}` carries {lines} non-blank lines (cap 800)");
}

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

#[test]
fn typescript() {
    corpus(typescript::PROSE, "typescript", &["extract.md"]);
}
