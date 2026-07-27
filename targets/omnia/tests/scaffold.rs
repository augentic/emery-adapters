//! Base-repo scaffold behavior: fill-only writes from the embedded templates.

use std::fs;
use std::path::Path;

use omnia::scaffold::{PUBLISH_WORKFLOW, VET_CONFIG, ensure_missing, publish_placeholders};
use serde::Deserialize;
use tempfile::TempDir;

#[derive(Deserialize)]
struct Manifest {
    assemblies: Assemblies,
}

#[derive(Deserialize)]
struct Assemblies {
    core: Assembly,
}

#[derive(Deserialize)]
struct Assembly {
    files: Vec<FileEntry>,
}

#[derive(Deserialize)]
struct FileEntry {
    target: String,
}

/// Every target path the core assembly declares, in manifest order.
fn manifest_targets() -> Vec<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("templates/manifest.yaml");
    let manifest: Manifest = serde_saphyr::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    manifest.assemblies.core.files.into_iter().map(|file| file.target).collect()
}

#[test]
fn empty_tree() {
    let tmp = TempDir::new().unwrap();
    let expected = manifest_targets();

    let report = ensure_missing(tmp.path()).unwrap();

    assert_eq!(report.written, expected, "every template written in manifest order");
    assert!(report.skipped.is_empty());
    for path in &expected {
        assert!(tmp.path().join(path).is_file(), "{path} exists");
    }

    // Spot-check rendered contents against the embedded template bodies.
    let makefile = fs::read_to_string(tmp.path().join("Makefile.toml")).unwrap();
    assert!(makefile.contains("[tasks.vet]"), "cargo-vet task present");
    assert!(makefile.contains(r#""-D", "warnings""#), "clippy denies warnings");
    let vet_config = fs::read_to_string(tmp.path().join(VET_CONFIG)).unwrap();
    assert!(vet_config.contains("[imports.bytecode-alliance]"), "standard vet imports");
    assert!(
        !tmp.path().join("supply-chain/imports.lock").exists(),
        "imports.lock is cargo-vet output, never scaffolded"
    );
    let toolchain = fs::read_to_string(tmp.path().join("rust-toolchain.toml")).unwrap();
    assert!(toolchain.contains("wasm32-wasip2"), "wasm component target pinned");
    assert!(
        fs::read_dir(tmp.path()).unwrap().all(|entry| {
            !entry.unwrap().file_name().to_string_lossy().ends_with(".scaffold-tmp")
        }),
        "no atomic-write temp files left behind"
    );
}

#[test]
fn idempotent() {
    let tmp = TempDir::new().unwrap();
    ensure_missing(tmp.path()).unwrap();
    let before = fs::read_to_string(tmp.path().join("deny.toml")).unwrap();

    let report = ensure_missing(tmp.path()).unwrap();

    assert!(report.written.is_empty(), "second pass writes nothing");
    assert_eq!(report.skipped, manifest_targets());
    assert_eq!(fs::read_to_string(tmp.path().join("deny.toml")).unwrap(), before);
}

#[test]
fn fills_gaps_only() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("deny.toml"), "# consumer-customized\n").unwrap();

    let report = ensure_missing(tmp.path()).unwrap();

    assert_eq!(report.skipped, ["deny.toml"], "existing file left untouched");
    assert_eq!(report.written.len(), manifest_targets().len() - 1);
    assert_eq!(
        fs::read_to_string(tmp.path().join("deny.toml")).unwrap(),
        "# consumer-customized\n",
        "never overwritten"
    );
    assert!(tmp.path().join("Makefile.toml").is_file(), "missing siblings still written");
}

#[test]
fn publish_placeholders_pinned() {
    // `guest.md` and `configuration.md` name these tokens in prose; the
    // runtime list derives from the embedded template, and this pins the
    // two against each other.
    assert_eq!(publish_placeholders(), ["<PACKAGE_NAME>", "<STORAGE_ACCOUNT>", "<RESOURCE_GROUP>"]);
    assert!(manifest_targets().contains(&PUBLISH_WORKFLOW.to_string()), "publish target declared");
}

/// `configuration.md` keeps reference copies of the template bodies for the
/// prelude-failure fallback path; each fenced body must byte-match its
/// template so the prose cannot drift from what the prelude writes.
#[test]
fn prose_reference_parity() {
    const PAIRS: &[(&str, &str)] = &[
        ("### .cargo/config.toml", "cargo-config.toml"),
        ("### rustfmt.toml", "rustfmt.toml"),
        ("### rust-toolchain.toml", "rust-toolchain.toml"),
        ("### .vscode/settings.json", "vscode-settings.json"),
        ("### clippy.toml", "clippy.toml"),
        ("### taplo.toml", "taplo.toml"),
        ("### .gitignore", "gitignore"),
        ("### Makefile", "Makefile"),
        ("### Makefile.toml", "Makefile.toml"),
        ("### deny.toml", "deny.toml"),
        ("#### supply-chain/README.md", "supply-chain-README.md"),
        ("#### supply-chain/config.toml", "supply-chain-config.toml"),
        ("#### supply-chain/audits.toml", "supply-chain-audits.toml"),
        ("### audit.yaml", "workflow-audit.yaml"),
        ("### ci.yaml", "workflow-ci.yaml"),
        ("### patch.yaml", "workflow-patch.yaml"),
        ("### release.yaml", "workflow-release.yaml"),
        ("### publish.yaml", "workflow-publish.yaml"),
    ];
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let doc = fs::read_to_string(root.join("prose/references/configuration.md")).unwrap();
    for (heading, template) in PAIRS {
        let body = fenced_block_after(&doc, heading);
        let expected = fs::read_to_string(root.join("templates/core").join(template)).unwrap();
        assert_eq!(
            body, expected,
            "configuration.md `{heading}` drifted from templates/core/{template}"
        );
    }
}

/// The first fenced code block after the exact heading line, with the
/// fence length respected (the supply-chain README body uses a
/// four-backtick fence around nested triple-backtick blocks).
fn fenced_block_after(doc: &str, heading: &str) -> String {
    let lines: Vec<&str> = doc.lines().collect();
    let heading_idx = lines
        .iter()
        .position(|line| *line == heading)
        .unwrap_or_else(|| panic!("heading {heading:?} not found"));
    let open = (heading_idx + 1..lines.len())
        .find(|&i| lines[i].starts_with("```"))
        .unwrap_or_else(|| panic!("no fenced block after {heading:?}"));
    let fence: String = lines[open].chars().take_while(|&c| c == '`').collect();
    let close = (open + 1..lines.len())
        .find(|&i| lines[i] == fence)
        .unwrap_or_else(|| panic!("unterminated fence after {heading:?}"));
    let mut body = lines[open + 1..close].join("\n");
    body.push('\n');
    body
}
