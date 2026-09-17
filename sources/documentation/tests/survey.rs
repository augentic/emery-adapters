//! Asserts what the documentation adapter decides before the SDK's fan-out.
//!
//! How a tree cuts into seams — one `Files` seam per top-level directory that
//! meets the floor, nested documents staying with that directory, the root's
//! own files and any directory beneath the floor folding into `.`, dot
//! entries left out — and no cut at all when the input is no finer than
//! itself. The survey is a plain fn over the `SourceInput`: it has no model
//! to ask, so no test needs one. What the SDK's listing does for every
//! adapter — the engine's own files pruned, an unreadable tree refused — is
//! the SDK suite's, not this one's.

use documentation::survey::survey;
use emery_sdk::{Seam, SourceInput};

const KEY: &str = "docs";

// An inline value has no tree to cut: the bound value, whole.
#[test]
fn inline_value() {
    let input = SourceInput::value(KEY, "Orders are placed over HTTP.");
    let seams = survey(&input).expect("a value is surveyed");

    assert_eq!(seams, [Seam::Whole]);
}

// A tree of one directory cuts no finer than itself: the bound tree, whole,
// rather than one seam that is the tree again.
#[test]
fn one_directory() {
    let seams = cut(["guide/intro.md", "guide/setup.md"]);

    assert_eq!(seams, [Seam::Whole]);
}

// The cut is the first path segment. Each top-level directory that meets the
// floor is a seam of its own documents, nested ones included — `guide/advanced/`
// meets the floor by itself but is no seam; the root's own file and a
// directory of one fold into `.`, so every document is in exactly one seam.
#[test]
fn directories() {
    let seams = cut([
        "README.md",
        "api/orders.md",
        "api/users.md",
        "guide/advanced/setup.md",
        "guide/advanced/topics.md",
        "guide/intro.md",
        "notes/todo.md",
    ]);

    assert_eq!(
        seams,
        [
            files(["README.md", "notes/todo.md"]),
            files(["api/orders.md", "api/users.md"]),
            files(["guide/advanced/setup.md", "guide/advanced/topics.md", "guide/intro.md"]),
        ]
    );
}

// A dot entry is tooling, not documentation: a dot directory is not entered
// and a dot file is not listed, at the root or beneath a directory — so
// neither folds into `.` nor pads a seam.
#[test]
fn dot_entries() {
    let seams = cut([
        ".github/workflows/ci.yml",
        "api/orders.md",
        "api/users.md",
        "guide/.draft.md",
        "guide/intro.md",
        "guide/setup.md",
    ]);

    assert_eq!(
        seams,
        [files(["api/orders.md", "api/users.md"]), files(["guide/intro.md", "guide/setup.md"]),]
    );
}

// Surveys a scratch tree of empty documents at these root-relative paths, as
// the engine lends it.
fn cut<const N: usize>(files: [&str; N]) -> Vec<Seam> {
    let scratch = tempfile::tempdir().expect("a scratch tree");
    for file in files {
        let path = scratch.path().join(file);
        std::fs::create_dir_all(path.parent().expect("a file has a parent"))
            .expect("the directory is created");
        std::fs::write(path, "").expect("the document is written");
    }

    let root = scratch.path().to_str().expect("a UTF-8 scratch root");
    survey(&SourceInput::workspace(KEY, root)).expect("the tree is surveyed")
}

fn files<const N: usize>(paths: [&str; N]) -> Seam {
    Seam::Files(paths.into_iter().map(str::to_string).collect())
}
