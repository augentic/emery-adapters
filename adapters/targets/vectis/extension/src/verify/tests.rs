//! Unit tests for the `vectis verify` engine.

use serde_json::Value;
use tempfile::tempdir;

use super::*;

fn write_project_yaml(root: &Path, platforms: &[&str]) {
    let yaml_platforms: Vec<String> = platforms.iter().map(|p| format!("  - {p}")).collect();
    let content = format!(
        "name: test-app\nadapter: vectis\nspecify_version: '2.0'\nplatforms:\n{}",
        yaml_platforms.join("\n"),
    );
    let specify_dir = root.join(".specify");
    std::fs::create_dir_all(&specify_dir).expect("mkdir .specify");
    std::fs::write(specify_dir.join("project.yaml"), content).expect("write project.yaml");
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
    let dir = root.join("Android/app/src/main/kotlin/com/test");
    std::fs::create_dir_all(&dir).expect("mkdir Android");
    std::fs::write(dir.join("MainActivity.kt"), "class MainActivity").expect("write kt");
}

// ── verify mode ────────────────────────────────────────────────────

#[test]
fn verify_all_present_exits_clean() {
    let tmp = tempdir().unwrap();
    write_project_yaml(tmp.path(), &["core", "ios", "android"]);
    scaffold_core(tmp.path());
    scaffold_ios(tmp.path());
    scaffold_android(tmp.path());

    let args = VerifyArgs {
        mode: VerifyMode::Verify,
        path: Some(tmp.path().to_path_buf()),
    };
    let result = run(&args).expect("verify should succeed");
    let findings = result["findings"].as_array().expect("findings array");

    let errors: Vec<&Value> = findings.iter().filter(|f| f["severity"] == "error").collect();
    assert!(errors.is_empty(), "expected no error findings: {result}");
    assert_eq!(verify_exit_code(&result), 0);
}

#[test]
fn verify_missing_shell_exits_one() {
    let tmp = tempdir().unwrap();
    write_project_yaml(tmp.path(), &["core", "ios"]);
    scaffold_core(tmp.path());

    let args = VerifyArgs {
        mode: VerifyMode::Verify,
        path: Some(tmp.path().to_path_buf()),
    };
    let result = run(&args).expect("verify should succeed");
    let findings = result["findings"].as_array().expect("findings array");

    let errors: Vec<&Value> = findings.iter().filter(|f| f["severity"] == "error").collect();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0]["id"], "platform-shell-missing");
    assert!(errors[0]["message"].as_str().unwrap().contains("ios"));
    assert_eq!(verify_exit_code(&result), 1);
}

#[test]
fn verify_web_desktop_emit_info_not_error() {
    let tmp = tempdir().unwrap();
    write_project_yaml(tmp.path(), &["core", "web", "desktop"]);
    scaffold_core(tmp.path());

    let args = VerifyArgs {
        mode: VerifyMode::Verify,
        path: Some(tmp.path().to_path_buf()),
    };
    let result = run(&args).expect("verify should succeed");
    let findings = result["findings"].as_array().expect("findings array");

    let errors: Vec<&Value> = findings.iter().filter(|f| f["severity"] == "error").collect();
    assert!(errors.is_empty(), "web/desktop should not produce errors: {result}");

    let infos: Vec<&Value> = findings.iter().filter(|f| f["severity"] == "info").collect();
    assert_eq!(infos.len(), 2);
    assert!(infos.iter().all(|f| f["id"] == "platform-not-yet-supported"));
    assert_eq!(verify_exit_code(&result), 0);
}

// ── bootstrap-app-icon mode ────────────────────────────────────────

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
        std::fs::create_dir_all(path.parent().unwrap()).expect("mkdir source parent");
        std::fs::write(&path, "<svg/>").expect("write source svg");
    }
}

#[test]
fn bootstrap_app_icon_greenfield_flags_ios_and_android() {
    let tmp = tempdir().unwrap();
    write_project_yaml(tmp.path(), &["core", "ios", "android"]);

    let args = VerifyArgs {
        mode: VerifyMode::BootstrapAppIcon,
        path: Some(tmp.path().to_path_buf()),
    };
    let result = run(&args).expect("bootstrap-app-icon should succeed");
    assert_eq!(result["mode"], "bootstrap-app-icon");
    let findings = result["findings"].as_array().expect("findings array");

    assert_eq!(findings.len(), 2, "expected ios + android findings: {result}");
    assert!(findings.iter().all(|f| f["id"] == "plan-bootstrap-app-icon-missing"));
    assert!(findings.iter().all(|f| f["severity"] == "error"));
    assert_eq!(verify_exit_code(&result), 1);
}

