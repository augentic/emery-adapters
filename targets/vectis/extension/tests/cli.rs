//! CLI coverage for the `vectis` subcommand surface.

use assert_cmd::Command;
use serde_json::Value;
use tempfile::tempdir;

fn vectis() -> Command {
    Command::cargo_bin("vectis").expect("binary `vectis` is present")
}

fn vectis_validate() -> Command {
    let mut cmd = vectis();
    cmd.arg("validate");
    cmd
}

fn parse_json(stdout: &[u8]) -> Value {
    let s = String::from_utf8(stdout.to_vec()).expect("utf8 stdout");
    serde_json::from_str(&s).unwrap_or_else(|err| panic!("stdout is not JSON ({err}): {s}"))
}

#[test]
fn assets_clean_run_exits_zero() {
    let tmp = tempdir().unwrap();
    let assets_path = tmp.path().join("assets.yaml");
    std::fs::write(&assets_path, "version: 1\nassets: {}\n").expect("write assets.yaml");

    let assert = vectis_validate().args(["assets"]).arg(&assets_path).assert().success();
    let value = parse_json(&assert.get_output().stdout);

    assert_eq!(value["mode"], "assets");
    assert_eq!(value["path"], assets_path.display().to_string());
    assert_eq!(value["errors"].as_array().map(Vec::len), Some(0));
    assert_eq!(value["warnings"].as_array().map(Vec::len), Some(0));
}

#[test]
fn findings_exit_one_with_success_envelope() {
    let tmp = tempdir().unwrap();
    let tokens_path = tmp.path().join("tokens.yaml");
    std::fs::write(&tokens_path, ": : not valid yaml :::\n").expect("write tokens.yaml");

    let assert = vectis_validate().args(["tokens"]).arg(&tokens_path).assert().failure();
    let output = assert.get_output();
    let value = parse_json(&output.stdout);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(value["mode"], "tokens");
    assert_eq!(value["errors"].as_array().map(Vec::len), Some(1));
    assert!(
        value["errors"][0]["message"].as_str().unwrap_or("").contains("invalid YAML"),
        "unexpected error payload: {value}"
    );
}

#[test]
fn missing_input_exits_two() {
    let tmp = tempdir().unwrap();
    let missing = tmp.path().join("missing-tokens.yaml");

    let assert = vectis_validate().args(["tokens"]).arg(&missing).assert().failure();
    let output = assert.get_output();
    let value = parse_json(&output.stdout);

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(value["error"], "invalid-project");
    assert_eq!(value["exit-code"], 2);
}

#[test]
fn invalid_invocation_exits_two() {
    vectis_validate().arg("nope").assert().failure().code(2);
}

#[test]
fn composition_absent_skips_cleanly() {
    // Core-only projects carry no composition.yaml by design; default
    // resolution (no `[path]`) must skip, not error.
    let tmp = tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join(".specify")).expect("mkdir .specify");

    let assert =
        vectis_validate().env("PROJECT_DIR", tmp.path()).arg("composition").assert().success();
    let value = parse_json(&assert.get_output().stdout);

    assert_eq!(value["mode"], "composition");
    assert_eq!(value["status"], "skipped");
    assert_eq!(value["errors"].as_array().map(Vec::len), Some(0));
}

#[test]
fn omitted_path_uses_default_root() {
    let tmp = tempdir().unwrap();
    let slice_dir = tmp.path().join(".specify/slices/active");
    std::fs::create_dir_all(&slice_dir).expect("mkdir slice");
    std::fs::write(slice_dir.join("layout.yaml"), "version: 1\nscreens: {}\n")
        .expect("write layout.yaml");

    let assert = vectis_validate().env("PROJECT_DIR", tmp.path()).arg("layout").assert().success();
    let value = parse_json(&assert.get_output().stdout);

    assert_eq!(value["mode"], "layout");
    let resolved = value["path"].as_str().expect("path is a string");
    assert!(
        resolved.ends_with(".specify/slices/active/layout.yaml"),
        "expected PROJECT_DIR default resolution, got: {resolved}"
    );
}

