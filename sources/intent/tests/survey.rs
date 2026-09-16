//! Asserts what the intent adapter makes of its input before the one model call.
//!
//! The brief it carries whole — inline as the SDK renders it, or read from a
//! one-file tree into a note — and the refusals it fails closed with. The
//! survey is a plain fn over the input: it has no model to ask, so no test
//! needs one.

use std::path::Path;

use emery_sdk::{Error, Seam, SourceInput};
use intent::survey::survey;

const KEY: &str = "intent";
const BRIEF: &str = "Let users reset passwords by email.";

// The scratch root as the engine lends one: a string.
fn utf8(root: &Path) -> &str {
    root.to_str().expect("a UTF-8 scratch root")
}

// An inline brief rides the turn as the SDK renders it: the bound value,
// whole.
#[test]
fn inline_value() {
    let input = SourceInput::value(KEY, BRIEF);
    let seams = survey(&input).expect("the brief is surveyed");

    assert_eq!(seams, [Seam::Whole]);
}

// Nested or not, the one file is read into the turn's seam: a note carrying
// the brief and naming the tree it came from.
#[test]
fn one_file() {
    let root = tempfile::tempdir().expect("a scratch tree");
    let nested = root.path().join("nested");
    std::fs::create_dir(&nested).expect("the nested directory is created");
    std::fs::write(nested.join("intent.md"), BRIEF).expect("the brief is written");

    let input = SourceInput::workspace(KEY, utf8(root.path()));
    let seams = survey(&input).expect("the tree is surveyed");

    let [Seam::Note(note)] = seams.as_slice() else {
        panic!("a one-file tree is one note, got {seams:?}");
    };
    assert!(note.contains(BRIEF), "the located file's contents are the intent string: {note}");
    assert!(note.contains("one-file tree"), "the seam names the tree source: {note}");
}

// The engine's own files beside the brief — a projection of the last
// revision, its store — are output, not input: the tree is still one file,
// and the brief is the one read.
#[test]
fn engine_files() {
    let root = tempfile::tempdir().expect("a scratch tree");
    std::fs::write(root.path().join("intent.md"), BRIEF).expect("the brief is written");
    std::fs::write(root.path().join("spec.md"), "# Spec").expect("the projection is written");
    std::fs::write(root.path().join("design.md"), "# Design").expect("the projection is written");
    std::fs::create_dir(root.path().join(".omnia")).expect("the store is created");
    std::fs::write(root.path().join(".omnia/store.json"), "{}").expect("the store is written");

    let input = SourceInput::workspace(KEY, utf8(root.path()));
    let seams = survey(&input).expect("the tree is surveyed");

    let [Seam::Note(note)] = seams.as_slice() else {
        panic!("a one-file tree is one note, got {seams:?}");
    };
    assert!(note.contains(BRIEF), "the brief is the one file read: {note}");
    assert!(!note.contains("# Spec"), "the projection is not the brief: {note}");
}

// No file, or several: a typed refusal before any model call.
#[test]
fn not_one_file() {
    let empty = tempfile::tempdir().expect("a scratch tree");
    let input = SourceInput::workspace(KEY, utf8(empty.path()));
    let none = survey(&input);
    assert!(matches!(none, Err(Error::BadRequest { .. })), "got {none:?}");

    let several = tempfile::tempdir().expect("a scratch tree");
    std::fs::write(several.path().join("one.md"), "first").expect("the first file is written");
    std::fs::write(several.path().join("two.md"), "second").expect("the second file is written");
    let input = SourceInput::workspace(KEY, utf8(several.path()));
    let many = survey(&input);
    assert!(matches!(many, Err(Error::BadRequest { .. })), "got {many:?}");
}

// An intent source is never legitimately empty: a typed refusal, never an
// empty success.
#[test]
fn empty_brief() {
    let input = SourceInput::value(KEY, "  \n");
    let inline = survey(&input);
    assert!(matches!(inline, Err(Error::BadRequest { .. })), "got {inline:?}");

    let root = tempfile::tempdir().expect("a scratch tree");
    std::fs::write(root.path().join("intent.md"), "\n\t \n").expect("the blank brief is written");
    let input = SourceInput::workspace(KEY, utf8(root.path()));
    let tree = survey(&input);
    assert!(matches!(tree, Err(Error::BadRequest { .. })), "got {tree:?}");
}
