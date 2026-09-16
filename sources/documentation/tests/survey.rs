//! Asserts what the documentation adapter decides before the SDK's fan-out.
//!
//! How a tree cuts into seams — one `Files` seam per top-level directory that
//! meets the floor, the rest of the tree as one more, and no cut at all when
//! the tree is no finer than itself. The survey is a plain fn over the input:
//! it has no model to ask, so no test needs one.

use std::path::Path;

use emery_sdk::{Seam, SourceContent};

fn workspace(root: &Path) -> SourceContent {
    SourceContent::Workspace(root.display().to_string())
}

// An empty document at each relative path, directories made on the way.
fn tree<const N: usize>(root: &Path, files: [&str; N]) {
    for file in files {
        let path = root.join(file);
        std::fs::create_dir_all(path.parent().expect("a file has a parent"))
            .expect("the directory is created");
        std::fs::write(path, "").expect("the document is written");
    }
}

fn files<const N: usize>(paths: [&str; N]) -> Seam {
    Seam::Files(paths.into_iter().map(str::to_string).collect())
}

// A tree of one directory cuts no finer than itself: the bound tree, whole.
#[test]
fn bound_tree() {
    let root = tempfile::tempdir().expect("a scratch tree");
    tree(root.path(), ["guide/intro.md", "guide/setup.md"]);

    let seams = documentation::survey(&workspace(root.path())).expect("the tree is surveyed");

    assert_eq!(seams, [Seam::Whole]);
}

// Two directories that meet the floor are two seams, each its own
// documents; a one-document directory and the root's own files are the
// third, so no document is left out of the survey.
#[test]
fn two_directories() {
    let root = tempfile::tempdir().expect("a scratch tree");
    tree(
        root.path(),
        [
            "api/orders.md",
            "api/users.md",
            "guide/intro.md",
            "guide/setup.md",
            "notes/todo.md",
            "README.md",
        ],
    );

    let seams = documentation::survey(&workspace(root.path())).expect("the tree is surveyed");

    assert_eq!(
        seams,
        [
            files(["api/orders.md", "api/users.md"]),
            files(["guide/intro.md", "guide/setup.md"]),
            files(["README.md", "notes/todo.md"]),
        ]
    );
}

// The engine's own files, wherever they sit, and dot entries are not
// documentation: no seam names them.
#[test]
fn engine_files() {
    let root = tempfile::tempdir().expect("a scratch tree");
    tree(
        root.path(),
        [
            "api/orders.md",
            "api/users.md",
            "guide/intro.md",
            "guide/setup.md",
            "guide/design.md",
            "spec.md",
            "design.md",
            ".omnia/storage/revision",
            ".github/workflows/ci.yml",
        ],
    );

    let seams = documentation::survey(&workspace(root.path())).expect("the tree is surveyed");

    assert_eq!(
        seams,
        [files(["api/orders.md", "api/users.md"]), files(["guide/intro.md", "guide/setup.md"]),]
    );
}

// An inline value has no tree to cut: the bound value, whole.
#[test]
fn inline_value() {
    let content = SourceContent::Value("Orders are placed over HTTP.".to_string());

    let seams = documentation::survey(&content).expect("a value is surveyed");

    assert_eq!(seams, [Seam::Whole]);
}
