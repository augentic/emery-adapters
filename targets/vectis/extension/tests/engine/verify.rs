//! Integration tests for the `vectis verify` engine.
//!
//! Drives the public `verify::run` / `verify::render_json` surface
//! (the same path the `verify` binary subcommand takes) against
//! tempdir project fixtures. The CLI smoke (`tests/cli.rs`) pins the
//! main verify / bootstrap-app-icon exit-code contract black-box; this
//! suite carries the edge cases that contract does not reach: the
//! `info`-severity web/desktop path, the bootstrap §4.1 / §6.3
//! materializable-source and shell-resident escape hatches, the shell
//! catalog completeness probe, and the empty-shell-dir detection
//! corners.

use std::path::Path;

use serde_json::Value;
use specify_vectis::VectisError;
use specify_vectis::verify::{VerifyArgs, VerifyMode, render_json, run};
use tempfile::tempdir;

use crate::engine_support::write_project_yaml;

/// Drive `verify` through its public render path and return the parsed
/// envelope plus the exit code. Routing through `render_json` keeps the
/// private `verify_exit_code` mapping under coverage.
fn verify(args: &VerifyArgs) -> (Value, u8) {
    let (rendered, code) = render_json(run(args));
    let value = serde_json::from_str(&rendered).expect("verify output is JSON");
    (value, code)
}

fn args(mode: VerifyMode, root: &Path) -> VerifyArgs {
    VerifyArgs {
        mode,
        path: Some(root.to_path_buf()),
    }
}

fn scaffold_core(root: &Path) {
    let dir = root.join("shared/src");
    std::fs::create_dir_all(&dir).expect("mkdir shared/src");
    std::fs::write(dir.join("app.rs"), "pub struct App;").expect("write app.rs");
}

fn scaffold_ios(root: &Path) {
    let dir = root.join("iOS/TestApp");
    std::fs::create_dir_all(&dir).expect("mkdir iOS/TestApp");
    std::fs::write(dir.join("ContentView.swift"), "struct ContentView {}").expect("write swift");
}

fn scaffold_android(root: &Path) {
    crate::engine_support::scaffold_android_verify_ready(root);
}

fn write_app_icon_assets(root: &Path, source_rel: Option<&str>) {
    let design = root.join("design-system");
    std::fs::create_dir_all(&design).expect("mkdir design-system");
    let source_line = source_rel.map_or_else(String::new, |s| format!("    source: {s}\n"));
    let content = format!(
        "version: 1\napp-icon: brand-mark\nassets:\n  brand-mark:\n    kind: vector\n    role: app-icon\n{source_line}",
    );
    std::fs::write(design.join("assets.yaml"), content).expect("write assets.yaml");
    if let Some(rel) = source_rel {
        let path = design.join(rel);
        std::fs::create_dir_all(path.parent().expect("source parent"))
            .expect("mkdir source parent");
        std::fs::write(&path, "<svg/>").expect("write source svg");
    }
}

fn write_design_system_inventory(root: &Path) {
    let specify = root.join(".specify/specs");
    std::fs::create_dir_all(&specify).expect("mkdir specs");
    let design = root.join("design-system");
    std::fs::create_dir_all(&design).expect("mkdir design-system");
    std::fs::write(
        design.join("assets.yaml"),
        "version: 1\nassets:\n  empty-tasks-hero:\n    kind: vector\n    role: illustration\n    source: assets/empty-tasks-hero.svg\n",
    )
    .expect("write assets.yaml");
    std::fs::write(
        specify.join("composition.yaml"),
        "version: 1\nscreens:\n  empty:\n    body:\n      - image:\n          name: empty-tasks-hero\n",
    )
    .expect("write composition.yaml");
}

fn scaffold_ios_with_xcassets(root: &Path) {
    let app = root.join("iOS/TodoApp");
    std::fs::create_dir_all(app.join("Resources/Assets.xcassets")).expect("mkdir xcassets");
    std::fs::write(app.join("ContentView.swift"), "struct ContentView {}").expect("write swift");
}

// ── verify mode ────────────────────────────────────────────────────

