//! Intent's own extract behaviour, natively over a scripted model: what the
//! adapter makes of its input before the one model call — the brief it
//! accepts inline or reads from a one-file tree into the turn's material,
//! and the refusals it fails closed with. The SDK's suite owns the request
//! shape every adapter shares; the root seam suites own what crosses the
//! component boundary and the corpus it embeds.

use std::path::Path;

use emery_sdk::{Context, Error, SourceAdapter as _, SourceContent, SourceInput};
use intent::Adapter;
use omnia_test::guest::Scripted;

const BRIEF: &str = "Let users reset passwords by email.";

const ANSWER: &str = r#"{"authority":"intent","claims":[
    {"kind":"intent","id":"intent","statement":"Let users reset passwords by email."}
]}"#;

const fn ctx(input: &SourceInput) -> Context<'_> {
    Context {
        adapter_id: "source:intent",
        input,
    }
}

fn workspace_input(root: &Path) -> SourceInput {
    SourceInput {
        key: "intent".to_string(),
        content: SourceContent::Workspace(root.display().to_string()),
    }
}

fn value_input(brief: &str) -> SourceInput {
    SourceInput {
        key: "intent".to_string(),
        content: SourceContent::Value(brief.to_string()),
    }
}

// A non-empty inline brief is accepted as the bound material and reaches
// the model.
#[tokio::test]
async fn inline_value() {
    let model = Scripted::answering([ANSWER]);

    let input = value_input(BRIEF);
    let evidence =
        Adapter::extract(&model, &ctx(&input)).await.expect("the scripted answer is accepted");

    assert_eq!(evidence.claims.len(), 1);
    let turn = &model.seen()[0].messages[0];
    assert!(turn.contains(BRIEF), "the brief is the material: {turn}");
}

// A one-file tree — nested or not — is read into the turn's material as the
// intent string, named as the tree it came from.
#[tokio::test]
async fn one_file() {
    let model = Scripted::answering([ANSWER]);
    let root = tempfile::tempdir().expect("a scratch tree");
    let nested = root.path().join("nested");
    std::fs::create_dir(&nested).expect("the nested directory is created");
    std::fs::write(nested.join("intent.md"), BRIEF).expect("the brief is written");

    let input = workspace_input(root.path());
    let evidence =
        Adapter::extract(&model, &ctx(&input)).await.expect("the scripted answer is accepted");

    assert_eq!(evidence.claims.len(), 1);
    let turn = &model.seen()[0].messages[0];
    assert!(turn.contains(BRIEF), "the located file's contents are the intent string: {turn}");
    assert!(turn.contains("one-file tree"), "the material names the tree source: {turn}");
}

// A tree that is not the one-file encoding — no file, or several — is a
// typed refusal before any model call.
#[tokio::test]
async fn not_one_file() {
    // The refusal precedes the model, so nothing is scripted.
    let model = Scripted::default();

    let empty = tempfile::tempdir().expect("a scratch tree");
    let input = workspace_input(empty.path());
    let none = Adapter::extract(&model, &ctx(&input)).await;
    assert!(matches!(none, Err(Error::BadRequest { .. })), "got {none:?}");

    let several = tempfile::tempdir().expect("a scratch tree");
    std::fs::write(several.path().join("one.md"), "first").expect("the first file is written");
    std::fs::write(several.path().join("two.md"), "second").expect("the second file is written");
    let input = workspace_input(several.path());
    let many = Adapter::extract(&model, &ctx(&input)).await;
    assert!(matches!(many, Err(Error::BadRequest { .. })), "got {many:?}");

    assert!(model.seen().is_empty(), "no judgment leg runs on a malformed input");
}

// The intent source is never legitimately empty (the prompt's own
// contract): an empty brief is a typed refusal, never an empty success.
#[tokio::test]
async fn empty_brief() {
    let model = Scripted::default();

    let blank = value_input("  \n");
    let inline = Adapter::extract(&model, &ctx(&blank)).await;
    assert!(matches!(inline, Err(Error::BadRequest { .. })), "got {inline:?}");

    let root = tempfile::tempdir().expect("a scratch tree");
    std::fs::write(root.path().join("intent.md"), "\n\t \n").expect("the blank brief is written");
    let input = workspace_input(root.path());
    let tree = Adapter::extract(&model, &ctx(&input)).await;
    assert!(matches!(tree, Err(Error::BadRequest { .. })), "got {tree:?}");

    assert!(model.seen().is_empty(), "no judgment leg runs on an empty brief");
}
