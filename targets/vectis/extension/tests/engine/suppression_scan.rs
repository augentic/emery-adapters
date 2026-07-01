//! Integration tests for the inline lint-suppression scan.

use std::fmt::Write;
use std::fs;
use std::path::Path;

use specify_vectis::verify::{FINDING_ID, suppression_scan_findings};
use tempfile::tempdir;

fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("mkdir parents");
    }
    fs::write(path, content).expect("write file");
}

fn write_project(root: &Path, platforms: &[&str]) {
    let specify = root.join(".specify");
    fs::create_dir_all(&specify).expect("mkdir .specify");
    let mut platform_lines = String::new();
    for platform in platforms {
        writeln!(platform_lines, "  - {platform}").expect("write platform line");
    }
    fs::write(
        specify.join("project.yaml"),
        format!("name: test-app\nadapter: vectis\nplatforms:\n{platform_lines}"),
    )
    .expect("project.yaml");
}

#[test]
fn clean_trees_emit_no_findings() {
    let tmp = tempdir().unwrap();
    write_project(tmp.path(), &["core", "ios", "android"]);
    write_file(&tmp.path().join("shared/src/app.rs"), "pub struct App;\n");
    write_file(&tmp.path().join("iOS/TestApp/ContentView.swift"), "struct ContentView {}\n");
    write_file(&tmp.path().join("Android/app/src/main/java/com/test/Main.kt"), "class Main\n");

    let findings =
        suppression_scan_findings(tmp.path(), &["core".into(), "ios".into(), "android".into()]);
    assert!(findings.is_empty(), "expected no findings: {findings:?}");
}

#[test]
fn rust_allow_is_forbidden() {
    let tmp = tempdir().unwrap();
    write_project(tmp.path(), &["core"]);
    write_file(&tmp.path().join("shared/src/app.rs"), "#[allow(dead_code)]\npub struct App;\n");

    let findings = suppression_scan_findings(tmp.path(), &["core".into()]);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0]["id"], FINDING_ID);
    assert_eq!(findings[0]["line"], 1);
    assert!(findings[0]["message"].as_str().unwrap().contains("#[allow("));
}

#[test]
fn rust_expect_is_forbidden() {
    let tmp = tempdir().unwrap();
    write_project(tmp.path(), &["core"]);
    write_file(
        &tmp.path().join("shared/src/app.rs"),
        "#[expect(clippy::too_many_lines)]\npub fn update() {}\n",
    );

    let findings = suppression_scan_findings(tmp.path(), &["core".into()]);
    assert_eq!(findings.len(), 1);
    assert!(findings[0]["message"].as_str().unwrap().contains("#[expect("));
}

#[test]
fn swiftlint_disable_is_forbidden() {
    let tmp = tempdir().unwrap();
    write_project(tmp.path(), &["ios"]);
    write_file(
        &tmp.path().join("iOS/TestApp/ContentView.swift"),
        "// swiftlint:disable force_unwrapping\nstruct ContentView {}\n",
    );

    let findings = suppression_scan_findings(tmp.path(), &["ios".into()]);
    assert_eq!(findings.len(), 1);
    assert!(findings[0]["message"].as_str().unwrap().contains("swiftlint:disable"));
}

#[test]
fn swift_format_ignore_is_forbidden() {
    let tmp = tempdir().unwrap();
    write_project(tmp.path(), &["ios"]);
    write_file(
        &tmp.path().join("iOS/TestApp/Core.swift"),
        "struct Core { // swift-format-ignore: legacy\n}\n",
    );

    let findings = suppression_scan_findings(tmp.path(), &["ios".into()]);
    assert_eq!(findings.len(), 1);
    assert!(findings[0]["message"].as_str().unwrap().contains("swift-format-ignore"));
}

#[test]
fn kotlin_suppress_is_forbidden() {
    let tmp = tempdir().unwrap();
    write_project(tmp.path(), &["android"]);
    write_file(
        &tmp.path().join("Android/app/src/main/java/com/test/Main.kt"),
        "@Suppress(\"UNUSED_PARAMETER\")\nclass Main\n",
    );

    let findings = suppression_scan_findings(tmp.path(), &["android".into()]);
    assert_eq!(findings.len(), 1);
    assert!(findings[0]["message"].as_str().unwrap().contains("@Suppress("));
}

