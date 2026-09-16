//! Asserts what the intent adapter makes of its input before the one model call.
//!
//! The brief it carries whole — inline as the SDK renders it, or read from a
//! one-file tree into a note — and the refusals it fails closed with. The
//! survey is a plain fn over the call's context: it has no model to ask, so
//! no test needs one.

use std::path::Path;

use emery_sdk::{Context, Error, Seam, SourceContent, SourceInput};
use intent::survey::survey;

const BRIEF: &str = "Let users reset passwords by email.";

const fn ctx(input: &SourceInput) -> Context<'_> {
    Context {
        adapter_id: "source:intent",
        input,
    }
}

fn workspace(root: &Path) -> SourceInput {
    SourceInput {
        key: "intent".to_string(),
        content: SourceContent::Workspace(root.display().to_string()),
    }
}

fn value(brief: &str) -> SourceInput {
    SourceInput {
        key: "intent".to_string(),
        content: SourceContent::Value(brief.to_string()),
    }
}

// An inline brief rides the turn as the SDK renders it: the bound value,
// whole.
#[test]
fn inline_value() {
    let input = value(BRIEF);
    let seams = survey(&ctx(&input)).expect("the brief is surveyed");

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

    let input = workspace(root.path());
    let seams = survey(&ctx(&input)).expect("the tree is surveyed");

    let [Seam::Note(note)] = seams.as_slice() else {
        panic!("a one-file tree is one note, got {seams:?}");
    };
    assert!(note.contains(BRIEF), "the located file's contents are the intent string: {note}");
    assert!(note.contains("one-file tree"), "the seam names the tree source: {note}");
}

// No file, or several: a typed refusal before any model call.
#[test]
fn not_one_file() {
    let empty = tempfile::tempdir().expect("a scratch tree");
    let input = workspace(empty.path());
    let none = survey(&ctx(&input));
    assert!(matches!(none, Err(Error::BadRequest { .. })), "got {none:?}");

    let several = tempfile::tempdir().expect("a scratch tree");
    std::fs::write(several.path().join("one.md"), "first").expect("the first file is written");
    std::fs::write(several.path().join("two.md"), "second").expect("the second file is written");
    let input = workspace(several.path());
    let many = survey(&ctx(&input));
    assert!(matches!(many, Err(Error::BadRequest { .. })), "got {many:?}");
}

// An intent source is never legitimately empty: a typed refusal, never an
// empty success.
#[test]
fn empty_brief() {
    let input = value("  \n");
    let inline = survey(&ctx(&input));
    assert!(matches!(inline, Err(Error::BadRequest { .. })), "got {inline:?}");

    let root = tempfile::tempdir().expect("a scratch tree");
    std::fs::write(root.path().join("intent.md"), "\n\t \n").expect("the blank brief is written");
    let input = workspace(root.path());
    let tree = survey(&ctx(&input));
    assert!(matches!(tree, Err(Error::BadRequest { .. })), "got {tree:?}");
}