#[test]
fn bootstrap_app_icon_core_only_clean() {
    let tmp = tempdir().unwrap();
    write_project_yaml(tmp.path(), &["core"]);

    let args = VerifyArgs {
        mode: VerifyMode::BootstrapAppIcon,
        path: Some(tmp.path().to_path_buf()),
    };
    let result = run(&args).expect("bootstrap-app-icon should succeed");
    let findings = result["findings"].as_array().expect("findings array");

    assert!(findings.is_empty(), "core-only must not trigger the gate: {result}");
    assert_eq!(verify_exit_code(&result), 0);
}

#[test]
fn bootstrap_app_icon_materializable_source_clean() {
    let tmp = tempdir().unwrap();
    write_project_yaml(tmp.path(), &["core", "ios", "android"]);
    write_app_icon_assets(tmp.path(), Some("assets/brand-mark.svg"));

    let args = VerifyArgs {
        mode: VerifyMode::BootstrapAppIcon,
        path: Some(tmp.path().to_path_buf()),
    };
    let result = run(&args).expect("bootstrap-app-icon should succeed");
    let findings = result["findings"].as_array().expect("findings array");

    assert!(findings.is_empty(), "path A source should satisfy the gate: {result}");
    assert_eq!(verify_exit_code(&result), 0);
}

#[test]
fn bootstrap_app_icon_missing_source_flags_platforms() {
    let tmp = tempdir().unwrap();
    write_project_yaml(tmp.path(), &["core", "ios"]);
    // assets.yaml present but the `app-icon` entry carries no `source:`
    // master and no platform pin → unsatisfiable.
    write_app_icon_assets(tmp.path(), None);

    let args = VerifyArgs {
        mode: VerifyMode::BootstrapAppIcon,
        path: Some(tmp.path().to_path_buf()),
    };
    let result = run(&args).expect("bootstrap-app-icon should succeed");
    let findings = result["findings"].as_array().expect("findings array");

    assert_eq!(findings.len(), 1, "ios should be flagged: {result}");
    assert_eq!(findings[0]["id"], "plan-bootstrap-app-icon-missing");
    assert_eq!(verify_exit_code(&result), 1);
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

    let args = VerifyArgs {
        mode: VerifyMode::BootstrapAppIcon,
        path: Some(tmp.path().to_path_buf()),
    };
    let result = run(&args).expect("bootstrap-app-icon should succeed");
    let findings = result["findings"].as_array().expect("findings array");

    assert!(findings.is_empty(), "shell-resident icon should satisfy §6.3: {result}");
    assert_eq!(verify_exit_code(&result), 0);
}

// ── error paths ────────────────────────────────────────────────────

#[test]
fn missing_project_yaml_returns_error() {
    let tmp = tempdir().unwrap();

    let args = VerifyArgs {
        mode: VerifyMode::Verify,
        path: Some(tmp.path().to_path_buf()),
    };
    let err = run(&args).unwrap_err();
    assert!(matches!(err, VectisError::InvalidProject { .. }));
}

#[test]
fn project_yaml_without_platforms_returns_error() {
    let tmp = tempdir().unwrap();
    let specify_dir = tmp.path().join(".specify");
    std::fs::create_dir_all(&specify_dir).expect("mkdir .specify");
    std::fs::write(specify_dir.join("project.yaml"), "name: test-app\nadapter: vectis\n")
        .expect("write project.yaml");

    let args = VerifyArgs {
        mode: VerifyMode::Verify,
        path: Some(tmp.path().to_path_buf()),
    };
    let err = run(&args).unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("platforms"), "error should mention platforms: {msg}");
}

// ── render_json integration ────────────────────────────────────────

#[test]
fn render_json_verify_clean_exits_zero() {
    let tmp = tempdir().unwrap();
    write_project_yaml(tmp.path(), &["core"]);
    scaffold_core(tmp.path());

    let args = VerifyArgs {
        mode: VerifyMode::Verify,
        path: Some(tmp.path().to_path_buf()),
    };
    let (json, code) = super::render_json(run(&args));
    assert_eq!(code, 0);
    let value: Value = serde_json::from_str(&json).expect("valid JSON");
    assert_eq!(value["mode"], "verify");
}

#[test]
fn render_json_verify_miss_exits_one() {
    let tmp = tempdir().unwrap();
    write_project_yaml(tmp.path(), &["core", "ios"]);
    scaffold_core(tmp.path());

    let args = VerifyArgs {
        mode: VerifyMode::Verify,
        path: Some(tmp.path().to_path_buf()),
    };
    let (json, code) = super::render_json(run(&args));
    assert_eq!(code, 1);
    let value: Value = serde_json::from_str(&json).expect("valid JSON");
    assert_eq!(value["mode"], "verify");
}

