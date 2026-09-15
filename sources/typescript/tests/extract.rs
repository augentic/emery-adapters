//! TypeScript's own extract behaviour
//!
//! What the adapter decides before the SDK's fan-out: how a tree cuts into
//! materials — one note per top-level directory that meets the floor, the
//! rest of the tree as one more, each naming its files and lent the root; no
//! cut at all when the tree is no finer than itself; and which entries are
//! not production source — and the noun its bound tree goes by in the turn.

use std::path::Path;

use emery_sdk::{Context, Material, SourceAdapter as _, SourceContent, SourceInput};
use omnia_test::guest::Scripted;
use typescript::Adapter;

const fn ctx(input: &SourceInput) -> Context<'_> {
    Context {
        adapter_id: "source:typescript",
        input,
    }
}

fn workspace(root: &Path) -> SourceInput {
    SourceInput {
        key: "legacy-monolith".to_string(),
        content: SourceContent::Workspace(root.display().to_string()),
    }
}

// An empty module at each relative path, directories made on the way.
fn tree<const N: usize>(root: &Path, files: [&str; N]) {
    for file in files {
        let path = root.join(file);
        std::fs::create_dir_all(path.parent().expect("a file has a parent"))
            .expect("the directory is created");
        std::fs::write(path, "").expect("the module is written");
    }
}

// Each material's note; anything else is not this adapter's survey.
fn notes(materials: &[Material]) -> Vec<&str> {
    materials
        .iter()
        .map(|material| match material {
            Material::Prepared(note) => note.as_str(),
            other => panic!("a typescript material is a prepared note, got {other:?}"),
        })
        .collect()
}

// The files a note names to mine, in order.
fn named(note: &str) -> Vec<&str> {
    note.lines().filter_map(|line| line.strip_prefix("- `")?.strip_suffix('`')).collect()
}

// A tree of one directory cuts no finer than itself: the bound tree, whole,
// in one turn that names the source by the adapter's noun.
#[tokio::test]
async fn bound_tree() {
    let model = Scripted::answering([r#"{"claims":[]}"#]);
    let root = tempfile::tempdir().expect("a scratch tree");
    tree(root.path(), ["src/index.ts", "src/server.ts"]);

    let input = workspace(root.path());
    let evidence =
        Adapter::extract(&model, &ctx(&input)).await.expect("the scripted answer is accepted");

    assert_eq!(evidence.kind, Adapter::KIND);
    let turn = &model.seen()[0].messages[0];
    assert!(turn.contains("the TypeScript / JavaScript source tree"), "{turn}");
}

// Two directories that meet the floor are two notes, each naming its own
// modules; a one-module directory and the root's own files are the third.
// Every note lends the root, so an import into a sibling directory resolves.
#[test]
fn two_directories() {
    let root = tempfile::tempdir().expect("a scratch tree");
    tree(
        root.path(),
        [
            "routes/orders.ts",
            "routes/users.ts",
            "services/mail.ts",
            "services/orders.ts",
            "models/order.ts",
            "index.ts",
        ],
    );

    let input = workspace(root.path());
    let materials = Adapter::survey(&ctx(&input)).expect("the tree is surveyed");

    let notes = notes(&materials);
    let named: Vec<_> = notes.iter().map(|note| named(note)).collect();
    assert_eq!(
        named,
        [
            vec!["routes/orders.ts", "routes/users.ts"],
            vec!["services/mail.ts", "services/orders.ts"],
            vec!["index.ts", "models/order.ts"],
        ]
    );
    let lend = format!("read-only view at `{}`", root.path().display());
    for note in notes {
        assert!(note.contains(&lend), "the root is lent: {note}");
    }
}

// Dependencies, build output, tests, declaration files, and dot entries are
// not production source, and a file this adapter does not read is not a
// module: no note names them.
#[test]
fn non_production() {
    let root = tempfile::tempdir().expect("a scratch tree");
    tree(
        root.path(),
        [
            "routes/orders.ts",
            "routes/users.ts",
            "routes/README.md",
            "services/index.ts",
            "services/mail.ts",
            "services/mail.test.ts",
            "services/mail.spec.ts",
            "services/types.d.ts",
            "node_modules/left-pad/index.js",
            "dist/bundle.js",
            "tests/orders.e2e.ts",
            ".git/HEAD",
        ],
    );

    let input = workspace(root.path());
    let materials = Adapter::survey(&ctx(&input)).expect("the tree is surveyed");

    let named: Vec<_> = notes(&materials).into_iter().map(named).collect();
    assert_eq!(
        named,
        [
            vec!["routes/orders.ts", "routes/users.ts"],
            vec!["services/index.ts", "services/mail.ts"],
        ]
    );
}

// An inline value has no tree to cut: the bound value, whole.
#[test]
fn inline_value() {
    let input = SourceInput {
        key: "legacy-monolith".to_string(),
        content: SourceContent::Value("export const port = 8080;".to_string()),
    };

    let materials = Adapter::survey(&ctx(&input)).expect("a value is surveyed");

    assert_eq!(materials, [Material::Bound]);
}