#[test]
fn default_root_walks_up_from_nested_cwd() {
    // No PROJECT_DIR: default resolution must walk up from the process
    // CWD to the nearest `.specify/` root, then resolve the slice-local
    // layout against that root. A child process owns its own CWD, so
    // this exercises the walk-up without mutating the test process.
    let tmp = tempdir().unwrap();
    let slice_dir = tmp.path().join(".specify/slices/active");
    std::fs::create_dir_all(&slice_dir).expect("mkdir slice");
    std::fs::write(slice_dir.join("layout.yaml"), "version: 1\nscreens: {}\n")
        .expect("write layout.yaml");
    let nested = tmp.path().join("a/b/c");
    std::fs::create_dir_all(&nested).expect("mkdir nested cwd");

    let assert = vectis_validate()
        .env_remove("PROJECT_DIR")
        .current_dir(&nested)
        .arg("layout")
        .assert()
        .success();
    let value = parse_json(&assert.get_output().stdout);

    assert_eq!(value["mode"], "layout");
    let resolved = value["path"].as_str().expect("path is a string");
    assert!(
        resolved.ends_with(".specify/slices/active/layout.yaml"),
        "expected CWD walk-up to the .specify root, got: {resolved}"
    );
}

#[test]
fn all_mode_recurses_findings_exit_code() {
    let tmp = tempdir().unwrap();
    let design = tmp.path().join("design-system");
    std::fs::create_dir_all(&design).expect("mkdir design-system");
    std::fs::write(design.join("tokens.yaml"), ": : not valid yaml :::\n")
        .expect("write tokens.yaml");

    let assert = vectis_validate().env("PROJECT_DIR", tmp.path()).arg("all").assert().failure();
    let output = assert.get_output();
    let value = parse_json(&output.stdout);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(value["mode"], "all");
    assert!(
        value["results"].as_array().expect("results array").iter().any(|entry| {
            entry["report"]["errors"].as_array().is_some_and(|errors| !errors.is_empty())
        }),
        "expected nested findings in all-mode payload: {value}"
    );
}

// ── verify subcommand ──────────────────────────────────────────────

fn vectis_verify() -> Command {
    let mut cmd = vectis();
    cmd.arg("verify");
    cmd
}

fn write_project_yaml(root: &std::path::Path, platforms: &[&str]) {
    let yaml_platforms: Vec<String> = platforms.iter().map(|p| format!("  - {p}")).collect();
    let content = format!(
        "name: test-app\nadapter: vectis\nspecify_version: '2.0'\nplatforms:\n{}",
        yaml_platforms.join("\n"),
    );
    let specify_dir = root.join(".specify");
    std::fs::create_dir_all(&specify_dir).expect("mkdir .specify");
    std::fs::write(specify_dir.join("project.yaml"), content).expect("write project.yaml");
}

fn scaffold_core(root: &std::path::Path) {
    let dir = root.join("shared/src");
    std::fs::create_dir_all(&dir).expect("mkdir shared/src");
    std::fs::write(dir.join("app.rs"), "pub struct App;").expect("write app.rs");
}

fn scaffold_ios(root: &std::path::Path) {
    let dir = root.join("iOS/TestApp");
    std::fs::create_dir_all(&dir).expect("mkdir iOS/TestApp");
    std::fs::write(dir.join("ContentView.swift"), "struct ContentView {}").expect("write swift");
    std::fs::write(root.join("iOS/project.yml"), "name: TestApp\n").expect("project yml");
    specify_vectis::ios_scaffold::sync_ios_scaffold_files(root).expect("sync ios scaffold");
    let stamp_dir = root.join("iOS/.vectis");
    std::fs::create_dir_all(&stamp_dir).expect("mkdir .vectis");
    std::fs::write(stamp_dir.join("verify.ok"), "test-stamp\n").expect("ios verify stamp");
}

