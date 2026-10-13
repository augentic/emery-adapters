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

#[test]
fn emery_node() {
    target_corpus(emery_node::PROSE, "node");
}

// The commit of the omnia exemplar the seed renders, named by the layout and
// the seed document, and the one `emery_omnia_exemplar` reads a checkout at.
const OMNIA_EXEMPLAR_COMMIT: &str = "31d89ab45c61c30f9df76ad6e11ca46982b3d239";

// The root manifest the seed lays pins omnia once: every `omnia` crate it
// names is at one version, so a bump is one edit and the tree it lays
// resolves.
#[test]
fn emery_omnia() {
    target_corpus(emery_omnia::PROSE, "omnia");

    let manifest = omnia_seed("Cargo.toml");
    let pins: Vec<(&str, &str)> = manifest
        .lines()
        .filter(|line| line.starts_with("omnia"))
        .filter_map(|line| {
            let (name, rest) = line.split_once(" = ")?;
            let version = rest
                .split_once("version = \"")
                .map(|(_, version)| version)
                .or_else(|| rest.strip_prefix('"'))?;
            let (version, _) = version.split_once('"')?;
            Some((name.trim(), version))
        })
        .collect();
    assert!(pins.len() >= 2, "the seed pins the sdk and the host crates: {pins:?}");
    let versions: std::collections::BTreeSet<&str> =
        pins.iter().map(|(_, version)| *version).collect();
    assert_eq!(versions.len(), 1, "every omnia pin in the seed is one version: {pins:?}");

    // the seed is every file of the layout outside `crates/<stem>/`, each
    // fenced in the seed document byte for byte
    let paths: std::collections::BTreeSet<&str> =
        emery_omnia::SEED.iter().map(|seed| seed.path.as_ref()).collect();
    assert_eq!(
        paths,
        [
            ".github/dependabot.yml",
            ".github/workflows/audit.yaml",
            ".github/workflows/ci.yaml",
            ".github/workflows/patch.yaml",
            ".github/workflows/publish.yaml",
            ".github/workflows/release.yaml",
            ".gitignore",
            ".vscode/settings.json",
            "Cargo.toml",
            "Makefile",
            "clippy.toml",
            "crates/common/Cargo.toml",
            "crates/common/src/config.rs",
            "crates/common/src/lib.rs",
            "crates/common/src/routes.rs",
            "crates/common/src/routes/http.rs",
            "crates/common/src/routes/topic.rs",
            "deny.toml",
            "examples/runtime.rs",
            "mise.toml",
            "rust-toolchain.toml",
            "rustfmt.toml",
            "src/lib.rs",
            "src/messaging.rs",
            "src/routes.rs",
            "supply-chain/README.md",
            "supply-chain/audits.toml",
            "supply-chain/config.toml",
            "taplo.toml",
            "tests/messaging.rs",
            "tests/routes.rs",
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(paths.len(), emery_omnia::SEED.len(), "a seed path is declared once");
    let document = body(emery_omnia::PROSE, "references/seed.md").expect("the seed is listed");
    for seed in emery_omnia::SEED {
        emery_sdk::beneath(&seed.path)
            .unwrap_or_else(|bad| panic!("the seed path `{}` {bad}", seed.path));
        assert!(
            document.contains(&format!("\n## `{}`\n", seed.path)),
            "the seed document heads `{}`",
            seed.path
        );
        assert!(
            document.contains(&format!("\n{}```", seed.content)),
            "the seed document fences `{}` byte for byte",
            seed.path
        );
    }

    let layout = body(emery_omnia::PROSE, "references/layout.md").expect("the layout is listed");
    for doc in [layout, document] {
        assert!(doc.contains(OMNIA_EXEMPLAR_COMMIT), "the document names the exemplar's commit");
    }
}

// Holds the seed to the exemplar's guest template at `OMNIA_EXEMPLAR`, a
// checkout of `augentic/omnia-exemplar` at `OMNIA_EXEMPLAR_COMMIT`; unset,
// the scenario passes with nothing read, so CI needs no clone. An `exact`
// entry is laid byte for byte; a `seed` entry is rendered for the package
// `guest` and every line of it is in the seed, save where the layout states
// a departure: the root manifest pins `omnia-test` through the workspace,
// and the entry and the route rung keep the template's wiring around the
// lists that replace its sample route.
#[test]
fn emery_omnia_exemplar() {
    let Some(checkout) = std::env::var_os("OMNIA_EXEMPLAR") else {
        return;
    };
    let checkout = Path::new(&checkout);
    let read = |path: &str| {
        std::fs::read_to_string(checkout.join(path))
            .unwrap_or_else(|err| panic!("the exemplar's `{path}`: {err}"))
    };
    let render =
        |text: String| text.replace("<PACKAGE_NAME>", "guest").replace("<CRATE_NAME>", "guest");

    let manifest = read("templates/guest/manifest.yaml");
    let mut entries: Vec<(String, String, String)> = Vec::new();
    let (mut source, mut target) = (None, None);
    for line in manifest.lines().map(str::trim) {
        if let Some(value) = line.strip_prefix("- source:") {
            source = Some(value.trim().to_owned());
        } else if let Some(value) = line.strip_prefix("target:") {
            target = Some(value.trim().to_owned());
        } else if let Some(value) = line.strip_prefix("proof:") {
            let source = source.take().expect("an entry's source precedes its proof");
            let target = target.take().expect("an entry's target precedes its proof");
            entries.push((source, target, value.trim().to_owned()));
        }
    }
    assert!(entries.len() >= 20, "the manifest lists the template: {}", entries.len());

    for (source, target, proof) in &entries {
        let template = render(read(source));
        let seed = omnia_seed(target);
        match (proof.as_str(), target.as_str()) {
            ("exact", _) => {
                assert_eq!(seed, template, "`{target}` is the exemplar's byte for byte")
            }
            ("seed", "Cargo.toml") => {
                for line in template.lines().filter(|line| !line.starts_with('#')) {
                    if let Some(rest) = line.strip_prefix("omnia-test = { version = ") {
                        let (version, _) = rest.split_once(',').expect("the pin closes");
                        let pin = format!("omnia-test = {version}");
                        assert!(
                            seed.contains(&pin),
                            "`{target}` pins `{pin}` through the workspace"
                        );
                    } else {
                        held_line(&seed, target, line);
                    }
                }
            }
            ("seed", "src/lib.rs") => {
                let (wiring, _) = template
                    .split_once("/// Build the HTTP router")
                    .expect("the template's entry holds the router after the exports");
                for line in wiring
                    .lines()
                    .filter(|line| !line.starts_with("//") && !line.starts_with("use "))
                {
                    held_line(&seed, target, line);
                }
            }
            ("seed", "tests/routes.rs") => {
                for line in [
                    "use axum::body::{Body, to_bytes};",
                    "use tower::ServiceExt as _;",
                    "#[tokio::test]",
                ] {
                    held_line(&seed, target, line);
                }
                assert!(seed.contains("guest::router("), "`{target}` drives the crate's router");
                assert!(
                    seed.contains(".oneshot("),
                    "`{target}` drives the router through `oneshot`"
                );
            }
            ("seed", _) => {
                for line in template.lines() {
                    held_line(&seed, target, line);
                }
            }
            (proof, _) => panic!("`{target}` has the proof `{proof}` this scenario does not read"),
        }
    }
}

fn omnia_seed(path: &str) -> String {
    emery_omnia::SEED
        .iter()
        .find(|seed| seed.path == path)
        .unwrap_or_else(|| panic!("the seed lays `{path}`"))
        .content
        .to_string()
}

fn held_line(seed: &str, target: &str, line: &str) {
    if line.trim().is_empty() {
        return;
    }
    assert!(seed.lines().any(|held| held == line), "`{target}` holds the template's line `{line}`");
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
