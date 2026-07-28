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
    // `build.rs` fetches the exemplar contract into OUT_DIR and exports
    // the staged path; there is no committed templates/ tree.
    let path = Path::new(env!("OMNIA_TEMPLATES_DIR")).join("manifest.yaml");
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
    // runtime list derives from the fetched exemplar template, and this
    // pins the two against each other (the token set is also declared in
    // the exemplar manifest's `tokens` map).
    assert_eq!(publish_placeholders(), ["<PACKAGE_NAME>", "<STORAGE_ACCOUNT>", "<RESOURCE_GROUP>"]);
    assert!(manifest_targets().contains(&PUBLISH_WORKFLOW.to_string()), "publish target declared");
}
