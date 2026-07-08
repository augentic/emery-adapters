//! The dev-only prose overlay (`--features prose-overlay`): overlay
//! bodies win, misses fall back to the embedded table, and the doc set
//! never changes.

#![cfg(feature = "prose-overlay")]

use std::fs;

use adapter::registry::{Doc, body, find, resolve};
use tempfile::TempDir;

/// A sorted table, as the `prose` codegen emits.
static DOCS: &[Doc] = &[
    Doc {
        path: "prompts/build.md",
        body: "# embedded build",
    },
    Doc {
        path: "references/verifier.md",
        body: "# embedded verifier",
    },
];

// Rebase the process cwd into a fresh tempdir seeded with `.eval/prose/`
// overlay files — the overlay resolves against the cwd. Mutating the cwd
// is safe here because `cargo make test` runs under nextest with
// process-per-test isolation. The returned guard keeps the tree alive.
fn enter_overlay(files: &[(&str, &str)]) -> TempDir {
    let dir = tempfile::tempdir().expect("create tempdir");
    for (path, contents) in files {
        let file = dir.path().join(".eval/prose").join(path);
        let parent = file.parent().expect("overlay files sit under .eval/prose");
        fs::create_dir_all(parent).expect("create overlay tree");
        fs::write(&file, contents).expect("write overlay file");
    }
    std::env::set_current_dir(dir.path()).expect("enter tempdir");
    dir
}

// Same cwd rebase, but with a directory squatting on the overlay path —
// present but unreadable as a file.
fn enter_overlay_with_dir_at(path: &str) -> TempDir {
    let dir = tempfile::tempdir().expect("create tempdir");
    fs::create_dir_all(dir.path().join(".eval/prose").join(path)).expect("create overlay dir");
    std::env::set_current_dir(dir.path()).expect("enter tempdir");
    dir
}

#[test]
fn overlay_body_wins() {
    let _dir = enter_overlay(&[("prompts/build.md", "# overlaid build")]);
    assert_eq!(body(DOCS, "prompts/build.md"), "# overlaid build");
    assert_eq!(resolve(DOCS, "prompts/build.md"), Some("# overlaid build"));
}

#[test]
fn absent_overlay_serves_embedded() {
    let _dir = enter_overlay(&[("prompts/build.md", "# overlaid build")]);
    assert_eq!(body(DOCS, "references/verifier.md"), "# embedded verifier");
    assert_eq!(resolve(DOCS, "references/verifier.md"), Some("# embedded verifier"));
}

// An empty overlay file is served as-is by design: `read_to_string`
// reads to EOF, so a partial read cannot masquerade as an empty body.
#[test]
fn empty_overlay_file_serves_empty_body() {
    let _dir = enter_overlay(&[("prompts/build.md", "")]);
    assert_eq!(body(DOCS, "prompts/build.md"), "");
}

// A present-but-unreadable overlay path (a directory here) must fail
// loud rather than silently fall back to the embedded body.
#[test]
#[should_panic(expected = "is unreadable")]
fn unreadable_overlay_file_panics() {
    let _dir = enter_overlay_with_dir_at("prompts/build.md");
    let _ = body(DOCS, "prompts/build.md");
}

#[test]
#[should_panic(expected = "document `prompts/missing.md` is not embedded")]
fn miss_in_both_panics() {
    let _dir = enter_overlay(&[("prompts/build.md", "# overlaid build")]);
    let _ = body(DOCS, "prompts/missing.md");
}

// An overlay file for a path outside the embedded table never extends
// the doc set: existence is always the table's.
#[test]
fn overlay_never_adds_entries() {
    let _dir = enter_overlay(&[("prompts/extra.md", "# not in the table")]);
    assert!(find(DOCS, "prompts/extra.md").is_none());
    assert_eq!(resolve(DOCS, "prompts/extra.md"), None);
}

#[test]
#[should_panic(expected = "document `prompts/extra.md` is not embedded")]
fn overlay_only_path_keeps_the_panic_contract() {
    let _dir = enter_overlay(&[("prompts/extra.md", "# not in the table")]);
    let _ = body(DOCS, "prompts/extra.md");
}
