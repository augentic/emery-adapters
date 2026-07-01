//! Core scaffold zero-suppression and clippy-clean integration tests.

use std::process::Command;

use specify_vectis::scaffold::{Versions, parse_caps, plan_core, write_plan};
use tempfile::tempdir;

fn versions() -> Versions {
    Versions::embedded().expect("embedded versions parse")
}

fn plan_app_rs(caps: Option<&str>) -> String {
    let caps = parse_caps(caps).expect("parse caps");
    let plan = plan_core("Counter", "com.vectis.counter", &caps, &versions()).expect("plan core");
    plan.files
        .iter()
        .find(|file| file.relative_path == "shared/src/app.rs")
        .expect("app.rs in core plan")
        .contents
        .clone()
}

fn assert_no_inline_suppressions(app_rs: &str) {
    assert!(!app_rs.contains("#[allow"), "app.rs must not contain #[allow]: {app_rs}");
    assert!(!app_rs.contains("#[expect"), "app.rs must not contain #[expect]: {app_rs}");
}

fn write_core_scaffold(root: &std::path::Path, caps: Option<&str>) {
    let caps = parse_caps(caps).expect("parse caps");
    let plan = plan_core("Counter", "com.vectis.counter", &caps, &versions()).expect("plan core");
    write_plan(root, &plan).expect("write core scaffold");
}

fn run_clippy_d_warnings(root: &std::path::Path) {
    let output = Command::new("cargo")
        .args(["clippy", "--all-targets", "--", "-D", "warnings"])
        .current_dir(root)
        .output()
        .expect("spawn cargo clippy");
    assert!(
        output.status.success(),
        "cargo clippy --all-targets -- -D warnings failed in {}:\nstdout:\n{}\nstderr:\n{}",
        root.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn core_scaffold_app_rs_has_no_inline_suppressions_render_only() {
    assert_no_inline_suppressions(&plan_app_rs(None));
}

#[test]
fn core_scaffold_app_rs_has_no_inline_suppressions_http_kv() {
    assert_no_inline_suppressions(&plan_app_rs(Some("http,kv")));
}

#[test]
fn core_scaffold_clippy_clean_render_only() {
    let dir = tempdir().expect("tempdir");
    write_core_scaffold(dir.path(), None);
    run_clippy_d_warnings(dir.path());
}

#[test]
fn core_scaffold_clippy_clean_http_kv() {
    let dir = tempdir().expect("tempdir");
    write_core_scaffold(dir.path(), Some("http,kv"));
    run_clippy_d_warnings(dir.path());
}
