//! Verifies every shipped adapter's embedded prompt corpus.
//!
//! The checks run natively over each adapter's `PROSE` and its `prose/` tree.
//! Runtime use of each embedded prompt is covered by `source.rs` and
//! `target.rs`.

#![cfg(not(target_arch = "wasm32"))]

use std::path::Path;

use emery_sdk::survey::Inventory;
use emery_sdk::target::{Report, Verdict};
use emery_sdk::{Doc, Evidence, RUNTIME, body, check};

// Every `sources/*` and `targets/*` component must have a matching test here.
test_programs::foreach_adapter!();
test_programs::foreach_target!();

// `prompts` are the documents the SDK puts to the model: `extract.md` for every
// adapter, and `survey.md` for one that has the model name its surfaces.
fn corpus(docs: &[Doc], name: &str, prompts: &[&str]) {
    held(docs, "sources", name, prompts);

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

// A target puts two prompts, `build.md` and `verify.md`, and its worked
// examples are the SDK's `Report` and `Verdict`.
fn target_corpus(docs: &[Doc], name: &str) {
    held(docs, "targets", name, &["build.md", "verify.md"]);

    let prompt = body(docs, "build.md").expect("the build prompt is listed");
    capped("build.md", prompt);
    let report: Report = serde_json::from_str(fenced_json(prompt, "## Worked example"))
        .unwrap_or_else(|err| {
            panic!("`build.md`: the worked example is not the SDK's Report: {err}")
        });
    assert!(!report.written.is_empty(), "`build.md`: the worked example wrote nothing");
    for path in &report.written {
        emery_sdk::beneath(path)
            .unwrap_or_else(|bad| panic!("`build.md`: the worked example's `{path}` {bad}"));
    }

    let prompt = body(docs, "verify.md").expect("the verify prompt is listed");
    capped("verify.md", prompt);
    let verdict: Verdict = serde_json::from_str(fenced_json(prompt, "## Worked example"))
        .unwrap_or_else(|err| {
            panic!("`verify.md`: the worked example is not the SDK's Verdict: {err}")
        });
    let findings = verdict.findings();
    assert!(
        findings.is_empty(),
        "`verify.md`: the worked example fails the gate:\n{}",
        findings.join("\n")
    );
}

// The list is held to the tree under `<axis>/<name>/prose` and to the
// prompts the SDK puts, with the SDK's runtime references as the imports.
fn held(docs: &[Doc], axis: &str, name: &str, prompts: &[&str]) {
    let tree = Path::new(env!("CARGO_MANIFEST_DIR")).join(axis).join(name).join("prose");
    let findings = check(docs, &tree, prompts, RUNTIME);
    assert!(
        findings.is_empty(),
        "`{name}`'s PROSE disagree with its tree:\n{}",
        findings.join("\n")
    );
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
    corpus(documentation::PROSE, "documentation", &["extract.md", "survey.md"]);
    surveying(documentation::PROSE);
}

#[test]
fn intent() {
    corpus(intent::PROSE, "intent", &["extract.md"]);
}

// `survey.md` is the prompt of the one survey turn a workspace opens, so its
// worked example is held to the SDK's survey answer the same way
// `extract.md`'s is held to its Evidence.
#[test]
fn typescript() {
    corpus(typescript::PROSE, "typescript", &["extract.md", "survey.md"]);
    surveying(typescript::PROSE);
}

#[test]
fn python() {
    corpus(python::PROSE, "python", &["extract.md", "survey.md"]);
    surveying(python::PROSE);
}

#[test]
fn typescript_target() {
    target_corpus(typescript_target::PROSE, "typescript-target");
}

fn surveying(docs: &[Doc]) {
    let prompt = body(docs, "survey.md").expect("the survey prompt is listed");
    capped("survey.md", prompt);
    let inventory: Inventory = serde_json::from_str(fenced_json(prompt, "## Worked example"))
        .unwrap_or_else(|err| {
            panic!("`survey.md`: the worked example is not the SDK's Inventory: {err}")
        });
    assert!(!inventory.surfaces.is_empty(), "`survey.md`: the worked example names no surface");
    for surface in &inventory.surfaces {
        surface.anchor().unwrap_or_else(|err| {
            panic!("`survey.md`: surface `{}`'s anchor `{}` {err}", surface.name, surface.anchor)
        });
        assert!(
            emery_sdk::is_kebab(&surface.stem),
            "`survey.md`: surface `{}`'s stem `{}` is not kebab-case",
            surface.name,
            surface.stem
        );
    }
}
