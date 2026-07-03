//! Android scaffold zero-suppression, strict Gradle flags, and sync/drift tests.

use std::fs;

use specify_vectis::android_scaffold::{
    DRIFT_FINDING_ID, REQUIRED_GRADLE_ALL_WARNINGS_AS_ERRORS, REQUIRED_JAVA_COMPILE_WERROR,
    REQUIRED_MAKEFILE_RUSTFLAGS, android_scaffold_drift_findings, resolve_android_app_name,
    resolve_android_package, sync_android_scaffold_files,
};
use specify_vectis::scaffold::{ScaffoldPlan, Versions, parse_caps, plan_android};
use tempfile::tempdir;

use crate::engine_support::{ProjectDirGuard, env_lock};

fn versions() -> Versions {
    Versions::embedded().expect("embedded versions parse")
}

fn plan(caps: Option<&str>) -> ScaffoldPlan {
    let caps = parse_caps(caps).expect("parse caps");
    plan_android("Counter", "com.vectis.counter", &caps, &versions()).expect("plan android")
}

fn kotlin_sources(plan: &ScaffoldPlan) -> impl Iterator<Item = &str> {
    plan.files
        .iter()
        .filter(|file| {
            std::path::Path::new(&file.relative_path)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("kt"))
        })
        .map(|file| file.contents.as_str())
}

fn assert_no_suppress(plan: &ScaffoldPlan) {
    for contents in kotlin_sources(plan) {
        assert!(
            !contents.contains("@Suppress"),
            "Kotlin source must not contain @Suppress:\n{contents}"
        );
    }
}

fn assert_gradle_strict_flags(plan: &ScaffoldPlan) {
    let app = plan
        .files
        .iter()
        .find(|file| file.relative_path == "Android/app/build.gradle.kts")
        .unwrap_or_else(|| panic!("Android/app/build.gradle.kts missing from android plan"))
        .contents
        .as_str();
    assert!(
        app.contains(REQUIRED_GRADLE_ALL_WARNINGS_AS_ERRORS),
        "Android/app/build.gradle.kts must set allWarningsAsErrors = true:\n{app}"
    );
    assert!(
        app.contains(REQUIRED_JAVA_COMPILE_WERROR),
        "Android/app/build.gradle.kts must add JavaCompile -Werror:\n{app}"
    );

    let shared = plan
        .files
        .iter()
        .find(|file| file.relative_path == "Android/shared/build.gradle.kts")
        .unwrap_or_else(|| panic!("Android/shared/build.gradle.kts missing from android plan"))
        .contents
        .as_str();
    assert!(
        !shared.contains(REQUIRED_GRADLE_ALL_WARNINGS_AS_ERRORS),
        "Android/shared/build.gradle.kts must not set allWarningsAsErrors (generated UniFFI only):\n{shared}"
    );
    assert!(
        !shared.contains(REQUIRED_JAVA_COMPILE_WERROR),
        "Android/shared/build.gradle.kts must not add JavaCompile -Werror (generated UniFFI only):\n{shared}"
    );
}

fn assert_shared_cargo_extension_profile(plan: &ScaffoldPlan) {
    let contents = plan
        .files
        .iter()
        .find(|file| file.relative_path == "Android/shared/build.gradle.kts")
        .unwrap_or_else(|| panic!("Android/shared/build.gradle.kts missing from android plan"))
        .contents
        .as_str();
    assert!(
        contents.contains("profile = \"debug\""),
        "Android/shared/build.gradle.kts must set profile = \"debug\" in CargoExtension:\n{contents}"
    );
    let cargo_block = contents
        .split("extensions.configure<CargoExtension>")
        .nth(1)
        .unwrap_or_else(|| panic!("CargoExtension block missing from shared build.gradle.kts:\n{contents}"));
    assert!(
        !cargo_block.contains("adapter = \"debug\""),
        "CargoExtension block must not use invalid adapter = \"debug\" property:\n{cargo_block}"
    );
}

fn assert_makefile_strict_rustflags(plan: &ScaffoldPlan) {
    let contents = plan
        .files
        .iter()
        .find(|file| file.relative_path == "Android/Makefile")
        .unwrap_or_else(|| panic!("Android/Makefile missing from android plan"))
        .contents
        .as_str();
    assert!(
        contents.contains(REQUIRED_MAKEFILE_RUSTFLAGS),
        "Android/Makefile must prefix cargo with {REQUIRED_MAKEFILE_RUSTFLAGS}:\n{contents}"
    );
    assert!(
        !contents.contains("android setup .."),
        "Android/Makefile setup-extension must invoke `android setup` without a `..` path (PROJECT_DIR is used instead):\n{contents}"
    );
}

