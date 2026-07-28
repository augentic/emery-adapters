//! Local `vectis-exemplar` materialize contract.
//!
//! Requires a checkout at `VECTIS_EXEMPLAR_DIR` or `../vectis-exemplar` relative
//! to the emery-adapters workspace root (sibling of this repo). Skips clearly
//! when absent so CI without the template still passes.

use std::fs;
use std::path::{Path, PathBuf};

use tempfile::tempdir;
use vectis::scaffold::materialize::{
    DEFAULT_RELATIVE_DIR, Identity, TEMPLATE_DIR_ENV, map_relative_path, resolve_dir, run,
    substitute_identity,
};

fn template_dir() -> Option<PathBuf> {
    if let Ok(raw) = std::env::var(TEMPLATE_DIR_ENV) {
        let path = PathBuf::from(raw);
        if path.is_dir() {
            return Some(path);
        }
        eprintln!("skipping: {TEMPLATE_DIR_ENV}={} is not a directory", path.display());
        return None;
    }

    // targets/vectis → emery-adapters → sibling vectis-exemplar
    let from_crate =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join(DEFAULT_RELATIVE_DIR);
    if from_crate.is_dir() {
        return Some(from_crate);
    }

    // Also accept resolve_dir against the adapters workspace root.
    let adapters_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    if let Some(path) = resolve_dir(&adapters_root) {
        return Some(path);
    }

    eprintln!(
        "skipping: no local vectis-exemplar (set {TEMPLATE_DIR_ENV} or clone {DEFAULT_RELATIVE_DIR} next to emery-adapters)"
    );
    None
}

fn require_template() -> Option<PathBuf> {
    let dir = template_dir()?;
    // Cheap shape gate so a random directory does not produce opaque failures.
    if !dir.join("Cargo.toml").is_file() || !dir.join("shared").is_dir() {
        eprintln!("skipping: {} does not look like vectis-exemplar", dir.display());
        return None;
    }
    Some(dir)
}

#[test]
fn path_and_identity_rewrite() {
    let id = Identity::new("Counter", "com.example.counter").unwrap();
    assert_eq!(
        map_relative_path("iOS/VectisApp/VectisApp.swift", &id),
        "iOS/Counter/Counter.swift"
    );
    assert_eq!(
        map_relative_path("Android/app/src/main/java/io/augentic/vectisapp/MainActivity.kt", &id),
        "Android/app/src/main/java/com/example/counter/MainActivity.kt"
    );
    let body = substitute_identity(
        "package io.augentic.vectisapp\nstruct VectisApp_iOSApp\nSCHEME := VectisApp-iOS\nfun VectisTheme()\npub struct VectisApp;\nrootProject.name = \"Vectis\"\n<string name=\"app_name\">Vectis</string>\n",
        &id,
    );
    assert!(body.contains("package com.example.counter"));
    assert!(body.contains("struct Counter_iOSApp"));
    assert!(body.contains("SCHEME := Counter-iOS"));
    assert!(body.contains("fun CounterTheme()"));
    assert!(body.contains("pub struct Counter;"));
    assert!(body.contains("rootProject.name = \"Counter\""));
    assert!(body.contains("<string name=\"app_name\">Counter</string>"));
    assert!(!body.contains("VectisApp"));
    assert!(!body.contains("io.augentic.vectisapp"));
}