#[test]
fn verify_all_present_exits_clean() {
    let tmp = tempdir().unwrap();
    write_project_yaml(tmp.path(), &["core", "ios", "android"]);
    scaffold_core(tmp.path());
    scaffold_ios(tmp.path());
    scaffold_android(tmp.path());

    let (result, code) = verify(&args(VerifyMode::Verify, tmp.path()));
    let findings = result["findings"].as_array().expect("findings array");
    assert!(
        !findings.iter().any(|f| f["severity"] == "error"),
        "expected no error findings: {result}"
    );
    assert_eq!(code, 0);
}

#[test]
fn verify_missing_shell_exits_one() {
    let tmp = tempdir().unwrap();
    write_project_yaml(tmp.path(), &["core", "ios"]);
    // Only the core shell is scaffolded; the declared `ios` directory is
    // absent entirely (distinct from a present-but-empty shell dir).
    scaffold_core(tmp.path());

    let (result, code) = verify(&args(VerifyMode::Verify, tmp.path()));
    let findings = result["findings"].as_array().expect("findings array");
    let errors: Vec<&Value> = findings.iter().filter(|f| f["severity"] == "error").collect();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0]["id"], "platform-shell-missing");
    assert!(errors[0]["message"].as_str().unwrap().contains("ios"));
    assert_eq!(code, 1);
}

#[test]
fn web_desktop_emit_info_not_error() {
    let tmp = tempdir().unwrap();
    write_project_yaml(tmp.path(), &["core", "web", "desktop"]);
    scaffold_core(tmp.path());

    let (result, code) = verify(&args(VerifyMode::Verify, tmp.path()));
    let findings = result["findings"].as_array().expect("findings array");
    assert!(
        !findings.iter().any(|f| f["severity"] == "error"),
        "web/desktop should not produce errors: {result}"
    );
    let infos: Vec<&Value> = findings.iter().filter(|f| f["severity"] == "info").collect();
    assert_eq!(infos.len(), 2);
    assert!(infos.iter().all(|f| f["id"] == "platform-not-yet-supported"));
    assert_eq!(code, 0);
}

#[test]
fn ios_dir_without_swift_files_is_not_present() {
    let tmp = tempdir().unwrap();
    write_project_yaml(tmp.path(), &["core", "ios"]);
    scaffold_core(tmp.path());
    let ios_dir = tmp.path().join("iOS");
    std::fs::create_dir_all(&ios_dir).expect("mkdir iOS");
    std::fs::write(ios_dir.join("README.md"), "placeholder").expect("write readme");

    let (result, _code) = verify(&args(VerifyMode::Verify, tmp.path()));
    let findings = result["findings"].as_array().expect("findings array");
    assert!(
        findings.iter().any(|f| f["id"] == "platform-shell-missing"
            && f["message"].as_str().is_some_and(|m| m.contains("ios"))),
        "iOS dir with no .swift files should be flagged missing: {result}"
    );
}

#[test]
fn android_dir_without_kt_files_is_not_present() {
    let tmp = tempdir().unwrap();
    write_project_yaml(tmp.path(), &["core", "android"]);
    scaffold_core(tmp.path());
    let android_dir = tmp.path().join("Android");
    std::fs::create_dir_all(&android_dir).expect("mkdir Android");
    std::fs::write(android_dir.join("build.gradle"), "").expect("write gradle");

    let (result, _code) = verify(&args(VerifyMode::Verify, tmp.path()));
    let findings = result["findings"].as_array().expect("findings array");
    assert!(
        findings.iter().any(|f| f["id"] == "platform-shell-missing"
            && f["message"].as_str().is_some_and(|m| m.contains("android"))),
        "Android dir with no .kt files should be flagged missing: {result}"
    );
}

// ── bootstrap-app-icon mode ────────────────────────────────────────

