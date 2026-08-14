//! Screenshots survey / extract operation behavior.

use std::path::Path;

use adapter::Source as _;
use adapter::seam::{
    Authority, ClaimKind, Context, Lead, SourceContent, SourceInput, SourceWorkspace,
};
use omnia_testkit::model::Harness;
use screenshots::Adapter;

fn ctx() -> Context<'static> {
    Context {
        adapter_id: "source:screenshots",
        project_root: Path::new("."),
        mcp_url: None,
        lend: Some(".".to_string()),
    }
}

fn workspace_input() -> SourceInput {
    SourceInput {
        key: "screens".to_string(),
        content: SourceContent::Workspace(SourceWorkspace {
            id: "view-1".to_string(),
            root: ".".to_string(),
        }),
        focus: None,
    }
}

#[tokio::test]
async fn survey_focused_children() {
    let model = Harness::answering([
        r#"{"children":[{"lead":"task-list-empty","synopsis":"Empty-state variant of the task list.","parent":"task-list","focus":"task-list"}]}"#,
    ]);
    let mut input = workspace_input();
    input.focus = Some(Lead::new("task-list", "Task list screen with add button."));

    let result = Adapter::survey(&model, &ctx(), &input).await.unwrap();

    assert!(result.leads.is_empty(), "focused returns children only");
    assert_eq!(result.children.len(), 1);
    assert_eq!(result.children[0].lead, "task-list-empty");
    assert_eq!(result.children[0].parent.as_deref(), Some("task-list"));
    let user = &model.requests()[0].messages[0].content;
    assert!(user.contains("focused survey"), "user message names the focused path");
    assert!(user.contains("screen images"), "the note names the image-set material");
}

#[tokio::test]
async fn extract_spatial_kinds() {
    let model = Harness::answering([r#"{
            "authority": "documentation",
            "claims": [
                {"kind": "region", "id": "task-list.header", "path": "task-list.png"},
                {"kind": "container", "id": "task-list.rows", "path": "task-list.png"},
                {"kind": "leaf", "id": "task-list.add-button", "path": "task-list.png"}
            ]
        }"#]);
    let mut input = workspace_input();
    input.focus = Some(Lead::new("task-list", "Task list screen with add button."));

    let evidence = Adapter::extract(&model, &ctx(), &input).await.unwrap();

    assert_eq!(evidence.authority, Authority::Documentation);
    let kinds: Vec<ClaimKind> = evidence.claims.iter().map(|claim| claim.kind).collect();
    assert_eq!(kinds, [ClaimKind::Region, ClaimKind::Container, ClaimKind::Leaf]);
    let request = &model.requests()[0];
    assert!(request.system.as_deref().unwrap().starts_with("# `screenshots.extract`"));
    assert!(
        request.messages[0].content.contains("screen images"),
        "the note names the image-set material"
    );
}
