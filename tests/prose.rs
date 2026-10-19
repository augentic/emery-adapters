//! Validates each shipped adapter's embedded `PROSE` table against its `prose/` directory.

#![cfg(not(target_arch = "wasm32"))]

use std::path::Path;

use emery_sdk::survey::Inventory;
use emery_sdk::target::{Report, Verdict};
use emery_sdk::{ClaimKind, Doc, Evidence, Lexicon, RUNTIME, body, check};

// Every shipped adapter component must have a matching test here.
test_programs::foreach_adapter!();
test_programs::foreach_target!();

// `prompts` are the documents the SDK puts to the model: `extract.md` for every
// adapter, and `survey.md` for one that has the model name its surfaces.
fn corpus(docs: &[Doc], crate_dir: &str, prompts: &[&str]) {
    held(docs, crate_dir, prompts);

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
fn target_corpus(docs: &[Doc], crate_dir: &str) {
    held(docs, crate_dir, &["build.md", "verify.md"]);

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

// The list is held to the tree under `adapters/<crate>/prose` and to the
// prompts the SDK puts, with the SDK's runtime references as the imports.
fn held(docs: &[Doc], crate_dir: &str, prompts: &[&str]) {
    let tree = Path::new(env!("CARGO_MANIFEST_DIR")).join("adapters").join(crate_dir).join("prose");
    let findings = check(docs, &tree, prompts, RUNTIME);
    assert!(
        findings.is_empty(),
        "`{crate_dir}`'s PROSE disagree with its tree:\n{}",
        findings.join("\n")
    );
}

// The SDK holds a `requirement` or `criterion` to the seam's lexicon, so a
// worked example naming a technology teaches what the gate returns.
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
    let lexicon = Lexicon::default();
    for claim in &evidence.claims {
        if !matches!(claim.kind, ClaimKind::Requirement | ClaimKind::Criterion) {
            continue;
        }
        let body = claim.extras.get("statement").or_else(|| claim.extras.get("criterion"));
        for text in claim.id.as_deref().into_iter().chain(body.and_then(|b| b.as_str())) {
            let named = lexicon.names(text);
            assert!(
                named.is_empty(),
                "`{path}`: `{text}` names the technology {}",
                named.join(", ")
            );
        }
    }
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
fn emery_documentation() {
    corpus(emery_documentation::PROSE, "documentation", &["extract.md", "survey.md"]);
    surveying(emery_documentation::PROSE);
}

#[test]
fn emery_intent() {
    corpus(emery_intent::PROSE, "intent", &["extract.md"]);
}

// `survey.md` is the prompt of the one survey turn a workspace opens, so its
// worked example is held to the SDK's survey answer the same way
// `extract.md`'s is held to its Evidence.
#[test]
fn emery_typescript() {
    corpus(emery_typescript::PROSE, "typescript", &["extract.md", "survey.md"]);
    surveying(emery_typescript::PROSE);
}

#[test]
fn emery_python() {
    corpus(emery_python::PROSE, "python", &["extract.md", "survey.md"]);
    surveying(emery_python::PROSE);
}

// The three root files the init scaffolds are fenced in `layout.md` as what the
// tree holds, so the file the crate embeds and the fence the model reads are
// held to one body.
#[test]
fn emery_node() {
    target_corpus(emery_node::PROSE, "node");

    let layout = body(emery_node::PROSE, "references/layout.md").expect("the layout is listed");
    for (file, scaffolded) in [
        ("package.json", include_str!("../adapters/node/seed/package.json")),
        ("tsconfig.json", include_str!("../adapters/node/seed/tsconfig.json")),
        (".gitignore", include_str!("../adapters/node/seed/.gitignore")),
    ] {
        let fenced = fenced_after(layout, &format!("`{file}`:"));
        assert_eq!(
            scaffolded.trim_end_matches('\n'),
            fenced,
            "`layout.md` fences `{file}` as `adapters/node/seed/{file}` spells it"
        );
    }
}

// The body of the first fence after `label`'s line, whatever its language tag.
fn fenced_after<'d>(doc: &'d str, label: &str) -> &'d str {
    let (_, section) = doc
        .split_once(&format!("\n{label}\n"))
        .unwrap_or_else(|| panic!("the document carries no `{label}` line"));
    let (_, fenced) = section.split_once("```").expect("the label is followed by a fence");
    let (_, body) = fenced.split_once('\n').expect("the fence opens on a line of its own");
    let (body, _) = body.split_once("\n```").expect("the fence closes");
    body
}

#[test]
fn emery_omnia() {
    target_corpus(emery_omnia::PROSE, "omnia");
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