#[test]
fn bootstrap_app_icon_greenfield_flags_ios_and_android() {
    let tmp = tempdir().unwrap();
    write_project_yaml(tmp.path(), &["core", "ios", "android"]);

    let (result, code) = verify(&args(VerifyMode::BootstrapAppIcon, tmp.path()));
    assert_eq!(result["mode"], "bootstrap-app-icon");
    let findings = result["findings"].as_array().expect("findings array");
    assert_eq!(findings.len(), 2, "expected ios + android findings: {result}");
    assert!(findings.iter().all(|f| f["id"] == "plan-bootstrap-app-icon-missing"));
    assert!(findings.iter().all(|f| f["severity"] == "error"));
    assert_eq!(code, 1);
}

#[test]
fn bootstrap_app_icon_core_only_clean() {
    let tmp = tempdir().unwrap();
    write_project_yaml(tmp.path(), &["core"]);

    let (result, code) = verify(&args(VerifyMode::BootstrapAppIcon, tmp.path()));
    let findings = result["findings"].as_array().expect("findings array");
    assert!(findings.is_empty(), "core-only must not trigger the gate: {result}");
    assert_eq!(code, 0);
}

#[test]
fn bootstrap_app_icon_materializable_source_clean() {
    let tmp = tempdir().unwrap();
    write_project_yaml(tmp.path(), &["core", "ios", "android"]);
    write_app_icon_assets(tmp.path(), Some("assets/brand-mark.svg"));

    let (result, code) = verify(&args(VerifyMode::BootstrapAppIcon, tmp.path()));
    let findings = result["findings"].as_array().expect("findings array");
    assert!(findings.is_empty(), "path A source should satisfy the gate: {result}");
    assert_eq!(code, 0);
}

#[test]
fn bootstrap_app_icon_missing_source_flags_platforms() {
    let tmp = tempdir().unwrap();
    write_project_yaml(tmp.path(), &["core", "ios"]);
    // assets.yaml present but the `app-icon` entry carries no `source:`
    // master and no platform pin → unsatisfiable.
    write_app_icon_assets(tmp.path(), None);

    let (result, code) = verify(&args(VerifyMode::BootstrapAppIcon, tmp.path()));
    let findings = result["findings"].as_array().expect("findings array");
    assert_eq!(findings.len(), 1, "ios should be flagged: {result}");
    assert_eq!(findings[0]["id"], "plan-bootstrap-app-icon-missing");
    assert_eq!(code, 1);
}

#[test]
fn bootstrap_app_icon_shell_resident_escape_hatch() {
    let tmp = tempdir().unwrap();
    write_project_yaml(tmp.path(), &["core", "ios"]);
    // No assets.yaml, but the iOS shell already ships a launcher icon.
    let appiconset = tmp.path().join("iOS/TestApp/Resources/Assets.xcassets/AppIcon.appiconset");
    std::fs::create_dir_all(&appiconset).expect("mkdir appiconset");
    std::fs::write(
        appiconset.join("Contents.json"),
        r#"{"images":[{"filename":"AppIcon.png","idiom":"universal"}]}"#,
    )
    .expect("write Contents.json");
    std::fs::write(appiconset.join("AppIcon.png"), b"PNG").expect("write png");

    let (result, code) = verify(&args(VerifyMode::BootstrapAppIcon, tmp.path()));
    let findings = result["findings"].as_array().expect("findings array");
    assert!(findings.is_empty(), "shell-resident icon should satisfy §6.3: {result}");
    assert_eq!(code, 0);
}

#[test]
fn bootstrap_app_icon_android_shell_resident_anydpi() {
    let tmp = tempdir().unwrap();
    write_project_yaml(tmp.path(), &["core", "android"]);
    // §6.3 escape hatch via the adaptive-icon descriptor.
    let mipmap = tmp.path().join("Android/app/src/main/res/mipmap-anydpi-v26");
    std::fs::create_dir_all(&mipmap).expect("mkdir mipmap-anydpi-v26");
    std::fs::write(mipmap.join("ic_launcher.xml"), "<adaptive-icon/>")
        .expect("write ic_launcher.xml");

    let (result, code) = verify(&args(VerifyMode::BootstrapAppIcon, tmp.path()));
    let findings = result["findings"].as_array().expect("findings array");
    assert!(findings.is_empty(), "anydpi launcher should satisfy the gate: {result}");
    assert_eq!(code, 0);
}

