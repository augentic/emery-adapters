//! Screenshots survey / extract operation behavior.

use std::path::Path;

use adapter::Source as _;
use adapter::seam::{Authority, ClaimKind, Context, Lead, SourceInput};
use omnia_testkit::model::Harness;
use screenshots::Adapter;

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
    let ctx = Context {
        adapter_id: "source:screenshots",
        project_root: Path::new("."),
        mcp_url: None,
        lend: "/prepared/screens".to_string(),
        source_key: Some("mockups".to_string()),
    };
    let input = SourceInput::Workspace("/prepared/screens".to_string());
    let lead = Lead {
        lead: "task-list".to_string(),
        synopsis: "Task list screen with add button.".to_string(),
        topics: Vec::new(),
    };

    let evidence = Adapter::extract(&model, &ctx, &input, &lead).await.unwrap();

    assert_eq!(evidence.authority, Authority::Documentation);
    let kinds: Vec<ClaimKind> = evidence.claims.iter().map(|claim| claim.kind).collect();
    assert_eq!(kinds, [ClaimKind::Region, ClaimKind::Container, ClaimKind::Leaf]);
    let request = &model.requests()[0];
    assert!(request.system.as_deref().unwrap().starts_with("# `screenshots.extract`"));
    let user = &request.messages[0].content;
    assert!(user.contains("screen images"), "the binding note names the image-set material");
    assert!(user.contains("Source binding key: `mockups`"), "and the binding key");
    assert!(user.contains("working directory"), "the lent tree is the agent's workspace");
    assert!(
        request.system.as_deref().unwrap().contains("`$PROJECT_DIR` is unreachable"),
        "extract must not instruct a project-tree candidate-cache write"
    );
}