#[test]
fn kotlin_file_suppress_is_forbidden() {
    let tmp = tempdir().unwrap();
    write_project(tmp.path(), &["android"]);
    write_file(
        &tmp.path().join("Android/shared/src/Helper.kt"),
        "@file:Suppress(\"MagicNumber\")\npackage com.test\n",
    );

    let findings = suppression_scan_findings(tmp.path(), &["android".into()]);
    assert_eq!(findings.len(), 1);
    assert!(findings[0]["message"].as_str().unwrap().contains("@file:Suppress"));
}

#[test]
fn rust_crate_level_allow_is_forbidden() {
    let tmp = tempdir().unwrap();
    write_project(tmp.path(), &["core"]);
    write_file(&tmp.path().join("shared/src/lib.rs"), "#![allow(dead_code)]\npub mod app;\n");

    let findings = suppression_scan_findings(tmp.path(), &["core".into()]);
    assert_eq!(findings.len(), 1);
    assert!(findings[0]["message"].as_str().unwrap().contains("#![allow("));
}

#[test]
fn rust_string_literal_mention_is_not_forbidden() {
    let tmp = tempdir().unwrap();
    write_project(tmp.path(), &["core"]);
    write_file(
        &tmp.path().join("shared/src/app.rs"),
        "const HELP: &str = \"never use #[allow(dead_code)] in shared/src\";\n",
    );

    let findings = suppression_scan_findings(tmp.path(), &["core".into()]);
    assert!(findings.is_empty(), "string literals must not trip the scan: {findings:?}");
}

#[test]
fn rust_block_comment_mention_is_not_forbidden() {
    let tmp = tempdir().unwrap();
    write_project(tmp.path(), &["core"]);
    write_file(
        &tmp.path().join("shared/src/app.rs"),
        "/* #[allow(dead_code)] is forbidden in agent-authored code */\npub struct App;\n",
    );

    let findings = suppression_scan_findings(tmp.path(), &["core".into()]);
    assert!(findings.is_empty(), "block comments must not trip the scan: {findings:?}");
}

#[test]
fn kotlin_string_literal_mention_is_not_forbidden() {
    let tmp = tempdir().unwrap();
    write_project(tmp.path(), &["android"]);
    write_file(
        &tmp.path().join("Android/app/src/main/java/com/test/Main.kt"),
        "const val HELP = \"@Suppress(\\\"UNUSED\\\") is forbidden\"\n",
    );

    let findings = suppression_scan_findings(tmp.path(), &["android".into()]);
    assert!(findings.is_empty(), "string literals must not trip the scan: {findings:?}");
}

#[test]
fn swift_string_literal_mention_is_not_forbidden() {
    let tmp = tempdir().unwrap();
    write_project(tmp.path(), &["ios"]);
    write_file(
        &tmp.path().join("iOS/TestApp/ContentView.swift"),
        "let help = \"swiftlint:disable is forbidden in shell Swift\"\n",
    );

    let findings = suppression_scan_findings(tmp.path(), &["ios".into()]);
    assert!(findings.is_empty(), "string literals must not trip the scan: {findings:?}");
}

#[test]
fn generated_subtree_is_skipped() {
    let tmp = tempdir().unwrap();
    write_project(tmp.path(), &["ios", "android"]);
    write_file(&tmp.path().join("iOS/generated/Stub.swift"), "// swiftlint:disable everything\n");
    write_file(&tmp.path().join("Android/app/src/generated/Bindings.kt"), "@Suppress(\"ALL\")\n");
    write_file(&tmp.path().join("iOS/TestApp/ContentView.swift"), "struct ContentView {}\n");
    write_file(&tmp.path().join("Android/app/src/main/java/com/test/Main.kt"), "class Main\n");

    let findings = suppression_scan_findings(tmp.path(), &["ios".into(), "android".into()]);
    assert!(findings.is_empty(), "generated/ trees must be skipped: {findings:?}");
}