#[test]
fn render_json_error_exits_two() {
    let tmp = tempdir().unwrap();

    let args = VerifyArgs {
        mode: VerifyMode::Verify,
        path: Some(tmp.path().to_path_buf()),
    };
    let (json, code) = super::render_json(run(&args));
    assert_eq!(code, 2);
    let value: Value = serde_json::from_str(&json).expect("valid JSON");
    assert_eq!(value["error"], "invalid-project");
    assert_eq!(value["exit-code"], 2);
}

// ── catalog completeness (RFC-46 §7) ─────────────────────────────

fn write_design_system_inventory(root: &Path) {
    let specify = root.join(".specify/specs");
    std::fs::create_dir_all(&specify).expect("mkdir specs");
    let design = root.join("design-system");
    std::fs::create_dir_all(&design).expect("mkdir design-system");
    std::fs::write(
        design.join("assets.yaml"),
        r"
version: 1
assets:
  empty-tasks-hero:
    kind: vector
    role: illustration
    source: assets/empty-tasks-hero.svg
",
    )
    .expect("write assets.yaml");
    std::fs::write(
        specify.join("composition.yaml"),
        r"
version: 1
screens:
  empty:
    body:
      - image:
          name: empty-tasks-hero
",
    )
    .expect("write composition.yaml");
}

fn scaffold_ios_with_xcassets(root: &Path) {
    let app = root.join("iOS/TodoApp");
    std::fs::create_dir_all(app.join("Resources/Assets.xcassets")).expect("mkdir xcassets");
    std::fs::write(app.join("ContentView.swift"), "struct ContentView {}").expect("write swift");
}

#[test]
fn verify_catalog_missing_imageset_exits_one() {
    let tmp = tempdir().unwrap();
    write_project_yaml(tmp.path(), &["core", "ios", "android"]);
    scaffold_core(tmp.path());
    scaffold_ios_with_xcassets(tmp.path());
    scaffold_android(tmp.path());
    write_design_system_inventory(tmp.path());

    let args = VerifyArgs {
        mode: VerifyMode::Verify,
        path: Some(tmp.path().to_path_buf()),
    };
    let result = run(&args).expect("verify should succeed");
    let findings = result["findings"].as_array().expect("findings array");

    let catalog_errors: Vec<&Value> =
        findings.iter().filter(|f| f["id"] == "shell-catalog-entry-missing").collect();
    assert!(!catalog_errors.is_empty(), "expected shell catalog finding: {result}");
    assert_eq!(verify_exit_code(&result), 1);
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
    std::fs::create_dir_all(drawable.parent().unwrap()).expect("mkdir drawable");
    std::fs::write(&drawable, b"PNG").expect("write android png");

    let args = VerifyArgs {
        mode: VerifyMode::Verify,
        path: Some(tmp.path().to_path_buf()),
    };
    let result = run(&args).expect("verify should succeed");
    let findings = result["findings"].as_array().expect("findings array");

    let errors: Vec<&Value> = findings.iter().filter(|f| f["severity"] == "error").collect();
    assert!(errors.is_empty(), "expected no catalog errors: {result}");
    assert_eq!(verify_exit_code(&result), 0);
}

// ── shell detection edge cases ─────────────────────────────────────

#[test]
fn ios_dir_without_swift_files_is_not_present() {
    let tmp = tempdir().unwrap();
    write_project_yaml(tmp.path(), &["core", "ios"]);
    scaffold_core(tmp.path());
    let ios_dir = tmp.path().join("iOS");
    std::fs::create_dir_all(&ios_dir).expect("mkdir iOS");
    std::fs::write(ios_dir.join("README.md"), "placeholder").expect("write readme");

    let args = VerifyArgs {
        mode: VerifyMode::Verify,
        path: Some(tmp.path().to_path_buf()),
    };
    let result = run(&args).expect("verify should succeed");
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

    let args = VerifyArgs {
        mode: VerifyMode::Verify,
        path: Some(tmp.path().to_path_buf()),
    };
    let result = run(&args).expect("verify should succeed");
    let findings = result["findings"].as_array().expect("findings array");

    assert!(
        findings.iter().any(|f| f["id"] == "platform-shell-missing"
            && f["message"].as_str().is_some_and(|m| m.contains("android"))),
        "Android dir with no .kt files should be flagged missing: {result}"
    );
}
