//! Unit tests for Crux shell presence heuristics.

use std::path::Path;

use tempfile::tempdir;

use super::{SUPPORTED_SHELL_PLATFORMS, shell_present};

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

#[test]
fn all_supported_present() {
    let tmp = tempdir().unwrap();
    scaffold_core(tmp.path());
    scaffold_ios(tmp.path());
    scaffold_android(tmp.path());

    assert!(shell_present(tmp.path(), "core"));
    assert!(shell_present(tmp.path(), "ios"));
    assert!(shell_present(tmp.path(), "android"));
}

#[test]
fn ios_absent_when_only_core_android() {
    let tmp = tempdir().unwrap();
    scaffold_core(tmp.path());
    scaffold_android(tmp.path());

    assert!(!shell_present(tmp.path(), "ios"));
    assert!(shell_present(tmp.path(), "core"));
    assert!(shell_present(tmp.path(), "android"));
}

#[test]
fn greenfield_all_supported_absent() {
    let tmp = tempdir().unwrap();

    assert!(!shell_present(tmp.path(), "core"));
    assert!(!shell_present(tmp.path(), "ios"));
    assert!(!shell_present(tmp.path(), "android"));
}

#[test]
fn web_desktop_treated_present() {
    let tmp = tempdir().unwrap();
    scaffold_core(tmp.path());

    assert!(shell_present(tmp.path(), "web"));
    assert!(shell_present(tmp.path(), "desktop"));
}

#[test]
fn ios_without_swift_not_present() {
    let tmp = tempdir().unwrap();
    scaffold_core(tmp.path());
    let ios_dir = tmp.path().join("iOS");
    std::fs::create_dir_all(&ios_dir).expect("mkdir iOS");
    std::fs::write(ios_dir.join("README.md"), "placeholder").expect("write readme");

    assert!(!shell_present(tmp.path(), "ios"));
}

#[test]
fn android_without_kt_not_present() {
    let tmp = tempdir().unwrap();
    scaffold_core(tmp.path());
    let android_dir = tmp.path().join("Android");
    std::fs::create_dir_all(&android_dir).expect("mkdir Android");
    std::fs::write(android_dir.join("build.gradle"), "").expect("write gradle");

    assert!(!shell_present(tmp.path(), "android"));
}

#[test]
fn supported_platforms_closed_set() {
    assert_eq!(SUPPORTED_SHELL_PLATFORMS, &["core", "ios", "android"]);
}
