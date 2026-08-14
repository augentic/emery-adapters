//! Intent survey / extract operation behavior.

use std::path::Path;

use adapter::Source as _;
use adapter::seam::{
    Authority, ClaimKind, Context, Error, Lead, SourceContent, SourceInput, SourceWorkspace,
};
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
        focus: None,
    }
}

fn extract_input(lead: Lead) -> SourceInput {
    let mut input = value_input();
    input.focus = Some(lead);
    input
}

#[tokio::test]
async fn survey_inline_binding() {
    let model = Harness::answering([
        r#"{"leads":[{"lead":"password-reset","synopsis":"Let users reset passwords by email."}]}"#,
    ]);

    let result = Adapter::survey(&model, &ctx(), &value_input()).await.unwrap();

    assert_eq!(result.leads.len(), 1);
    assert!(result.children.is_empty(), "unfocused returns leads only");
    let request = &model.requests()[0];
    assert!(request.system.as_deref().unwrap().starts_with("# intent.survey"));
    let user = &request.messages[0].content;
    assert!(user.contains("inline value"), "prompt names the inline binding");
    assert!(user.contains("no `$SOURCE_DIR`"), "prompt says no source tree is bound");
    assert!(user.contains("exactly one lead"), "prompt carries the degenerate cardinality");
    assert!(user.contains("Let users reset passwords by email."), "value is on the wire");
    assert!(user.contains("Do not read `plan.yaml`"), "adapters never parse the plan");
    assert!(request.workspace.is_none(), "inline value lends no workspace");
}

#[tokio::test]
async fn survey_focused_children() {
    let model = Harness::answering([
        r#"{"children":[{"lead":"reset-expiry","synopsis":"Reset links expire after 30 minutes.","parent":"password-reset","focus":"password-reset"}]}"#,
    ]);
    let mut input = value_input();
    input.focus = Some(Lead::new("password-reset", "Let users reset passwords by email."));

    let result = Adapter::survey(&model, &ctx(), &input).await.unwrap();

    assert!(result.leads.is_empty(), "focused returns children only");
    assert_eq!(result.children.len(), 1);
    assert_eq!(result.children[0].lead, "reset-expiry");
    assert_eq!(result.children[0].parent.as_deref(), Some("password-reset"));
    let user = &model.requests()[0].messages[0].content;
    assert!(user.contains("focused survey"), "user message names the focused path");
    assert!(user.contains("- lead: password-reset"), "parent lead is rendered");
    assert!(user.contains("`children` array"), "answer shape is children");
}

#[tokio::test]
async fn survey_single_file_workspace() {
    let model = Harness::answering([
        r#"{"leads":[{"lead":"password-reset","synopsis":"Let users reset passwords by email."}]}"#,
    ]);
    let root = tempfile::tempdir().unwrap();
    let nested = root.path().join("nested");
    std::fs::create_dir(&nested).unwrap();
    std::fs::write(nested.join("intent.md"), "Let users reset passwords by email.").unwrap();

    let result = Adapter::survey(&model, &ctx(), &workspace_input(root.path())).await.unwrap();

    assert_eq!(result.leads.len(), 1);
    let user = &model.requests()[0].messages[0].content;
    assert!(
        user.contains("Let users reset passwords by email."),
        "the located file's contents are interpolated as the intent string"
    );
}

#[tokio::test]
async fn survey_rejects_multi_file_workspace() {
    let model = Harness::answering([r#"{"leads":[]}"#]);
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("one.md"), "first").unwrap();
    std::fs::write(root.path().join("two.md"), "second").unwrap();

    let result = Adapter::survey(&model, &ctx(), &workspace_input(root.path())).await;

    assert!(matches!(result, Err(Error::InvalidRequest(_))), "got {result:?}");
    assert!(model.requests().is_empty(), "no judgment leg runs on a malformed input");
}

#[tokio::test]
async fn survey_rejects_empty_workspace() {
    let model = Harness::answering([r#"{"leads":[]}"#]);
    let root = tempfile::tempdir().unwrap();

    let result = Adapter::survey(&model, &ctx(), &workspace_input(root.path())).await;

    assert!(matches!(result, Err(Error::InvalidRequest(_))), "got {result:?}");
    assert!(model.requests().is_empty(), "no judgment leg runs on a malformed input");
}

#[tokio::test]
async fn extract_intent_claim() {
    let model = Harness::answering([
        r#"{"authority":"intent","claims":[{"kind":"intent","id":"password-reset","statement":"Let users reset passwords by email."}]}"#,
    ]);
    let lead = Lead::new("password-reset", "Let users reset passwords by email.");

    let evidence = Adapter::extract(&model, &ctx(), &extract_input(lead)).await.unwrap();

    assert_eq!(evidence.authority, Authority::Intent);
    assert_eq!(evidence.claims.len(), 1);
    assert_eq!(evidence.claims[0].kind, ClaimKind::Intent);
    assert_eq!(evidence.claims[0].id.as_deref(), Some("password-reset"));
    let request = &model.requests()[0];
    assert!(request.system.as_deref().unwrap().starts_with("# intent.extract"));
}

#[tokio::test]
async fn extract_requires_focus() {
    let model = Harness::answering::<&str>([]);

    let result = Adapter::extract(&model, &ctx(), &value_input()).await;

    match result {
        Err(Error::InvalidRequest(detail)) => {
            assert!(detail.contains("input.focus"), "detail: {detail}");
        }
        other => panic!("expected InvalidRequest, got {other:?}"),
    }
    assert!(model.requests().is_empty(), "missing focus never reaches the model");
}