fn scaffold_android(root: &std::path::Path) {
    let dir = root.join("Android/app/src/main/kotlin/com/test");
    std::fs::create_dir_all(&dir).expect("mkdir Android");
    std::fs::write(dir.join("MainActivity.kt"), "class MainActivity").expect("write kt");
    let _unused = specify_vectis::android::run_for_shell_dir(&root.join("Android"));
    std::fs::write(root.join("Android/local.properties"), "sdk.dir=/tmp/android-sdk\n")
        .expect("local.properties");
    std::fs::write(
        root.join("Android/gradle.properties"),
        "android.useAndroidX=true\norg.gradle.java.home=/Library/Java/JavaVirtualMachines/temurin-21.jdk/Contents/Home\n",
    )
    .expect("gradle.properties");
    let shared_build = root.join("Android/shared/build.gradle.kts");
    std::fs::create_dir_all(shared_build.parent().expect("parent")).expect("shared dir");
    std::fs::write(&shared_build, "ndkVersion = \"26.1.10909125\"\n").expect("shared build");
    let apk_parent = root.join("Android/app/build/outputs/apk/debug");
    std::fs::create_dir_all(&apk_parent).expect("apk dir");
    std::fs::write(apk_parent.join("app-debug.apk"), b"PK").expect("apk");
    let stamp_dir = root.join("Android/.vectis");
    std::fs::create_dir_all(&stamp_dir).expect("mkdir .vectis");
    std::fs::write(stamp_dir.join("verify.ok"), "test-stamp\n").expect("android verify stamp");
}

#[test]
fn verify_host_prereq_core_only_exits_zero() {
    let tmp = tempdir().unwrap();
    write_project_yaml(tmp.path(), &["core"]);

    let assert = vectis_verify()
        .args(["--mode", "host-prereq"])
        .arg(tmp.path())
        .assert()
        .success();
    let value = parse_json(&assert.get_output().stdout);

    assert_eq!(value["mode"], "host-prereq");
    let findings = value["findings"].as_array().expect("findings array");
    assert!(findings.is_empty(), "core-only must not require host tools: {value}");
}

#[test]
fn verify_verify_all_present_exits_zero() {
    let tmp = tempdir().unwrap();
    write_project_yaml(tmp.path(), &["core", "ios", "android"]);
    scaffold_core(tmp.path());
    scaffold_ios(tmp.path());
    scaffold_android(tmp.path());

    let assert = vectis_verify().args(["--mode", "verify"]).arg(tmp.path()).assert().success();
    let value = parse_json(&assert.get_output().stdout);

    assert_eq!(value["mode"], "verify");
    let findings = value["findings"].as_array().expect("findings array");
    assert!(
        findings.iter().all(|f| f["severity"] != "error"),
        "expected no error findings: {value}"
    );
}

#[test]
fn verify_verify_missing_shell_exits_one() {
    let tmp = tempdir().unwrap();
    write_project_yaml(tmp.path(), &["core", "android"]);
    scaffold_core(tmp.path());

    let assert = vectis_verify().args(["--mode", "verify"]).arg(tmp.path()).assert().failure();
    let output = assert.get_output();
    let value = parse_json(&output.stdout);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(value["mode"], "verify");
    let findings = value["findings"].as_array().expect("findings array");
    assert!(findings.iter().any(|f| f["id"] == "platform-shell-missing"));
}

#[test]
fn verify_bootstrap_app_icon_greenfield_exits_one() {
    let tmp = tempdir().unwrap();
    write_project_yaml(tmp.path(), &["core", "ios", "android"]);

    let assert =
        vectis_verify().args(["--mode", "bootstrap-app-icon"]).arg(tmp.path()).assert().failure();
    let output = assert.get_output();
    let value = parse_json(&output.stdout);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(value["mode"], "bootstrap-app-icon");
    let findings = value["findings"].as_array().expect("findings array");
    assert_eq!(findings.len(), 2, "expected ios + android findings: {value}");
    assert!(findings.iter().all(|f| f["id"] == "plan-bootstrap-app-icon-missing"));
}

#[test]
fn verify_bootstrap_app_icon_core_only_exits_zero() {
    let tmp = tempdir().unwrap();
    write_project_yaml(tmp.path(), &["core"]);

    let assert =
        vectis_verify().args(["--mode", "bootstrap-app-icon"]).arg(tmp.path()).assert().success();
    let value = parse_json(&assert.get_output().stdout);

    assert_eq!(value["mode"], "bootstrap-app-icon");
    assert!(value["findings"].as_array().expect("findings array").is_empty());
}

#[test]
fn verify_missing_project_yaml_exits_two() {
    let tmp = tempdir().unwrap();

    let assert = vectis_verify().args(["--mode", "verify"]).arg(tmp.path()).assert().failure();
    let output = assert.get_output();
    let value = parse_json(&output.stdout);

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(value["error"], "invalid-project");
    assert_eq!(value["exit-code"], 2);
}

