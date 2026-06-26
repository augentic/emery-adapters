//! Shared helpers for the per-mode integration tests under
//! `tests/engine_*.rs`. Each `tests/engine_*.rs` is its own binary
//! target, so individual helpers look "dead" to whichever binary
//! does not call them; silence the lint at module scope.

#![allow(dead_code, reason = "shared test helpers; not every integration binary uses every helper")]

use std::io::Write;
use std::path::PathBuf;

use serde_json::Value;
use tempfile::{NamedTempFile, TempDir};

pub fn errors_array(envelope: &Value) -> &[Value] {
    envelope.get("errors").and_then(Value::as_array).expect("errors array").as_slice()
}

pub fn warnings_array(envelope: &Value) -> &[Value] {
    envelope.get("warnings").and_then(Value::as_array).expect("warnings array").as_slice()
}

pub fn write_named(content: &str) -> NamedTempFile {
    let mut file = NamedTempFile::new().expect("tempfile");
    file.write_all(content.as_bytes()).expect("write fixture");
    file
}

/// Build a project tree under a fresh tempdir matching the canonical
/// Specify layout: `<root>/design-system/assets.yaml` and
/// `<root>/design-system/assets/**` for raster + vector files.
/// Returns the tempdir and the assets.yaml path.
pub fn write_assets_project(yaml: &str, raster_files: &[&str]) -> (TempDir, PathBuf) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let design = tmp.path().join("design-system");
    std::fs::create_dir_all(design.join("assets/android")).expect("mkdir assets/android");
    std::fs::create_dir_all(design.join("assets/ios")).expect("mkdir assets/ios");
    let assets_path = design.join("assets.yaml");
    std::fs::write(&assets_path, yaml).expect("write assets.yaml");
    for rel in raster_files {
        let p = design.join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).expect("mkdir parent");
        }
        std::fs::write(&p, b"PNGSTUB").expect("write fixture file");
    }
    (tmp, assets_path)
}

/// Drop a `.specify/specs/composition.yaml` under `<project>/` so the
/// asset-validator's sibling-discovery walk picks it up.
pub fn write_specs_composition(project: &std::path::Path, yaml: &str) {
    let dir = project.join(".specify").join("specs");
    std::fs::create_dir_all(&dir).expect("mkdir .specify/specs");
    std::fs::write(dir.join("composition.yaml"), yaml).expect("write composition.yaml");
}

/// Materialise a Specify project root with `.specify/project.yaml`.
pub fn write_specify_project() -> TempDir {
    let tmp = tempfile::tempdir().expect("tempdir");
    write_project_yaml(tmp.path(), &["core", "ios", "android"]);
    tmp
}

/// Materialise `.specify/project.yaml` under an existing project root.
pub fn write_project_yaml(project: &std::path::Path, platforms: &[&str]) {
    let dot_specify = project.join(".specify");
    std::fs::create_dir_all(&dot_specify).expect("mkdir .specify");
    let yaml_platforms: Vec<String> = platforms.iter().map(|p| format!("  - {p}")).collect();
    let content = format!(
        "name: demo\nadapter: vectis\nspecify_version: '2.0'\nplatforms:\n{}",
        yaml_platforms.join("\n")
    );
    std::fs::write(dot_specify.join("project.yaml"), content).expect("write project.yaml");
}

/// Minimal Android shell tree (`.kt` present) for verify / scaffold tests.
pub fn scaffold_android_shell(project: &std::path::Path) {
    let dir = project.join("Android/app/src/main/kotlin/com/test");
    std::fs::create_dir_all(&dir).expect("mkdir Android");
    std::fs::write(dir.join("MainActivity.kt"), "class MainActivity").expect("write kt");
}

/// Android shell with toolchain + debug APK stubs so `verify --mode verify`
/// exits clean when `android` is declared.
pub fn scaffold_android_verify_ready(project: &std::path::Path) {
    scaffold_android_shell(project);
    let _unused = specify_vectis::android::run_for_shell_dir(&project.join("Android"));
    std::fs::write(project.join("Android/local.properties"), "sdk.dir=/tmp/android-sdk\n")
        .expect("local.properties");
    std::fs::write(
        project.join("Android/gradle.properties"),
        "android.useAndroidX=true\norg.gradle.java.home=/Library/Java/JavaVirtualMachines/temurin-21.jdk/Contents/Home\n",
    )
    .expect("gradle.properties");
    let shared_build = project.join("Android/shared/build.gradle.kts");
    std::fs::create_dir_all(shared_build.parent().expect("parent")).expect("shared dir");
    std::fs::write(&shared_build, "ndkVersion = \"26.1.10909125\"\n").expect("shared build");
    let apk_parent = project.join("Android/app/build/outputs/apk/debug");
    std::fs::create_dir_all(&apk_parent).expect("apk dir");
    std::fs::write(apk_parent.join("app-debug.apk"), b"PK").expect("apk");
    let stamp_dir = project.join("Android/.vectis");
    std::fs::create_dir_all(&stamp_dir).expect("mkdir .vectis");
    std::fs::write(stamp_dir.join("verify.ok"), "test-stamp\n").expect("android verify stamp");
}

/// iOS shell with immutable scaffold files synced so `verify --mode verify`
/// exits clean when `ios` is declared.
pub fn scaffold_ios_verify_ready(project: &std::path::Path, app_name: &str) {
    let ios = project.join("iOS");
    let app_dir = ios.join(app_name);
    std::fs::create_dir_all(&app_dir).expect("mkdir iOS app dir");
    std::fs::write(app_dir.join("ContentView.swift"), "struct ContentView {}")
        .expect("write swift");
    std::fs::write(ios.join("project.yml"), format!("name: {app_name}\n")).expect("project.yml");
    specify_vectis::ios_scaffold::sync_ios_scaffold_files(project).expect("sync ios scaffold");
    let stamp_dir = ios.join(".vectis");
    std::fs::create_dir_all(&stamp_dir).expect("mkdir .vectis");
    std::fs::write(stamp_dir.join("verify.ok"), "test-stamp\n").expect("ios verify stamp");
}