fn write_minimal_android_tree(root: &std::path::Path, app_name: &str, package: &str) {
    let android = root.join("Android");
    let package_path = package.replace('.', "/");
    let kotlin_dir = android.join(format!("app/src/main/java/{package_path}"));
    fs::create_dir_all(&kotlin_dir).expect("kotlin dir");
    fs::write(android.join("settings.gradle.kts"), format!("rootProject.name = \"{app_name}\"\n"))
        .expect("settings.gradle");
    fs::write(android.join("app/build.gradle.kts"), format!("namespace = \"{package}\"\n"))
        .expect("app build.gradle");
    fs::write(
        kotlin_dir.join(format!("{app_name}Application.kt")),
        format!("package {package}\nclass {app_name}Application\n"),
    )
    .expect("application kt");
}

#[test]
fn android_scaffold_kt_has_no_suppress_render_only() {
    assert_no_suppress(&plan(None));
}

#[test]
fn android_scaffold_kt_has_no_suppress_http_kv_time_platform() {
    assert_no_suppress(&plan(Some("http,kv,time,platform")));
}

#[test]
fn android_scaffold_gradle_treats_warnings_as_errors_render_only() {
    assert_gradle_strict_flags(&plan(None));
    assert_shared_cargo_extension_profile(&plan(None));
}

#[test]
fn android_scaffold_gradle_treats_warnings_as_errors_http() {
    assert_gradle_strict_flags(&plan(Some("http")));
    assert_shared_cargo_extension_profile(&plan(Some("http")));
}

#[test]
fn android_scaffold_makefile_uses_strict_rustflags_render_only() {
    assert_makefile_strict_rustflags(&plan(None));
}

#[test]
fn sync_restores_drifted_gradle_without_strict_flags() {
    let dir = tempdir().unwrap();
    write_minimal_android_tree(dir.path(), "Counter", "com.vectis.counter");
    fs::write(
        dir.path().join("Android/app/build.gradle.kts"),
        "namespace = \"com.vectis.counter\"\n",
    )
    .expect("drifted app build.gradle");

    let report = sync_android_scaffold_files(dir.path()).expect("sync");
    assert!(report.synced.iter().any(|p| p == "Android/app/build.gradle.kts"));

    let restored =
        fs::read_to_string(dir.path().join("Android/app/build.gradle.kts")).expect("read gradle");
    assert!(restored.contains(REQUIRED_GRADLE_ALL_WARNINGS_AS_ERRORS));
    assert!(restored.contains(REQUIRED_JAVA_COMPILE_WERROR));
}

#[test]
fn sync_preserves_substituted_ndk_version_in_shared_build() {
    let dir = tempdir().unwrap();
    write_minimal_android_tree(dir.path(), "Counter", "com.vectis.counter");
    sync_android_scaffold_files(dir.path()).expect("initial sync");

    let shared_build = dir.path().join("Android/shared/build.gradle.kts");
    let mut contents = fs::read_to_string(&shared_build).expect("read shared build");
    contents = contents.replace("__ANDROID_NDK_VERSION__", "26.1.10909125");
    fs::write(&shared_build, &contents).expect("write substituted ndk");

    let report = sync_android_scaffold_files(dir.path()).expect("second sync");
    assert!(
        report.unchanged.iter().any(|p| p == "Android/shared/build.gradle.kts"),
        "sync must not revert host-substituted NDK pin: {report:?}"
    );
    let on_disk = fs::read_to_string(&shared_build).expect("read shared build");
    assert!(on_disk.contains("26.1.10909125"));
}

#[test]
fn drift_findings_flag_missing_gradle_strict_flags() {
    let dir = tempdir().unwrap();
    write_minimal_android_tree(dir.path(), "Counter", "com.vectis.counter");
    fs::write(
        dir.path().join("Android/app/build.gradle.kts"),
        "namespace = \"com.vectis.counter\"\n",
    )
    .expect("drifted app build.gradle");

    let findings = android_scaffold_drift_findings(dir.path());
    assert!(
        findings.iter().any(|f| {
            f["path"] == "Android/app/build.gradle.kts"
                && f["message"].as_str().unwrap().contains("allWarningsAsErrors")
        }),
        "expected strict Gradle hint: {findings:?}"
    );
}