#[test]
fn verify_uses_project_dir_env() {
    let tmp = tempdir().unwrap();
    write_project_yaml(tmp.path(), &["core"]);
    scaffold_core(tmp.path());

    let assert = vectis_verify()
        .env("PROJECT_DIR", tmp.path())
        .args(["--mode", "verify"])
        .assert()
        .success();
    let value = parse_json(&assert.get_output().stdout);

    assert_eq!(value["mode"], "verify");
    let findings = value["findings"].as_array().expect("findings array");
    assert!(findings.iter().all(|f| f["severity"] != "error"));
}

// ── schema subcommand ──────────────────────────────────────────────

fn vectis_schema() -> Command {
    let mut cmd = vectis();
    cmd.arg("schema");
    cmd
}

#[test]
fn schema_tokens_exits_zero_with_valid_json() {
    let assert = vectis_schema().arg("tokens").assert().success();
    let value = parse_json(&assert.get_output().stdout);
    assert!(value["$id"].as_str().is_some(), "$id field must be present");
    assert_eq!(value["title"], "Specify Tokens Artifact");
}

#[test]
fn schema_assets_exits_zero_with_valid_json() {
    let assert = vectis_schema().arg("assets").assert().success();
    let value = parse_json(&assert.get_output().stdout);
    assert!(value["$id"].as_str().is_some_and(|id| id.contains("assets")));
}

#[test]
fn schema_composition_exits_zero_with_valid_json() {
    let assert = vectis_schema().arg("composition").assert().success();
    let value = parse_json(&assert.get_output().stdout);
    assert!(value["$id"].as_str().is_some_and(|id| id.contains("composition")));
}

#[test]
fn schema_unknown_exits_two_with_error_envelope() {
    let assert = vectis_schema().arg("nonexistent").assert().failure();
    let output = assert.get_output();
    let value = parse_json(&output.stdout);

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(value["error"], "unknown-schema");
    assert_eq!(value["exit-code"], 2);
    assert!(
        value["message"].as_str().unwrap_or("").contains("nonexistent"),
        "error message should mention the requested name"
    );
}

// ── infer subcommand ───────────────────────────────────────────────

#[test]
fn infer_emits_name_free_cluster_report() {
    let tmp = tempdir().unwrap();
    let comp = tmp.path().join("composition.yaml");
    std::fs::write(
        &comp,
        "version: 1\nscreens:\n  home:\n    name: Home\n    footer:\n      - group:\n          items:\n            - icon-button: {}\n            - icon-button: {}\n  search:\n    name: Search\n    footer:\n      - group:\n          items:\n            - icon-button: {}\n            - icon-button: {}\n",
    )
    .expect("write composition.yaml");

    let assert = vectis().args(["infer", "--composition"]).arg(&comp).assert().success();
    let value = parse_json(&assert.get_output().stdout);

    assert_eq!(value["version"], 1);
    let clusters = value["clusters"].as_array().expect("clusters array");
    assert_eq!(clusters.len(), 1, "expected one cluster: {value}");
    assert_eq!(clusters[0]["occurrences"], 2);
    assert_eq!(clusters[0]["bound-slug"], Value::Null);
    assert!(clusters[0]["fingerprint"].as_str().is_some(), "cluster carries a fingerprint");
}

// ── materialize subcommand ───────────────────────────────────────────

fn vectis_materialize() -> Command {
    let mut cmd = vectis();
    cmd.arg("materialize");
    cmd
}

#[test]
fn materialize_assets_clean_run_exits_zero() {
    let tmp = tempdir().unwrap();
    let assets_path = tmp.path().join("assets.yaml");
    std::fs::write(&assets_path, "version: 1\nassets: {}\n").expect("write assets.yaml");

    let assert = vectis_materialize().args(["assets"]).arg(&assets_path).assert().success();
    let value = parse_json(&assert.get_output().stdout);

    assert_eq!(value["command"], "materialize assets");
    assert_eq!(value["path"], assets_path.display().to_string());
    assert_eq!(value["dry_run"], false);
    assert_eq!(value["materialized"].as_array().map(Vec::len), Some(0));
    assert_eq!(value["skipped_pins"].as_array().map(Vec::len), Some(0));
    assert_eq!(value["errors"].as_array().map(Vec::len), Some(0));
}

