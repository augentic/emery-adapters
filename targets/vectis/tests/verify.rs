//! Public verification boundary: project loading, shell presence, catalog
//! checks, and exit-code projection.

use std::path::Path;

use tempfile::tempdir;
use vectis::verify::{VerifyMode, run, verify_exit_code};

fn write(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create fixture parent");
    }
    std::fs::write(path, content).expect("write fixture");
}

fn project(root: &Path, platforms: &[&str]) {
    let platforms =
        platforms.iter().map(|platform| format!("  - {platform}")).collect::<Vec<_>>().join("\n");
    write(&root.join(".emery/project.yaml"), &format!("name: test-app\nplatforms:\n{platforms}\n"));
}

fn finding_ids(value: &serde_json::Value) -> Vec<&str> {
    value["findings"]
        .as_array()
        .expect("findings array")
        .iter()
        .filter_map(|finding| finding["id"].as_str())
        .collect()
}

fn make_executable(path: &Path) {
    write(path, "#!/bin/sh\n");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;

        let mut permissions = std::fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).unwrap();
    }
}

fn fully_present_project(root: &Path) {
    project(root, &["core", "ios", "android"]);
    write(&root.join("shared/src/app.rs"), "pub struct App;");
    write(&root.join("iOS/TestApp/ContentView.swift"), "struct ContentView {}");
    write(&root.join("iOS/Makefile"), "DESTINATION ?= generic/platform=iOS Simulator\n");
    write(&root.join("iOS/project.yml"), "name: TestApp\n");
    write(&root.join("Android/Makefile"), ".PHONY: build\n");
    write(&root.join("Android/settings.gradle.kts"), "rootProject.name = \"TestApp\"\n");
    write(&root.join("Android/build.gradle.kts"), "// root\n");
    write(
        &root.join("Android/app/build.gradle.kts"),
        "android {\n    namespace = \"com.augentic.testapp\"\n}\n",
    );
    write(
        &root.join("Android/shared/build.gradle.kts"),
        "android {\n    ndkVersion = \"27.0.12077973\"\n}\n",
    );
    write(
        &root.join("Android/app/src/main/kotlin/com/augentic/testapp/MainActivity.kt"),
        "class MainActivity\n",
    );

    make_executable(&root.join("Android/gradlew"));
    write(&root.join("Android/gradle/wrapper/gradle-wrapper.jar"), "wrapper");
    write(
        &root.join("Android/gradle/wrapper/gradle-wrapper.properties"),
        "distributionUrl=https://example.invalid/gradle.zip\n",
    );
    write(&root.join("Android/local.properties"), "sdk.dir=/tmp/android-sdk\n");
    write(&root.join("Android/app/build/outputs/apk/debug/app-debug.apk"), "debug apk");
    write(&root.join("iOS/.vectis/verify.ok"), "ok\n");
    write(&root.join("Android/.vectis/verify.ok"), "ok\n");
}

#[test]
fn project_contract() {
    let missing = tempdir().unwrap();
    run(VerifyMode::Verify, missing.path()).unwrap_err();

    let malformed = tempdir().unwrap();
    write(&malformed.path().join(".emery/project.yaml"), "platforms: [");
    run(VerifyMode::Verify, malformed.path()).unwrap_err();

    let absent = tempdir().unwrap();
    write(&absent.path().join(".emery/project.yaml"), "name: test\n");
    run(VerifyMode::Verify, absent.path()).unwrap_err();

    let non_string = tempdir().unwrap();
    write(&non_string.path().join(".emery/project.yaml"), "platforms:\n  - 7\n");
    run(VerifyMode::Verify, non_string.path()).unwrap_err();
}