#[test]
fn bootstrap_app_icon_android_shell_resident_mipmap_png() {
    let tmp = tempdir().unwrap();
    write_project_yaml(tmp.path(), &["core", "android"]);
    // §6.3 escape hatch via a density-bucket launcher PNG (no anydpi
    // descriptor present, so the `mipmap-*` directory scan resolves it).
    let mipmap = tmp.path().join("Android/app/src/main/res/mipmap-xxxhdpi");
    std::fs::create_dir_all(&mipmap).expect("mkdir mipmap-xxxhdpi");
    std::fs::write(mipmap.join("ic_launcher.png"), b"PNG").expect("write ic_launcher.png");

    let (result, code) = verify(&args(VerifyMode::BootstrapAppIcon, tmp.path()));
    let findings = result["findings"].as_array().expect("findings array");
    assert!(findings.is_empty(), "mipmap-bucket launcher should satisfy the gate: {result}");
    assert_eq!(code, 0);
}

// ── error paths ────────────────────────────────────────────────────

#[test]
fn project_yaml_without_platforms_returns_error() {
    let tmp = tempdir().unwrap();
    let specify_dir = tmp.path().join(".specify");
    std::fs::create_dir_all(&specify_dir).expect("mkdir .specify");
    std::fs::write(specify_dir.join("project.yaml"), "name: test-app\nadapter: vectis\n")
        .expect("write project.yaml");

    let err = run(&args(VerifyMode::Verify, tmp.path())).unwrap_err();
    assert!(matches!(err, VectisError::InvalidProject { .. }));
    let msg = format!("{err}");
    assert!(msg.contains("platforms"), "error should mention platforms: {msg}");
}

#[test]
fn render_json_missing_project_yaml_exits_two() {
    // No `.specify/project.yaml` at all: the load fails, `run` returns
    // `InvalidProject`, and `render_json` maps it to the exit-2 error
    // envelope (the wire contract the `verify` binary subcommand emits).
    let tmp = tempdir().unwrap();

    let (rendered, code) = render_json(run(&args(VerifyMode::Verify, tmp.path())));
    assert_eq!(code, 2);
    let value: Value = serde_json::from_str(&rendered).expect("error envelope is JSON");
    assert_eq!(value["error"], "invalid-project");
    assert_eq!(value["exit-code"], 2);
}

// ── catalog completeness (RFC-46 §7) ─────────────────────────────

#[test]
fn verify_catalog_without_composition_emits_no_findings() {
    let tmp = tempdir().unwrap();
    write_project_yaml(tmp.path(), &["core", "ios", "android"]);
    scaffold_core(tmp.path());
    scaffold_ios_with_xcassets(tmp.path());
    scaffold_android(tmp.path());
    // assets.yaml present, but no composition.yaml references it → the
    // catalog scan resolves the inventory then returns early.
    let design = tmp.path().join("design-system");
    std::fs::create_dir_all(&design).expect("mkdir design-system");
    std::fs::write(
        design.join("assets.yaml"),
        "version: 1\nassets:\n  empty-tasks-hero:\n    kind: vector\n    role: illustration\n    source: assets/empty-tasks-hero.svg\n",
    )
    .expect("write assets.yaml");

    let (result, code) = verify(&args(VerifyMode::Verify, tmp.path()));
    let findings = result["findings"].as_array().expect("findings array");
    assert!(
        !findings.iter().any(|f| f["id"] == "shell-catalog-entry-missing"),
        "no composition means no catalog findings: {result}"
    );
    assert_eq!(code, 0);
}

