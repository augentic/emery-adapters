//! Documentation's own extract behaviour
//!
//! What the adapter decides before the SDK's fan-out: how a tree cuts into
//! materials — one `Within` per top-level directory that meets the floor,
//! the rest of the tree as one more, and no cut at all when the tree is no
//! finer than itself — and the noun its bound tree goes by in the turn.

use std::path::Path;

use documentation::Adapter;
use emery_sdk::{Context, Material, SourceAdapter as _, SourceContent, SourceInput};
use omnia_test::guest::Scripted;

const fn ctx(input: &SourceInput) -> Context<'_> {
    Context {
        adapter_id: "source:documentation",
        input,
    }
}

fn workspace(root: &Path) -> SourceInput {
    SourceInput {
        key: "docs".to_string(),
        content: SourceContent::Workspace(root.display().to_string()),
    }
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

fn within<const N: usize>(files: [&str; N]) -> Material {
    Material::Within(files.into_iter().map(str::to_string).collect())
}

// A tree of one directory cuts no finer than itself: the bound tree, whole,
// in one turn that names the source by the adapter's noun.
#[tokio::test]
async fn bound_tree() {
    let model = Scripted::answering([r#"{"claims":[]}"#]);
    let root = tempfile::tempdir().expect("a scratch tree");
    tree(root.path(), ["guide/intro.md", "guide/setup.md"]);

    let input = workspace(root.path());
    let evidence =
        Adapter::extract(&model, &ctx(&input)).await.expect("the scripted answer is accepted");

    assert_eq!(evidence.kind, Adapter::KIND);
    let turn = &model.seen()[0].messages[0];
    assert!(turn.contains("the documentation source tree"), "{turn}");
}

// Two directories that meet the floor are two materials, each its own
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

    let input = workspace(root.path());
    let materials = Adapter::survey(&ctx(&input)).expect("the tree is surveyed");

    assert_eq!(
        materials,
        [
            within(["api/orders.md", "api/users.md"]),
            within(["guide/intro.md", "guide/setup.md"]),
            within(["README.md", "notes/todo.md"]),
        ]
    );
}

// The engine's own files, wherever they sit, and dot entries are not
// documentation: no material names them.
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

    let input = workspace(root.path());
    let materials = Adapter::survey(&ctx(&input)).expect("the tree is surveyed");

    assert_eq!(
        materials,
        [within(["api/orders.md", "api/users.md"]), within(["guide/intro.md", "guide/setup.md"]),]
    );
}

// An inline value has no tree to cut: the bound value, whole.
#[test]
fn inline_value() {
    let input = SourceInput {
        key: "docs".to_string(),
        content: SourceContent::Value("Orders are placed over HTTP.".to_string()),
    };

    let materials = Adapter::survey(&ctx(&input)).expect("a value is surveyed");

    assert_eq!(materials, [Material::Bound]);
}
