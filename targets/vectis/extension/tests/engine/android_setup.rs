//! Integration tests for `vectis android setup`.

use std::path::Path;
use std::sync::{Mutex, MutexGuard, OnceLock};

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

fn env_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(())).lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[test]
fn android_setup_uses_project_dir_from_android_cwd() {
    let _guard = env_lock();
    let tmp = tempdir().expect("tempdir");
    write_project_yaml(tmp.path(), &["core", "android"]);
    setup_android_shell(tmp.path());

    let previous_dir = std::env::current_dir().expect("cwd");
    std::env::set_current_dir(tmp.path().join("Android")).expect("chdir Android");

    let previous_project_dir = std::env::var_os("PROJECT_DIR");
    #[expect(unsafe_code, reason = "edition-2024 set_var is unsafe; env_lock serializes access")]
    // SAFETY: this test serializes PROJECT_DIR mutation with `env_lock`.
    let () = unsafe { std::env::set_var("PROJECT_DIR", tmp.path()) };

    let payload = run(&AndroidCommand::Setup(AndroidSetupArgs { path: None })).expect("setup");

    #[expect(
        unsafe_code,
        reason = "edition-2024 set_var/remove_var are unsafe; env_lock serializes access"
    )]
    // SAFETY: this test serializes PROJECT_DIR mutation with `env_lock`.
    unsafe {
        match previous_project_dir {
            Some(value) => std::env::set_var("PROJECT_DIR", value),
            None => std::env::remove_var("PROJECT_DIR"),
        }
    }
    std::env::set_current_dir(previous_dir).expect("restore cwd");

    assert_eq!(setup_exit_code(&payload), 0);
    assert!(tmp.path().join("Android/gradlew").is_file());
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

#[test]
fn android_setup_rejects_partial_wrapper() {
    let tmp = tempdir().expect("tempdir");
    setup_android_shell(tmp.path());
    std::fs::write(tmp.path().join("Android/gradlew"), b"#!/bin/sh\n").expect("gradlew");

    let payload = run_for_shell_dir(&tmp.path().join("Android"));
    assert_eq!(setup_exit_code(&payload), 1);
    let findings = payload["findings"].as_array().expect("findings");
    assert_eq!(findings[0]["id"], "android-setup-wrapper-failed");
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
    let core_dir = tmp.path().join("shared/src");
    std::fs::create_dir_all(&core_dir).expect("mkdir shared/src");
    std::fs::write(core_dir.join("app.rs"), "pub struct App;").expect("write app.rs");
    crate::engine_support::scaffold_android_verify_ready(tmp.path());

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
