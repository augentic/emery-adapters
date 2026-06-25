//! Integration tests for `vectis android setup`.

use std::path::Path;

use serde_json::Value;
use specify_vectis::android::{
    AndroidCommand, AndroidSetupArgs, run, run_for_shell_dir, setup_exit_code,
};
use tempfile::tempdir;

use crate::engine_support::write_project_yaml;

fn setup_android_shell(root: &Path) {
    let dir = root.join("Android/app/src/main/kotlin/com/test");
    std::fs::create_dir_all(&dir).expect("mkdir Android");
    std::fs::write(dir.join("MainActivity.kt"), "class MainActivity").expect("write kt");
    std::fs::write(root.join("Android/gradle.properties"), "android.useAndroidX=true\n")
        .expect("write gradle.properties");
}

#[test]
fn android_setup_installs_vendored_wrapper() {
    let tmp = tempdir().expect("tempdir");
    setup_android_shell(tmp.path());

    let payload = run_for_shell_dir(&tmp.path().join("Android"));
    assert_eq!(setup_exit_code(&payload), 0);
    assert!(tmp.path().join("Android/gradlew").is_file());
    assert!(tmp.path().join("Android/gradle/wrapper/gradle-wrapper.jar").is_file());

    let again = run_for_shell_dir(&tmp.path().join("Android"));
    assert_eq!(setup_exit_code(&again), 0);
    let actions = again["actions"].as_array().expect("actions");
    assert!(actions.iter().any(|a| a["status"] == "skipped"));
}

#[test]
fn android_setup_command_resolves_project_root() {
    let tmp = tempdir().expect("tempdir");
    write_project_yaml(tmp.path(), &["core", "android"]);
    setup_android_shell(tmp.path());

    let payload = run(&AndroidCommand::Setup(AndroidSetupArgs {
        path: Some(tmp.path().to_path_buf()),
    }))
    .expect("setup");
    assert_eq!(setup_exit_code(&payload), 0);
}

#[test]
fn android_setup_errors_when_android_dir_missing() {
    let tmp = tempdir().expect("tempdir");
    write_project_yaml(tmp.path(), &["android"]);

    let err = run(&AndroidCommand::Setup(AndroidSetupArgs {
        path: Some(tmp.path().to_path_buf()),
    }))
    .expect_err("missing Android/");
    assert_eq!(err.exit_code(), 2);
}

fn toolchain_finding_ids(value: &Value) -> Vec<&str> {
    value["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .filter_map(|f| f["id"].as_str())
        .collect()
}

fn parse_verify_output(rendered: &str, code: u8) -> (Value, u8) {
    let value: Value = serde_json::from_str(rendered).expect("verify output is JSON");
    (value, code)
}

#[test]
fn verify_android_toolchain_findings_when_shell_incomplete() {
    use specify_vectis::verify::{VerifyArgs, VerifyMode, render_json, run as run_verify};

    let tmp = tempdir().expect("tempdir");
    write_project_yaml(tmp.path(), &["core", "android"]);
    setup_android_shell(tmp.path());

    let (rendered, code) = render_json(run_verify(&VerifyArgs {
        mode: VerifyMode::Verify,
        path: Some(tmp.path().to_path_buf()),
    }));
    let (value, code) = parse_verify_output(&rendered, code);
    assert_eq!(code, 1, "stdout: {value}");
    let ids = toolchain_finding_ids(&value);
    assert!(ids.contains(&"android-gradlew-missing"));
    assert!(ids.contains(&"android-apk-missing"));
}

#[test]
fn verify_android_toolchain_clean_after_setup_and_apk() {
    use specify_vectis::verify::{VerifyArgs, VerifyMode, render_json, run as run_verify};

    let tmp = tempdir().expect("tempdir");
    write_project_yaml(tmp.path(), &["core", "android"]);
    crate::engine_support::scaffold_android_shell(tmp.path());
    let core_dir = tmp.path().join("shared/src");
    std::fs::create_dir_all(&core_dir).expect("mkdir shared/src");
    std::fs::write(core_dir.join("app.rs"), "pub struct App;").expect("write app.rs");

    let _unused = run_for_shell_dir(&tmp.path().join("Android"));
    std::fs::write(tmp.path().join("Android/local.properties"), "sdk.dir=/tmp/android-sdk\n")
        .expect("local.properties");
    std::fs::write(
        tmp.path().join("Android/gradle.properties"),
        "android.useAndroidX=true\norg.gradle.java.home=/Library/Java/JavaVirtualMachines/temurin-21.jdk/Contents/Home\n",
    )
    .expect("gradle.properties");
    let shared_build = tmp.path().join("Android/shared/build.gradle.kts");
    std::fs::create_dir_all(shared_build.parent().expect("parent")).expect("shared dir");
    std::fs::write(&shared_build, "ndkVersion = \"26.1.10909125\"\n").expect("shared build");
    let apk_parent = tmp.path().join("Android/app/build/outputs/apk/debug");
    std::fs::create_dir_all(&apk_parent).expect("apk dir");
    std::fs::write(apk_parent.join("app-debug.apk"), b"PK").expect("apk");

    let (rendered, code) = render_json(run_verify(&VerifyArgs {
        mode: VerifyMode::Verify,
        path: Some(tmp.path().to_path_buf()),
    }));
    let (value, code) = parse_verify_output(&rendered, code);
    let ids = toolchain_finding_ids(&value);
    assert!(
        !ids.iter().any(|id| id.starts_with("android-")),
        "unexpected android toolchain findings: {ids:?} full={value}"
    );
    assert_eq!(code, 0);
}
