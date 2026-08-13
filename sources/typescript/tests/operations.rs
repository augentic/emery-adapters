//! TypeScript survey / extract operation behavior.

use std::path::Path;

use adapter::Source as _;
use adapter::seam::{
    Authority, ClaimKind, Context, Lead, SourceContent, SourceInput, SourceWorkspace,
};
use omnia_testkit::model::Harness;
use typescript::Adapter;

fn ctx() -> Context<'static> {
    Context {
        adapter_id: "source:typescript",
        project_root: Path::new("."),
        mcp_url: None,
        lend: Some(".".to_string()),
    }
}

fn workspace_input() -> SourceInput {
    SourceInput {
        key: "legacy-monolith".to_string(),
        content: SourceContent::Workspace(SourceWorkspace {
            id: "view-1".to_string(),
            root: ".".to_string(),
        }),
        focus: None,
    }
}

#[tokio::test]
async fn survey_framework_grammar() {
    let model = Harness::answering([
        r#"{"leads":[{"lead":"task-service","synopsis":"Task CRUD service module."}]}"#,
    ]);

    let result = Adapter::survey(&model, &ctx(), &workspace_input()).await.unwrap();

    assert_eq!(result.leads[0].lead, "task-service");
    assert!(result.children.is_empty(), "unfocused returns leads only");
    let request = &model.requests()[0];
    assert!(request.system.as_deref().unwrap().starts_with("# TypeScript / JavaScript source"));
    let user = &request.messages[0].content;
    assert!(user.contains("framework grammar"), "survey framing names the prompt's grammar");
    assert!(
        user.contains("TypeScript / JavaScript source tree"),
        "the note names the TS / JS tree"
    );
    assert!(user.contains("read-only"), "the note marks the tree read-only");
    assert!(user.contains("CID view"), "workspace is the CID view");
    assert!(user.contains("Do not read `plan.yaml`"), "adapters never parse the plan");
}

#[tokio::test]
async fn survey_focused_children() {
    let model = Harness::answering([
        r#"{"children":[{"lead":"task-create","synopsis":"POST /tasks handler.","parent":"task-service","focus":"task-service"}]}"#,
    ]);
    let mut input = workspace_input();
    input.focus = Some(Lead::new("task-service", "Task CRUD service module."));

    let result = Adapter::survey(&model, &ctx(), &input).await.unwrap();

    assert!(result.leads.is_empty(), "focused returns children only");
    assert_eq!(result.children.len(), 1);
    assert_eq!(result.children[0].lead, "task-create");
    assert_eq!(result.children[0].parent.as_deref(), Some("task-service"));
    let user = &model.requests()[0].messages[0].content;
    assert!(user.contains("focused survey"), "user message names the focused path");
    assert!(user.contains("- lead: task-service"), "parent lead is rendered");
}

#[tokio::test]
async fn extract_references_pointer() {
    let model = Harness::answering([r#"{
            "authority": "behaviour",
            "claims": [
                {"kind": "type", "path": "src/tasks/model.ts#L4-L18"},
                {"kind": "call", "path": "src/tasks/service.ts#L42"},
                {"kind": "excerpt", "path": "src/tasks/service.ts#L40-L55"}
            ]
        }"#]);
    let mut input = workspace_input();
    input.focus = Some(Lead::new("task-service", "Task CRUD service module."));

    let evidence = Adapter::extract(&model, &ctx(), &input).await.unwrap();

    assert_eq!(evidence.authority, Authority::Behaviour);
    let kinds: Vec<ClaimKind> = evidence.claims.iter().map(|claim| claim.kind).collect();
    assert_eq!(kinds, [ClaimKind::Type, ClaimKind::Call, ClaimKind::Excerpt]);
    let request = &model.requests()[0];
    assert!(request.system.as_deref().unwrap().starts_with("# TypeScript / JavaScript source"));
    let user = &request.messages[0].content;
    assert!(user.contains("references"), "the prompt points at the MCP-served references");
    assert!(user.contains("- lead: task-service"), "the lead renders as the prompt's block shape");
}