#[test]
fn materialize_assets_resolves_relative_path_against_project_dir() {
    let tmp = tempdir().unwrap();
    let slice_assets = tmp.path().join(".specify/slices/active/assets.yaml");
    std::fs::create_dir_all(slice_assets.parent().expect("parent")).expect("mkdir slice");
    std::fs::write(&slice_assets, "version: 1\nassets: {}\n").expect("write assets.yaml");

    let assert = vectis_materialize()
        .env("PROJECT_DIR", tmp.path())
        .args(["assets", ".specify/slices/active/assets.yaml"])
        .assert()
        .success();
    let value = parse_json(&assert.get_output().stdout);

    assert_eq!(value["command"], "materialize assets");
    assert_eq!(value["path"], slice_assets.display().to_string());
    assert_eq!(value["errors"].as_array().map(Vec::len), Some(0));
}

#[test]
fn materialize_assets_relative_path_without_project_dir_is_cwd_relative() {
    let tmp = tempdir().unwrap();
    let design = tmp.path().join("design-system");
    std::fs::create_dir_all(&design).expect("mkdir design-system");
    let assets_path = design.join("assets.yaml");
    std::fs::write(&assets_path, "version: 1\nassets: {}\n").expect("write assets.yaml");

    let assert = vectis_materialize()
        .env_remove("PROJECT_DIR")
        .current_dir(&design)
        .args(["assets", "assets.yaml"])
        .assert()
        .success();
    let value = parse_json(&assert.get_output().stdout);

    assert_eq!(value["path"], "assets.yaml");
    assert_eq!(value["errors"].as_array().map(Vec::len), Some(0));
}

#[test]
fn materialize_assets_dry_run_missing_file_exits_two() {
    let tmp = tempdir().unwrap();
    let missing = tmp.path().join("missing-assets.yaml");

    let assert =
        vectis_materialize().args(["assets", "--dry-run"]).arg(&missing).assert().failure();
    let output = assert.get_output();
    let value = parse_json(&output.stdout);

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(value["error"], "invalid-project");
    assert_eq!(value["exit-code"], 2);
}

#[test]
fn materialize_assets_missing_file_exits_two() {
    let tmp = tempdir().unwrap();
    let missing = tmp.path().join("missing-assets.yaml");

    let assert = vectis_materialize().args(["assets"]).arg(&missing).assert().failure();
    let output = assert.get_output();
    let value = parse_json(&output.stdout);

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(value["error"], "invalid-project");
    assert_eq!(value["exit-code"], 2);
}

// ── prepare subcommand ───────────────────────────────────────────────

fn vectis_prepare() -> Command {
    let mut cmd = vectis();
    cmd.arg("prepare");
    cmd
}

const TRIANGLE_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <path fill="#010203" d="M12 2L2 22h20z"/>
</svg>"##;

#[test]
fn prepare_build_slice_local_materializes_missing_export() {
    let tmp = tempdir().unwrap();
    write_project_yaml(tmp.path(), &["core", "ios"]);

    let design = tmp.path().join("design-system");
    std::fs::create_dir_all(design.join("assets")).expect("mkdir design assets");
    std::fs::write(design.join("assets/launcher.svg"), TRIANGLE_SVG).expect("write launcher svg");
    std::fs::write(
        design.join("assets.yaml"),
        r"version: 1
app-icon: launcher
assets:
  launcher:
    role: app-icon
    kind: vector
    source: assets/launcher.svg
",
    )
    .expect("write project assets");

    let slice_dir = tmp.path().join(".specify/slices/active");
    std::fs::create_dir_all(slice_dir.join("assets")).expect("mkdir assets");
    std::fs::write(slice_dir.join("assets/glyph.svg"), TRIANGLE_SVG).expect("write svg");

    let yaml = r#"version: 1
assets:
  glyph:
    kind: vector
    role: icon
    alt: "Glyph"
    source: assets/glyph.svg
"#;
    std::fs::write(slice_dir.join("assets.yaml"), yaml).expect("write slice assets");

    let assert = vectis_prepare()
        .env("PROJECT_DIR", tmp.path())
        .args(["build", ".specify/slices/active"])
        .assert()
        .success();
    let value = parse_json(&assert.get_output().stdout);

    assert_eq!(value["command"], "prepare build");
    assert_eq!(value["slice_dir"], ".specify/slices/active");
    assert_eq!(value["platforms"], serde_json::json!(["ios"]));
    assert!(
        value["materialized"]["materialized"].as_array().is_some_and(|arr| !arr.is_empty()),
        "expected materialized exports: {value}"
    );
    assert!(
        slice_dir.join("assets/exports/ios/glyph.imageset/glyph.pdf").is_file(),
        "ios export should exist after prepare build"
    );
}

