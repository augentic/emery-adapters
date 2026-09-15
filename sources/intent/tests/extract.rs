//! Asserts what the intent adapter makes of its input before the one model call.
//!
//! The brief it accepts inline or reads from a one-file tree, and the
//! refusals it fails closed with.

use std::path::Path;

use emery_sdk::{Context, Error, SourceAdapter as _, SourceContent, SourceInput};
use intent::Adapter;
use omnia_test::guest::Scripted;

const BRIEF: &str = "Let users reset passwords by email.";

const ANSWER: &str = r#"{"claims":[
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

// Nested or not, the one file is read into the turn's material.
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

// No file, or several: a typed refusal before any model call.
#[tokio::test]
async fn not_one_file() {
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

// An intent source is never legitimately empty: a typed refusal, never an
// empty success.
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
