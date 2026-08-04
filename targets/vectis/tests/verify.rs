//! Public verification boundary: project loading, shell presence, catalog
//! checks, and exit-code projection.

use std::path::Path;

use tempfile::tempdir;
use vectis::verify::{CORE_VERIFY_STAMP, VerifyMode, core_src_digest, run, verify_exit_code};

fn write(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create fixture parent");
    }
    std::fs::write(path, content).expect("write fixture");
}

fn write_fresh_core_stamp(root: &Path) {
    let digest =
        core_src_digest(root).expect("core digest io").expect("core digest for present shared/src");
    write(&root.join(CORE_VERIFY_STAMP), &format!("{digest}\n"));
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
    write(
        &root.join("iOS/Makefile"),
        "DESTINATION ?= generic/platform=iOS Simulator\n\
         package:\n\tcd ../shared && boltffi pack apple\n",
    );
    write(
        &root.join("iOS/project.yml"),
        "name: TestApp\npackages:\n  Shared:\n    path: ./generated/Shared\n",
    );
    write(
        &root.join("Android/Makefile"),
        ".PHONY: build\npackage:\n\tcd ../shared && boltffi pack android\n",
    );
    write(&root.join("Android/settings.gradle.kts"), "rootProject.name = \"TestApp\"\n");
    write(&root.join("Android/build.gradle.kts"), "// root\n");
    write(
        &root.join("Android/app/build.gradle.kts"),
        "android {\n    namespace = \"com.augentic.testapp\"\n}\n",
    );
    write(
        &root.join("Android/shared/build.gradle.kts"),
        "android {\n    jniLibs.directories += \"${rootProject.projectDir}/generated/jniLibs\"\n}\n",
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
    write_fresh_core_stamp(root);
}

#[test]
fn project_contract() {
    let missing = tempdir().unwrap();
    run(VerifyMode::Verify, missing.path(), missing.path()).unwrap_err();

    let malformed = tempdir().unwrap();
    write(&malformed.path().join(".emery/project.yaml"), "platforms: [");
    run(VerifyMode::Verify, malformed.path(), malformed.path()).unwrap_err();

    let absent = tempdir().unwrap();
    write(&absent.path().join(".emery/project.yaml"), "name: test\n");
    run(VerifyMode::Verify, absent.path(), absent.path()).unwrap_err();

    let non_string = tempdir().unwrap();
    write(&non_string.path().join(".emery/project.yaml"), "platforms:\n  - 7\n");
    run(VerifyMode::Verify, non_string.path(), non_string.path()).unwrap_err();
}

#[test]
fn shell_presence_and_exit_code() {
    let core = tempdir().unwrap();
    project(core.path(), &["core"]);

    let missing_core = run(VerifyMode::Verify, core.path(), core.path()).unwrap();
    assert!(finding_ids(&missing_core).contains(&"platform-shell-missing"));
    assert_eq!(verify_exit_code(&missing_core), 1);

    write(&core.path().join("shared/src/app.rs"), "pub struct App;");
    write_fresh_core_stamp(core.path());
    let present_core = run(VerifyMode::Verify, core.path(), core.path()).unwrap();
    assert!(finding_ids(&present_core).is_empty());
    assert_eq!(verify_exit_code(&present_core), 0);

    let shells = tempdir().unwrap();
    project(shells.path(), &["core", "ios", "android"]);
    write(&shells.path().join("shared/src/app.rs"), "pub struct App;");
    std::fs::create_dir_all(shells.path().join("iOS/App")).unwrap();
    std::fs::create_dir_all(shells.path().join("Android/app/src/main/kotlin")).unwrap();
    let missing_sources = run(VerifyMode::Verify, shells.path(), shells.path()).unwrap();
    let ids = finding_ids(&missing_sources);
    assert_eq!(ids.iter().filter(|id| **id == "platform-shell-missing").count(), 2);
    assert_eq!(verify_exit_code(&missing_sources), 1);

    let future = tempdir().unwrap();
    project(future.path(), &["core", "web", "desktop"]);
    write(&future.path().join("shared/src/app.rs"), "pub struct App;");
    write_fresh_core_stamp(future.path());
    let unsupported = run(VerifyMode::Verify, future.path(), future.path()).unwrap();
    let ids = finding_ids(&unsupported);
    assert_eq!(ids.iter().filter(|id| **id == "platform-not-yet-supported").count(), 2);
    assert_eq!(verify_exit_code(&unsupported), 0);
}

#[test]
fn core_verify_stamp_missing_stale_fresh() {
    let tmp = tempdir().unwrap();
    project(tmp.path(), &["core"]);
    write(&tmp.path().join("shared/src/app.rs"), "pub struct App;");

    let missing = run(VerifyMode::Verify, tmp.path(), tmp.path()).unwrap();
    assert!(
        finding_ids(&missing).contains(&"core-verify-stamp-missing"),
        "present core without stamp: {missing}"
    );
    assert_eq!(verify_exit_code(&missing), 1);

    write(&tmp.path().join(CORE_VERIFY_STAMP), "sha256:deadbeef\n");
    let stale = run(VerifyMode::Verify, tmp.path(), tmp.path()).unwrap();
    assert!(finding_ids(&stale).contains(&"core-verify-stamp-stale"), "mismatched digest: {stale}");
    assert!(!finding_ids(&stale).contains(&"core-verify-stamp-missing"));
    assert_eq!(verify_exit_code(&stale), 1);

    write_fresh_core_stamp(tmp.path());
    let fresh = run(VerifyMode::Verify, tmp.path(), tmp.path()).unwrap();
    assert!(finding_ids(&fresh).is_empty(), "matching digest passes: {fresh}");
    assert_eq!(verify_exit_code(&fresh), 0);

    // Editing a tracked source after the stamp was written goes stale.
    write(&tmp.path().join("shared/src/app.rs"), "pub struct App; // touched\n");
    let after_edit = run(VerifyMode::Verify, tmp.path(), tmp.path()).unwrap();
    assert!(
        finding_ids(&after_edit).contains(&"core-verify-stamp-stale"),
        "post-stamp core edit: {after_edit}"
    );
}

#[test]
fn core_src_digest_sorts_by_relative_path() {
    let reverse = tempdir().unwrap();
    // Create in reverse lexical order so directory walk order cannot
    // accidentally match the required relative-path sort.
    write(&reverse.path().join("shared/src/z.rs"), "z");
    write(&reverse.path().join("shared/src/m/b.rs"), "b");
    write(&reverse.path().join("shared/src/a.rs"), "a");

    let forward = tempdir().unwrap();
    write(&forward.path().join("shared/src/a.rs"), "a");
    write(&forward.path().join("shared/src/m/b.rs"), "b");
    write(&forward.path().join("shared/src/z.rs"), "z");

    let left = core_src_digest(reverse.path()).expect("digest io").expect("shared/src present");
    let right = core_src_digest(forward.path()).expect("digest io").expect("shared/src present");
    assert_eq!(left, right, "digest must be independent of creation/walk order");
}

#[cfg(unix)]
#[test]
fn core_verify_digest_unreadable_fails_closed() {
    use std::os::unix::fs::PermissionsExt as _;

    let tmp = tempdir().unwrap();
    project(tmp.path(), &["core"]);
    write(&tmp.path().join("shared/src/app.rs"), "pub struct App;");
    let locked = tmp.path().join("shared/src/locked.rs");
    write(&locked, "secret");
    let mut permissions = std::fs::metadata(&locked).unwrap().permissions();
    permissions.set_mode(0o000);
    std::fs::set_permissions(&locked, permissions).unwrap();

    let result = run(VerifyMode::Verify, tmp.path(), tmp.path());

    // Restore so tempdir cleanup can remove the tree.
    let mut permissions = std::fs::metadata(&locked).unwrap().permissions();
    permissions.set_mode(0o644);
    std::fs::set_permissions(&locked, permissions).unwrap();

    let findings = result.expect("verify returns findings JSON");
    assert!(
        finding_ids(&findings).contains(&"core-verify-digest-unreadable"),
        "unreadable core source must not skip the stamp gate: {findings}"
    );
    assert_eq!(verify_exit_code(&findings), 1);
}

#[test]
fn all_platforms_present() {
    let tmp = tempdir().unwrap();
    fully_present_project(tmp.path());

    let result = run(VerifyMode::Verify, tmp.path(), tmp.path()).unwrap();

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
    write(&root.join("iOS/.vectis/verify.ok"), "ok\n");
    write_fresh_core_stamp(root);
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
fn boltffi_dx_patterns_required() {
    let tmp = tempdir().unwrap();
    project(tmp.path(), &["core", "ios", "android"]);
    write(&tmp.path().join("shared/src/app.rs"), "pub struct App;");
    write(&tmp.path().join("iOS/TestApp/ContentView.swift"), "struct ContentView {}");
    write(
        &tmp.path().join("iOS/Makefile"),
        "DESTINATION ?= generic/platform=iOS Simulator\n# intentionally omit boltffi apple pack\n",
    );
    write(&tmp.path().join("iOS/project.yml"), "name: TestApp\npath: ./generated/Shared\n");
    write(
        &tmp.path().join("Android/Makefile"),
        ".PHONY: build\n# intentionally omit boltffi android pack\n",
    );
    write(&tmp.path().join("Android/settings.gradle.kts"), "rootProject.name = \"TestApp\"\n");
    write(&tmp.path().join("Android/build.gradle.kts"), "// root\n");
    write(
        &tmp.path().join("Android/app/build.gradle.kts"),
        "android {\n    namespace = \"com.augentic.testapp\"\n}\n",
    );
    write(&tmp.path().join("Android/shared/build.gradle.kts"), "android {}\n");
    write(
        &tmp.path().join("Android/app/src/main/kotlin/com/augentic/testapp/MainActivity.kt"),
        "class MainActivity\n",
    );
    make_executable(&tmp.path().join("Android/gradlew"));
    write(&tmp.path().join("Android/gradle/wrapper/gradle-wrapper.jar"), "wrapper");
    write(
        &tmp.path().join("Android/gradle/wrapper/gradle-wrapper.properties"),
        "distributionUrl=https://example.invalid/gradle.zip\n",
    );
    write(&tmp.path().join("Android/local.properties"), "sdk.dir=/tmp/android-sdk\n");
    write(&tmp.path().join("Android/app/build/outputs/apk/debug/app-debug.apk"), "debug apk");
    write(&tmp.path().join("iOS/.vectis/verify.ok"), "ok\n");
    write(&tmp.path().join("Android/.vectis/verify.ok"), "ok\n");
    write_fresh_core_stamp(tmp.path());

    let result = run(VerifyMode::Verify, tmp.path(), tmp.path()).unwrap();
    let ids = finding_ids(&result);
    assert!(
        ids.contains(&"ios-scaffold-file-drift"),
        "missing boltffi pack apple must drift: {result}"
    );
    assert!(
        ids.contains(&"android-scaffold-file-drift"),
        "missing boltffi pack android must drift: {result}"
    );
}

#[test]
fn catalog_through_verify() {
    let tmp = tempdir().unwrap();
    catalog_project(tmp.path());

    let missing = run(VerifyMode::Verify, tmp.path(), tmp.path()).unwrap();
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

    let imageset = tmp.path().join("iOS/TestApp/Assets.xcassets/empty-state.imageset");
    write(
        &imageset.join("Contents.json"),
        r#"{"images":[{"filename":"empty-state.png"}],"info":{"version":1,"author":"xcode"}}"#,
    );
    write(&imageset.join("empty-state.png"), "materialized");

    let present = run(VerifyMode::Verify, tmp.path(), tmp.path()).unwrap();
    assert!(!finding_ids(&present).contains(&"shell-catalog-entry-missing"));
}