#[test]
fn shell_presence_and_exit_code() {
    let core = tempdir().unwrap();
    project(core.path(), &["core"]);

    let missing_core = run(VerifyMode::Verify, core.path()).unwrap();
    assert!(finding_ids(&missing_core).contains(&"platform-shell-missing"));
    assert_eq!(verify_exit_code(&missing_core), 1);

    write(&core.path().join("shared/src/app.rs"), "pub struct App;");
    let present_core = run(VerifyMode::Verify, core.path()).unwrap();
    assert!(finding_ids(&present_core).is_empty());
    assert_eq!(verify_exit_code(&present_core), 0);

    let shells = tempdir().unwrap();
    project(shells.path(), &["core", "ios", "android"]);
    write(&shells.path().join("shared/src/app.rs"), "pub struct App;");
    std::fs::create_dir_all(shells.path().join("iOS/App")).unwrap();
    std::fs::create_dir_all(shells.path().join("Android/app/src/main/kotlin")).unwrap();
    let missing_sources = run(VerifyMode::Verify, shells.path()).unwrap();
    let ids = finding_ids(&missing_sources);
    assert_eq!(ids.iter().filter(|id| **id == "platform-shell-missing").count(), 2);
    assert_eq!(verify_exit_code(&missing_sources), 1);

    let future = tempdir().unwrap();
    project(future.path(), &["core", "web", "desktop"]);
    write(&future.path().join("shared/src/app.rs"), "pub struct App;");
    let unsupported = run(VerifyMode::Verify, future.path()).unwrap();
    let ids = finding_ids(&unsupported);
    assert_eq!(ids.iter().filter(|id| **id == "platform-not-yet-supported").count(), 2);
    assert_eq!(verify_exit_code(&unsupported), 0);
}

#[test]
fn all_platforms_present() {
    let tmp = tempdir().unwrap();
    fully_present_project(tmp.path());

    let result = run(VerifyMode::Verify, tmp.path()).unwrap();

    assert!(
        finding_ids(&result).is_empty(),
        "fully present core, iOS, and Android project verifies cleanly: {result}"
    );
    assert_eq!(verify_exit_code(&result), 0);
}

fn catalog_project(root: &Path) {
    project(root, &["core", "ios"]);
    write(&root.join("shared/src/app.rs"), "pub struct App;");
    write(&root.join("iOS/TestApp/ContentView.swift"), "struct ContentView {}");
    write(
        &root.join("design-system/assets.yaml"),
        "version: 1\nassets:\n  empty-state:\n    kind: vector\n    role: illustration\n    source: assets/empty-state.svg\n  app-logo:\n    kind: vector\n    role: app-icon\n    source: assets/app-logo.svg\n",
    );
    write(
        &root.join(".emery/specs/composition.yaml"),
        "version: 1\nscreens:\n  empty:\n    body:\n      - image:\n          name: empty-state\n      - image:\n          name: empty-state\n      - image:\n          name: app-logo\n      - image:\n          name: unknown-asset\n",
    );
}

#[test]
fn catalog_through_verify() {
    let tmp = tempdir().unwrap();
    catalog_project(tmp.path());

    let missing = run(VerifyMode::Verify, tmp.path()).unwrap();
    let catalog_findings: Vec<&serde_json::Value> = missing["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|finding| finding["id"] == "shell-catalog-entry-missing")
        .collect();
    assert_eq!(
        catalog_findings.len(),
        1,
        "duplicate references collapse while app-icon and unknown references are ignored"
    );
    assert!(catalog_findings[0]["message"].as_str().unwrap().contains("empty-state"));
    assert_eq!(verify_exit_code(&missing), 1);

    let imageset = tmp.path().join("iOS/TestApp/Resources/Assets.xcassets/empty-state.imageset");
    write(
        &imageset.join("Contents.json"),
        r#"{"images":[{"filename":"empty-state.png"}],"info":{"version":1,"author":"xcode"}}"#,
    );
    write(&imageset.join("empty-state.png"), "materialized");

    let present = run(VerifyMode::Verify, tmp.path()).unwrap();
    assert!(!finding_ids(&present).contains(&"shell-catalog-entry-missing"));
}
