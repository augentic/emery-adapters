//! Intent extract operation behavior over the `Source` capability.

use std::path::Path;

use emery_adapter::{
    Authority, ClaimKind, Context, Error, SourceAdapter as _, SourceContent, SourceInput,
};
use intent::Adapter;
use omnia_test::SeenFormat;
use omnia_test::guest::Scripted;

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
    let model = Scripted::answering([r#"{"authority":"intent","claims":[
            {"kind":"intent","id":"intent","statement":"Let users reset passwords by email."},
            {"kind":"requirement","id":"password-reset.request","statement":"Users reset passwords by email."}
        ]}"#]);

    let input = value_input("Let users reset passwords by email.");
    let evidence = Adapter::extract(&model, &ctx(&input)).await.unwrap();

    assert_eq!(evidence.authority, Authority::Intent);
    assert_eq!(evidence.claims.len(), 2);
    assert_eq!(evidence.claims[0].kind, ClaimKind::Intent);
    assert_eq!(evidence.claims[0].id.as_deref(), Some("intent"));
    assert_eq!(
        evidence.claims[0].extras.get("statement").and_then(|value| value.as_str()),
        Some("Let users reset passwords by email."),
    );
    // Only `requirement` claims form spec rows, so the directive is
    // lifted into one for reconciliation to join against other sources.
    assert_eq!(evidence.claims[1].kind, ClaimKind::Requirement);
    assert_eq!(evidence.claims[1].id.as_deref(), Some("password-reset.request"));
    assert_eq!(
        evidence.claims[1].extras.get("statement").and_then(|value| value.as_str()),
        Some("Users reset passwords by email."),
    );

    let seen = model.seen();
    assert_eq!(seen.len(), 1, "extract is a single judgment leg");
    let request = &seen[0];
    let system = request.system.as_deref().unwrap();
    assert!(system.starts_with("# intent.extract"));
    assert!(system.contains("whole brief, verbatim"), "the echo contract is stated");
    assert!(
        system.contains("One per distinct behavioural directive"),
        "the reconciliation-join contract is stated"
    );
    let user = &request.messages[0];
    assert!(user.contains("source key `intent`"), "passed source key is named");
    assert!(user.contains("inline value"), "prompt names the inline source");
    assert!(user.contains("no `$SOURCE_DIR` is lent"), "prompt says no source tree is bound");
    assert!(user.contains("Let users reset passwords by email."), "value is on the wire");
    let SeenFormat::Schema { name, schema } = &request.format else {
        panic!("expected schema format, got {:?}", request.format)
    };
    assert_eq!(name, "evidence");
    let schema: serde_json::Value = serde_json::from_str(schema).expect("the schema is JSON");
    assert!(
        schema.pointer("/$defs/Claim/properties/id/pattern").is_some(),
        "the claim-id grammar steers the answer"
    );
    assert!(request.check, "acceptance is the SDK's claim gate, not the reply text");
    assert!(request.workspace.is_none(), "inline value lends no workspace");
    assert_eq!(request.tools, ["list_docs", "read_doc"], "the reference tools are declared");
}

#[tokio::test]
async fn one_file() {
    let model = Scripted::answering([
        r#"{"authority":"intent","claims":[{"kind":"intent","id":"intent","statement":"Let users reset passwords by email."}]}"#,
    ]);
    let root = tempfile::tempdir().unwrap();
    let nested = root.path().join("nested");
    std::fs::create_dir(&nested).unwrap();
    std::fs::write(nested.join("intent.md"), "Let users reset passwords by email.").unwrap();

    let input = workspace_input(root.path());
    let evidence = Adapter::extract(&model, &ctx(&input)).await.unwrap();

    assert_eq!(evidence.claims.len(), 1);
    let user = &model.seen()[0].messages[0];
    assert!(
        user.contains("Let users reset passwords by email."),
        "the located file's contents are interpolated as the intent string"
    );
    assert!(user.contains("one-file tree"), "prompt names the tree source");
}

// An unreadable source fails closed before any judgment leg: a tree
// that is not the one-file encoding is a typed refusal.
#[tokio::test]
async fn multi_file() {
    // The refusal precedes the model, so nothing is scripted.
    let model = Scripted::default();
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("one.md"), "first").unwrap();
    std::fs::write(root.path().join("two.md"), "second").unwrap();

    let input = workspace_input(root.path());
    let result = Adapter::extract(&model, &ctx(&input)).await;

    assert!(matches!(result, Err(Error::BadRequest { .. })), "got {result:?}");
    assert!(model.seen().is_empty(), "no judgment leg runs on a malformed input");
}

#[tokio::test]
async fn empty_workspace() {
    let model = Scripted::default();
    let root = tempfile::tempdir().unwrap();

    let input = workspace_input(root.path());
    let result = Adapter::extract(&model, &ctx(&input)).await;

    assert!(matches!(result, Err(Error::BadRequest { .. })), "got {result:?}");
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

    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("intent.md"), "\n\t \n").unwrap();
    let input = workspace_input(root.path());
    let tree = Adapter::extract(&model, &ctx(&input)).await;
    assert!(matches!(tree, Err(Error::BadRequest { .. })), "got {tree:?}");

    assert!(model.seen().is_empty(), "no judgment leg runs on an empty brief");
}