#[test]
fn prepare_build_missing_app_icon_exits_one() {
    let tmp = tempdir().unwrap();
    write_project_yaml(tmp.path(), &["core", "ios"]);
    std::fs::create_dir_all(tmp.path().join(".specify/slices/active")).expect("mkdir slice");

    let assert = vectis_prepare()
        .env("PROJECT_DIR", tmp.path())
        .args(["build", ".specify/slices/active"])
        .assert()
        .failure();
    let output = assert.get_output();
    let value = parse_json(&output.stdout);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(value["command"], "prepare build");
    let findings = value["bootstrap_app_icon"]["findings"].as_array().expect("bootstrap findings");
    assert!(!findings.is_empty());
    assert!(findings.iter().all(|f| f["id"] == "plan-bootstrap-app-icon-missing"));
}

#[test]
fn prepare_build_invalid_slice_assets_exits_two() {
    let tmp = tempdir().unwrap();
    write_project_yaml(tmp.path(), &["core", "ios"]);
    let slice_dir = tmp.path().join(".specify/slices/active");
    std::fs::create_dir_all(&slice_dir).expect("mkdir slice");
    std::fs::write(slice_dir.join("assets.yaml"), ": : not valid yaml\n").expect("write assets");

    let assert = vectis_prepare()
        .env("PROJECT_DIR", tmp.path())
        .args(["build", ".specify/slices/active"])
        .assert()
        .failure();
    let output = assert.get_output();
    let value = parse_json(&output.stdout);

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(value["error"], "invalid-project");
    assert_eq!(value["exit-code"], 2);
}

// ── sync subcommand ────────────────────────────────────────────────────

fn vectis_sync() -> Command {
    let mut cmd = vectis();
    cmd.arg("sync");
    cmd
}

#[test]
fn sync_ios_scaffold_restores_drifted_makefile() {
    let tmp = tempdir().unwrap();
    write_project_yaml(tmp.path(), &["core", "ios"]);
    let ios = tmp.path().join("iOS");
    std::fs::create_dir_all(ios.join("TestApp")).expect("mkdir app");
    std::fs::write(ios.join("project.yml"), "name: TestApp\n").expect("project yml");
    std::fs::write(ios.join("TestApp/ContentView.swift"), "struct ContentView {}").expect("swift");
    std::fs::write(
        ios.join("Makefile"),
        // drift fixture — forbidden in real trees; sync must restore
        "sim-build:\n\t@xcodebuild -destination 'platform=iOS Simulator,name=iPhone 16'\n",
    )
    .expect("drifted makefile");

    let assert =
        vectis_sync().env("PROJECT_DIR", tmp.path()).args(["ios-scaffold"]).assert().success();
    let value = parse_json(&assert.get_output().stdout);

    assert_eq!(value["command"], "sync ios-scaffold");
    assert!(
        value["scaffold_sync"]["ios"]["synced"]
            .as_array()
            .expect("synced")
            .iter()
            .any(|entry| entry == "iOS/Makefile")
    );

    let restored = std::fs::read_to_string(ios.join("Makefile")).expect("read makefile");
    assert!(restored.contains(".vectis/sim-build.sh"));
    assert!(!restored.contains("iPhone 16"));
    let script = std::fs::read_to_string(ios.join(".vectis/sim-build.sh")).expect("read script");
    assert!(script.contains("generic/platform=iOS Simulator"));
}

#[test]
fn infer_missing_composition_exits_two() {
    let tmp = tempdir().unwrap();
    let missing = tmp.path().join("composition.yaml");

    let assert = vectis().args(["infer", "--composition"]).arg(&missing).assert().failure();
    let output = assert.get_output();
    let value = parse_json(&output.stdout);

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(value["error"], "invalid-project");
    assert_eq!(value["exit-code"], 2);
}