#[test]
fn allowlist_and_denylist() {
    let Some(template) = require_template() else {
        return;
    };
    let dest = tempdir().unwrap();
    let identity = Identity::new("Counter", "com.example.counter").unwrap();
    let report = run(&template, dest.path(), &identity).unwrap();

    assert!(report.files.iter().any(|p| p == "Cargo.toml"));
    assert!(report.files.iter().any(|p| p == "Cargo.lock"));
    assert!(report.files.iter().any(|p| p == "Makefile"));
    assert!(report.files.iter().any(|p| p == "Makefile.toml"));
    assert!(report.files.iter().any(|p| p == "rust-toolchain.toml"));
    assert!(report.files.iter().any(|p| p == "deny.toml"));
    assert!(report.files.iter().any(|p| p == "shared/Cargo.toml"));
    assert!(report.files.iter().any(|p| p == "shared/boltffi.toml"));
    assert!(report.files.iter().any(|p| p == "Android/gradle/libs.versions.toml"));
    assert!(report.files.iter().any(|p| p == "Android/gradlew"));
    assert!(report.files.iter().any(|p| p == "Android/gradle/wrapper/gradle-wrapper.jar"));
    assert!(report.files.iter().any(|p| p == "iOS/project.yml"));
    assert!(report.files.iter().any(|p| p == "iOS/Counter/Counter.swift"));
    assert!(
        report
            .files
            .iter()
            .any(|p| { p == "Android/app/src/main/java/com/example/counter/MainActivity.kt" })
    );
    assert!(report.files.iter().any(|p| p == ".maestro/config.yaml"));
    assert!(report.files.iter().any(|p| p == ".maestro/test-ids.yaml"));
    assert!(report.files.iter().any(|p| p == ".maestro/scripts/load-test-ids.sh"));
    assert!(report.files.iter().any(|p| p == "shared/src/bin/codegen/main.rs"));
    assert!(report.files.iter().any(|p| p == "iOS/Makefile"));
    assert!(report.files.iter().any(|p| p == "Android/Makefile"));
    assert!(report.files.iter().any(|p| p == "README.md"));
    assert!(report.files.iter().any(|p| p == "supply-chain/config.toml"));

    let ios_app = fs::read_to_string(dest.path().join("iOS/Counter/Counter.swift")).unwrap();
    assert!(ios_app.contains("struct Counter_iOSApp"));
    assert!(!ios_app.contains("VectisApp"));
    let makefile = fs::read_to_string(dest.path().join("iOS/Makefile")).unwrap();
    assert!(makefile.contains("SCHEME := Counter-iOS"));
    let settings = fs::read_to_string(dest.path().join("Android/settings.gradle.kts")).unwrap();
    assert!(settings.contains("rootProject.name = \"Counter\""));

    assert!(!dest.path().join("web").exists(), "web/ must not be copied");
    assert!(!dest.path().join("AGENTS.md").exists(), "AGENTS.md must not be copied");
    assert!(!dest.path().join(".git").exists());
    assert!(!dest.path().join(".github").exists());
    assert!(!dest.path().join("iOS/VectisApp").exists());
    assert!(!dest.path().join("iOS/VectisApp.xcodeproj").exists());
    assert!(!dest.path().join("Android/local.properties").exists());
    assert!(!dest.path().join("Android/.gradle").exists());
    assert!(!dest.path().join("target").exists());

    assert!(report.files.iter().all(|p| !p.starts_with("web/")), "no web/ path in report");
    assert!(report.files.iter().all(|p| !p.contains("AGENTS.md")));
}

#[test]
fn pin_files_byte_equal() {
    let Some(template) = require_template() else {
        return;
    };
    let dest = tempdir().unwrap();
    let identity = Identity::new("Counter", "com.example.counter").unwrap();
    run(&template, dest.path(), &identity).unwrap();

    // Identity-free pin / policy surfaces must be byte-identical to the template.
    for rel in [
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
        "deny.toml",
        "shared/Cargo.toml",
        "Android/gradle/libs.versions.toml",
        "supply-chain/config.toml",
        "supply-chain/audits.toml",
        "supply-chain/imports.lock",
    ] {
        assert_byte_equal(&template, dest.path(), rel);
    }

    // boltffi.toml carries the Android package id; the version pin line in
    // shared/Cargo.toml is identity-free and already asserted byte-equal above.
    let dest_bolt = fs::read_to_string(dest.path().join("shared/boltffi.toml")).unwrap();
    assert!(dest_bolt.contains("com.example.counter"), "package id must be rewritten");
    assert!(!dest_bolt.contains("io.augentic.vectisapp"), "template package id must not remain");

    let workspace = fs::read_to_string(dest.path().join("Cargo.toml")).unwrap();
    assert!(workspace.contains("crux_core = "));
    assert!(!workspace.contains("VectisApp"));
}