#[test]
fn verify_catalog_skips_app_icon_dedups_and_ignores_unknown_refs() {
    let tmp = tempdir().unwrap();
    write_project_yaml(tmp.path(), &["core", "ios", "android"]);
    scaffold_core(tmp.path());
    scaffold_ios_with_xcassets(tmp.path());
    scaffold_android(tmp.path());

    let design = tmp.path().join("design-system");
    std::fs::create_dir_all(&design).expect("mkdir design-system");
    std::fs::write(
        design.join("assets.yaml"),
        "version: 1\nassets:\n  brand-mark:\n    kind: vector\n    role: app-icon\n    source: assets/brand-mark.svg\n  empty-hero:\n    kind: vector\n    role: icon\n    source: assets/empty-hero.svg\n",
    )
    .expect("write assets.yaml");
    // composition references the app-icon asset (excluded from the
    // catalog scan), `empty-hero` twice (deduplicated), and a `ghost` id
    // absent from the inventory (silently ignored).
    let specs = tmp.path().join(".specify/specs");
    std::fs::create_dir_all(&specs).expect("mkdir specs");
    std::fs::write(
        specs.join("composition.yaml"),
        "version: 1\nscreens:\n  home:\n    body:\n      - image:\n          name: brand-mark\n      - image:\n          name: empty-hero\n      - image:\n          name: empty-hero\n      - image:\n          name: ghost\n",
    )
    .expect("write composition.yaml");
    // satisfy `empty-hero` on both shells (vector icon → xcassets imageset
    // on iOS, `drawable/<snake>.xml` on Android).
    let imageset = tmp.path().join("iOS/TodoApp/Resources/Assets.xcassets/empty-hero.imageset");
    std::fs::create_dir_all(&imageset).expect("mkdir imageset");
    std::fs::write(imageset.join("empty-hero@3x.png"), b"PNG").expect("write png");
    let drawable = tmp.path().join("Android/app/src/main/res/drawable/empty_hero.xml");
    std::fs::create_dir_all(drawable.parent().expect("drawable parent")).expect("mkdir drawable");
    std::fs::write(&drawable, "<vector/>").expect("write drawable xml");

    let (result, code) = verify(&args(VerifyMode::Verify, tmp.path()));
    let findings = result["findings"].as_array().expect("findings array");
    assert!(
        !findings.iter().any(|f| f["id"] == "shell-catalog-entry-missing"),
        "app-icon excluded, empty-hero satisfied, ghost ignored: {result}"
    );
    assert_eq!(code, 0);
}

#[test]
fn verify_catalog_missing_imageset_exits_one() {
    let tmp = tempdir().unwrap();
    write_project_yaml(tmp.path(), &["core", "ios", "android"]);
    scaffold_core(tmp.path());
    scaffold_ios_with_xcassets(tmp.path());
    scaffold_android(tmp.path());
    write_design_system_inventory(tmp.path());

    let (result, code) = verify(&args(VerifyMode::Verify, tmp.path()));
    let findings = result["findings"].as_array().expect("findings array");
    assert!(
        findings.iter().any(|f| f["id"] == "shell-catalog-entry-missing"),
        "expected shell catalog finding: {result}"
    );
    assert_eq!(code, 1);
}

#[test]
fn verify_catalog_present_imageset_exits_clean() {
    let tmp = tempdir().unwrap();
    write_project_yaml(tmp.path(), &["core", "ios", "android"]);
    scaffold_core(tmp.path());
    scaffold_ios_with_xcassets(tmp.path());
    scaffold_android(tmp.path());
    write_design_system_inventory(tmp.path());

    let imageset =
        tmp.path().join("iOS/TodoApp/Resources/Assets.xcassets/empty-tasks-hero.imageset");
    std::fs::create_dir_all(&imageset).expect("mkdir imageset");
    std::fs::write(imageset.join("empty-tasks-hero@3x.png"), b"PNG").expect("write png");

    let drawable =
        tmp.path().join("Android/app/src/main/res/drawable-xxxhdpi/empty_tasks_hero.png");
    std::fs::create_dir_all(drawable.parent().expect("drawable parent")).expect("mkdir drawable");
    std::fs::write(&drawable, b"PNG").expect("write android png");

    let (result, code) = verify(&args(VerifyMode::Verify, tmp.path()));
    let findings = result["findings"].as_array().expect("findings array");
    assert!(
        !findings.iter().any(|f| f["severity"] == "error"),
        "expected no catalog errors: {result}"
    );
    assert_eq!(code, 0);
}
