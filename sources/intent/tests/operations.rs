//! Intent extract operation behavior over the extract-only seam.

use std::path::Path;

use emery_adapter::answers::EVIDENCE_ANSWER_SCHEMA;
use emery_adapter::seam::{
    Authority, ClaimKind, Context, Error, SourceContent, SourceInput, SourceWorkspace,
};
use emery_adapter::{Format, Request, Source as _};
use intent::Adapter;
use omnia_testkit::model::Harness;

fn ctx() -> Context<'static> {
    Context {
        adapter_id: "source:intent",
        project_root: Path::new("."),
        mcp_url: None,
        lend: None,
    }
}

fn value_input() -> SourceInput {
    SourceInput::value("intent", "Let users reset passwords by email.")
}

fn workspace_input(root: &Path) -> SourceInput {
    SourceInput {
        key: "intent".to_string(),
        content: SourceContent::Workspace(SourceWorkspace {
            id: "view-1".to_string(),
            root: root.display().to_string(),
        }),
    }
}

fn schema_format(request: &Request) -> (&str, &str) {
    match &request.format {
        Format::Schema(schema) => (&schema.name, &schema.schema),
        other => panic!("expected schema format, got {other:?}"),
    }
}

#[tokio::test]
async fn extract_inline_value() {
    let model = Harness::answering([r#"{"authority":"intent","claims":[
            {"kind":"intent","id":"intent","statement":"Let users reset passwords by email."},
            {"kind":"requirement","id":"password-reset.request","statement":"Users reset passwords by email."}
        ]}"#]);

    let evidence = Adapter::extract(&model, &ctx(), &value_input()).await.unwrap();

    assert_eq!(evidence.authority, Authority::Intent);
    assert_eq!(evidence.claims.len(), 2);
    assert_eq!(evidence.claims[0].kind, ClaimKind::Intent);
    assert_eq!(evidence.claims[0].id.as_deref(), Some("intent"));
    // The verbatim echo lands on the open claim record's `statement`
    // extra — the value synthesis reads.
    assert_eq!(
        evidence.claims[0].extras.get("statement").and_then(|value| value.as_str()),
        Some("Let users reset passwords by email."),
    );
    // The directive is lifted into a requirement claim so deterministic
    // reconciliation can join it against other sources (only
    // `requirement` claims form spec rows).
    assert_eq!(evidence.claims[1].kind, ClaimKind::Requirement);
    assert_eq!(evidence.claims[1].id.as_deref(), Some("password-reset.request"));
    assert_eq!(
        evidence.claims[1].extras.get("statement").and_then(|value| value.as_str()),
        Some("Users reset passwords by email."),
    );

    let requests = model.requests();
    assert_eq!(requests.len(), 1, "extract is a single judgment leg");
    let request = &requests[0];
    assert!(request.system.as_deref().unwrap().starts_with("# intent.extract"));
    let user = &request.messages[0].content;
    assert!(user.contains("source key `intent`"), "passed source key is named");
    assert!(user.contains("inline value"), "prompt names the inline binding");
    assert!(user.contains("no `$SOURCE_DIR` is lent"), "prompt says no source tree is bound");
    assert!(user.contains("Let users reset passwords by email."), "value is on the wire");
    assert!(user.contains("verbatim"), "the echo contract is stated");
    assert!(
        user.contains("one `kind: \"requirement\"` claim per distinct behavioural directive"),
        "the reconciliation-join contract is stated"
    );
    let (name, schema) = schema_format(request);
    assert_eq!(name, "evidence");
    assert_eq!(schema, EVIDENCE_ANSWER_SCHEMA);
    assert!(request.workspace.is_none(), "inline value lends no workspace");
}

#[tokio::test]
async fn extract_single_file_workspace() {
    let model = Harness::answering([
        r#"{"authority":"intent","claims":[{"kind":"intent","id":"intent","statement":"Let users reset passwords by email."}]}"#,
    ]);
    let root = tempfile::tempdir().unwrap();
    let nested = root.path().join("nested");
    std::fs::create_dir(&nested).unwrap();
    std::fs::write(nested.join("intent.md"), "Let users reset passwords by email.").unwrap();

    let evidence = Adapter::extract(&model, &ctx(), &workspace_input(root.path())).await.unwrap();

    assert_eq!(evidence.claims.len(), 1);
    let user = &model.requests()[0].messages[0].content;
    assert!(
        user.contains("Let users reset passwords by email."),
        "the located file's contents are interpolated as the intent string"
    );
    assert!(user.contains("one-file tree"), "prompt names the tree binding");
}

// An unreadable source fails closed before any judgment leg (A14): a
// tree that is not the one-file encoding is a typed refusal, never an
// empty success.
#[tokio::test]
async fn extract_rejects_multi_file_workspace() {
    let model = Harness::answering([r#"{"authority":"intent","claims":[]}"#]);
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("one.md"), "first").unwrap();
    std::fs::write(root.path().join("two.md"), "second").unwrap();

    let result = Adapter::extract(&model, &ctx(), &workspace_input(root.path())).await;

    assert!(matches!(result, Err(Error::InvalidRequest(_))), "got {result:?}");
    assert!(model.requests().is_empty(), "no judgment leg runs on a malformed input");
}

#[tokio::test]
async fn extract_rejects_empty_workspace() {
    let model = Harness::answering([r#"{"authority":"intent","claims":[]}"#]);
    let root = tempfile::tempdir().unwrap();

    let result = Adapter::extract(&model, &ctx(), &workspace_input(root.path())).await;

    assert!(matches!(result, Err(Error::InvalidRequest(_))), "got {result:?}");
    assert!(model.requests().is_empty(), "no judgment leg runs on a malformed input");
}

// The intent binding is never legitimately empty (the prompt's own
// contract): an empty brief is a typed refusal, never an empty success.
#[tokio::test]
async fn extract_rejects_empty_brief() {
    let model = Harness::answering([r#"{"authority":"intent","claims":[]}"#]);

    let inline = Adapter::extract(&model, &ctx(), &SourceInput::value("intent", "  \n")).await;
    assert!(matches!(inline, Err(Error::InvalidRequest(_))), "got {inline:?}");

    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("intent.md"), "\n\t \n").unwrap();
    let tree = Adapter::extract(&model, &ctx(), &workspace_input(root.path())).await;
    assert!(matches!(tree, Err(Error::InvalidRequest(_))), "got {tree:?}");

    assert!(model.requests().is_empty(), "no judgment leg runs on an empty brief");
}