#[test]
fn refuses_overwrite_and_missing_template() {
    let missing = Path::new("/no/such/vectis-exemplar-dir-for-test");
    let dest = tempdir().unwrap();
    let identity = Identity::new("Counter", "com.example.counter").unwrap();
    let err = run(missing, dest.path(), &identity).unwrap_err();
    assert!(err.to_string().contains("template directory not found"));

    let Some(template) = require_template() else {
        return;
    };
    run(&template, dest.path(), &identity).unwrap();
    let err = run(&template, dest.path(), &identity).unwrap_err();
    assert!(err.to_string().contains("refusing to overwrite"));
}

#[test]
fn replaces_emery_init_gitignore_stub() {
    let Some(template) = require_template() else {
        return;
    };
    let dest = tempdir().unwrap();
    // Shape of `.gitignore` after `emery init` — framework lines only.
    fs::write(dest.path().join(".gitignore"), ".emery/scratch/\nworkspace/\n").unwrap();
    let identity = Identity::new("Counter", "com.example.counter").unwrap();
    let report = run(&template, dest.path(), &identity).unwrap();

    assert!(report.files.iter().any(|p| p == ".gitignore"));
    let gitignore = fs::read_to_string(dest.path().join(".gitignore")).unwrap();
    assert!(
        gitignore.lines().any(|line| line.trim() == "target/"),
        "template platform ignores must land"
    );
    assert!(
        gitignore.lines().any(|line| line.trim() == "Android/.gradle/"),
        "template Android ignores must land"
    );
    assert!(
        gitignore.lines().any(|line| line.trim() == ".emery/scratch/"),
        "Emery scratch entry must survive"
    );
    assert!(
        gitignore.lines().any(|line| line.trim() == "workspace/"),
        "Emery workspace entry must survive"
    );
}

#[test]
fn refuses_non_gitignore_root_overwrite() {
    let Some(template) = require_template() else {
        return;
    };
    let dest = tempdir().unwrap();
    fs::write(dest.path().join("Makefile"), "# pre-existing\n").unwrap();
    let identity = Identity::new("Counter", "com.example.counter").unwrap();
    let err = run(&template, dest.path(), &identity).unwrap_err();
    assert!(err.to_string().contains("refusing to overwrite"));
    assert!(err.to_string().contains("Makefile"));
}

#[test]
fn refuses_nested_gitignore_overwrite() {
    // Minimal template shape: nested `.gitignore` must stay fail-closed even
    // though the root `.gitignore` stub is overwriteable.
    let template = tempdir().unwrap();
    fs::write(template.path().join("Cargo.toml"), "[workspace]\n").unwrap();
    fs::write(template.path().join(".gitignore"), "target/\n").unwrap();
    for dir in ["shared", "iOS", "Android", "supply-chain", ".maestro"] {
        fs::create_dir_all(template.path().join(dir)).unwrap();
    }
    fs::write(template.path().join("Android/.gitignore"), "local.properties\n").unwrap();

    let dest = tempdir().unwrap();
    fs::create_dir_all(dest.path().join("Android")).unwrap();
    fs::write(dest.path().join("Android/.gitignore"), "# keep me\n").unwrap();
    // Root stub alone must not be enough to pass when a nested file collides.
    fs::write(dest.path().join(".gitignore"), ".emery/scratch/\n").unwrap();

    let identity = Identity::new("Counter", "com.example.counter").unwrap();
    let err = run(template.path(), dest.path(), &identity).unwrap_err();
    assert!(err.to_string().contains("refusing to overwrite"));
    assert!(err.to_string().contains("Android"));
}

fn assert_byte_equal(template: &Path, dest: &Path, rel: &str) {
    let left = fs::read(template.join(rel)).unwrap_or_else(|err| {
        panic!("read template {rel}: {err}");
    });
    let right = fs::read(dest.join(rel)).unwrap_or_else(|err| {
        panic!("read dest {rel}: {err}");
    });
    assert_eq!(left, right, "{rel} must be byte-identical to the template");
}