#[test]
fn resolve_app_name_from_settings_gradle() {
    let dir = tempdir().unwrap();
    write_minimal_android_tree(dir.path(), "TodoApp", "com.vectis.todoapp");

    assert_eq!(resolve_android_app_name(dir.path()).expect("app name"), "TodoApp");
}

#[test]
fn resolve_app_name_falls_back_to_application_kt() {
    let dir = tempdir().unwrap();
    let android = dir.path().join("Android");
    let package_path = "com/vectis/todoapp";
    fs::create_dir_all(android.join(format!("app/src/main/java/{package_path}")))
        .expect("kotlin dir");
    fs::write(android.join("settings.gradle.kts"), "rootProject.name = \"not-valid\"\n")
        .expect("settings.gradle");
    fs::write(
        android.join(format!("app/src/main/java/{package_path}/TodoAppApplication.kt")),
        "package com.vectis.todoapp\n",
    )
    .expect("application kt");
    fs::write(android.join("app/build.gradle.kts"), "namespace = \"com.vectis.todoapp\"\n")
        .expect("app build.gradle");

    assert_eq!(resolve_android_app_name(dir.path()).expect("app name"), "TodoApp");
}

#[test]
fn resolve_package_from_app_build_gradle() {
    let dir = tempdir().unwrap();
    write_minimal_android_tree(dir.path(), "Counter", "com.vectis.counter");

    assert_eq!(
        resolve_android_package(dir.path(), "Counter").expect("package"),
        "com.vectis.counter"
    );
}

#[test]
fn sync_is_noop_when_files_match_template() {
    let dir = tempdir().unwrap();
    sync_android_scaffold_files(dir.path()).expect("noop sync without android dir");

    write_minimal_android_tree(dir.path(), "Counter", "com.vectis.counter");
    let first = sync_android_scaffold_files(dir.path()).expect("first sync");
    assert!(first.synced.iter().any(|p| p == "Android/Makefile"));

    let makefile = fs::read_to_string(dir.path().join("Android/Makefile")).expect("read makefile");
    let second = sync_android_scaffold_files(dir.path()).expect("second sync");
    assert!(second.synced.is_empty());
    assert!(second.unchanged.iter().any(|p| p == "Android/Makefile"));
    assert_eq!(
        fs::read_to_string(dir.path().join("Android/Makefile")).expect("read makefile"),
        makefile
    );
}

#[test]
fn drift_findings_flag_missing_makefile() {
    let dir = tempdir().unwrap();
    write_minimal_android_tree(dir.path(), "Counter", "com.vectis.counter");

    let findings = android_scaffold_drift_findings(dir.path());
    assert!(
        findings.iter().any(|f| f["path"] == "Android/Makefile"),
        "expected missing Makefile finding: {findings:?}"
    );
    assert!(findings.iter().all(|f| f["id"] == DRIFT_FINDING_ID));
}

#[test]
fn sync_android_scaffold_command_restores_drifted_makefile() {
    let _guard = env_lock();
    let tmp = tempdir().unwrap();
    let project = tmp.path().join("project");
    fs::create_dir_all(project.join(".specify")).expect("specify dir");
    fs::write(project.join(".specify/project.yaml"), "platforms:\n  - core\n  - android\n")
        .expect("project yaml");
    write_minimal_android_tree(&project, "Counter", "com.vectis.counter");
    fs::write(project.join("Android/Makefile"), "verify:\n\t@echo drifted\n")
        .expect("drifted makefile");

    let _project_dir = ProjectDirGuard::set(&project);

    let outcome = specify_vectis::sync::run(&specify_vectis::sync::SyncCommand::AndroidScaffold(
        specify_vectis::sync::AndroidScaffoldArgs { path: None },
    ))
    .expect("sync android-scaffold");

    let synced = outcome["scaffold_sync"]["android"]["synced"].as_array().expect("synced array");
    assert!(synced.iter().any(|v| v == "Android/Makefile"));

    let restored = fs::read_to_string(project.join("Android/Makefile")).expect("read makefile");
    assert!(restored.contains(REQUIRED_MAKEFILE_RUSTFLAGS));
    assert!(!restored.contains("drifted"));
}
