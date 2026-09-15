//! Asserts what the TypeScript adapter decides before the SDK's fan-out.
//!
//! - Which entries are production source, and so the candidates its one
//!   survey turn offers.
//! - How the model's partition becomes seams: one note per group that
//!   meets the floor, the remainder as one more, each naming its files and
//!   lent the root.
//! - When no survey turn is spent at all: a tree no directory cut would
//!   split, or an inline value, is the bound input whole.
//!
//! What the survey call itself does for every adapter — the request shape,
//! the check, the fold — is the SDK's suite's.

use std::path::Path;

use emery_sdk::{Context, Seam, SourceAdapter as _, SourceContent, SourceInput, registry};
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

// Each seam's note; anything else is not this adapter's survey.
fn notes(seams: &[Seam]) -> Vec<&str> {
    seams
        .iter()
        .map(|seam| match seam {
            Seam::Note(note) => note.as_str(),
            other => panic!("a typescript seam is a note, got {other:?}"),
        })
        .collect()
}

// The files a note names to mine, or a survey turn offers, in order.
fn named(text: &str) -> Vec<&str> {
    text.lines().filter_map(|line| line.strip_prefix("- `")?.strip_suffix('`')).collect()
}

// A tree of one directory cuts no finer than itself: the bound tree, whole,
// and no survey turn spent deciding so.
#[tokio::test]
async fn bound_tree() {
    let model = Scripted::default();
    let root = tempfile::tempdir().expect("a scratch tree");
    tree(root.path(), ["src/index.ts", "src/server.ts"]);

    let input = workspace(root.path());
    let seams = Adapter::survey(&model, &ctx(&input)).await.expect("the tree is surveyed");

    assert_eq!(seams, [Seam::Whole]);
    assert!(model.seen().is_empty(), "no turn was spent");
}

// A tree of several directories is surveyed once, under the embedded survey
// prompt with the root lent and every production module offered. The
// model's groups become notes in answer order; a group under the floor
// folds, with every module the model left out, into the remainder's note,
// last. Every note lends the root, so an import into a sibling resolves.
#[tokio::test]
async fn two_directories() {
    let model = Scripted::answering([r#"{"groups":[
        {"name":"/orders routes","files":["services/orders.ts","routes/orders.ts"]},
        {"name":"/users routes","files":["routes/users.ts"]}
    ]}"#]);
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
    let seams = Adapter::survey(&model, &ctx(&input)).await.expect("the partition is accepted");

    let seen = model.seen();
    assert_eq!(seen.len(), 1, "one survey turn");
    let request = &seen[0];
    let prompt = registry::body(Adapter::docs(), "prompts/survey.md").expect("embedded");
    assert_eq!(request.system.as_deref(), Some(prompt), "the survey prompt is the system");
    let lend = root.path().display().to_string();
    assert_eq!(request.workspace.as_deref(), Some(lend.as_str()), "the root is lent");
    assert_eq!(
        named(&request.messages[0]),
        [
            "index.ts",
            "models/order.ts",
            "routes/orders.ts",
            "routes/users.ts",
            "services/mail.ts",
            "services/orders.ts",
        ],
        "every production module is a candidate"
    );

    let notes = notes(&seams);
    let named: Vec<_> = notes.iter().map(|note| named(note)).collect();
    assert_eq!(
        named,
        [
            vec!["routes/orders.ts", "services/orders.ts"],
            vec!["index.ts", "models/order.ts", "routes/users.ts", "services/mail.ts"],
        ]
    );
    let lend = format!("read-only view at `{lend}`");
    for note in notes {
        assert!(note.contains(&lend), "the root is lent: {note}");
    }
    model.assert_exhausted();
}

// Dependencies, build output, tests, declaration files, and dot entries are
// not production source, and a file this adapter does not read is not a
// module: the survey never offers them, so no group can name them. A model
// that discerns no surface answers no groups, and the remainder mines the
// tree whole.
#[tokio::test]
async fn non_production() {
    let model = Scripted::answering([r#"{"groups":[]}"#]);
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
    let seams = Adapter::survey(&model, &ctx(&input)).await.expect("the partition is accepted");

    let production =
        ["routes/orders.ts", "routes/users.ts", "services/index.ts", "services/mail.ts"];
    assert_eq!(named(&model.seen()[0].messages[0]), production, "the candidates offered");
    let named: Vec<_> = notes(&seams).into_iter().map(named).collect();
    assert_eq!(named, [production]);
}

// A partition the SDK's check refuses — here a module never offered — goes
// back to the model, and the adapter's notes are cut from the partition
// finally accepted, never the refused one.
#[tokio::test]
async fn corrected_partition() {
    let model = Scripted::answering([
        r#"{"groups":[{"name":"/orders routes","files":["routes/orders.ts","routes/ghost.ts"]}]}"#,
        r#"{"groups":[{"name":"/orders routes","files":["routes/orders.ts","services/orders.ts"]}]}"#,
    ]);
    let root = tempfile::tempdir().expect("a scratch tree");
    tree(root.path(), ["routes/orders.ts", "routes/users.ts", "services/orders.ts"]);

    let input = workspace(root.path());
    let seams =
        Adapter::survey(&model, &ctx(&input)).await.expect("the second partition is accepted");

    let named: Vec<_> = notes(&seams).into_iter().map(named).collect();
    assert_eq!(named, [vec!["routes/orders.ts", "services/orders.ts"], vec!["routes/users.ts"]]);
    let exchanges = model.exchanges();
    assert_eq!(exchanges.len(), 2, "one rejection, one acceptance");
    let correction = exchanges[0].outcome.as_ref().expect_err("the first partition is refused");
    assert!(correction.contains("`routes/ghost.ts`"), "{correction}");
    model.assert_exhausted();
}

// An inline value has no tree to cut: the bound value, whole, and no survey
// turn spent on it.
#[tokio::test]
async fn inline_value() {
    let model = Scripted::default();
    let input = SourceInput {
        key: "legacy-monolith".to_string(),
        content: SourceContent::Value("export const port = 8080;".to_string()),
    };

    let seams = Adapter::survey(&model, &ctx(&input)).await.expect("a value is surveyed");

    assert_eq!(seams, [Seam::Whole]);
    assert!(model.seen().is_empty(), "no turn was spent");
}
