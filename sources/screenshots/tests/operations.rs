//! Screenshots-specific operation behavior: the spatial claim kinds.

use std::path::Path;

use adapter::Source as _;
use adapter::seam::{Authority, ClaimKind, Context, Lead};
use screenshots::Screenshots;
use testkit::Harness;

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
    };
    let lead = Lead {
        lead: "task-list".to_string(),
        synopsis: "Task list screen with add button.".to_string(),
        topics: Vec::new(),
    };

    let evidence = Screenshots::extract(&model, &ctx, &lead).await.unwrap();

    assert_eq!(evidence.authority, Authority::Documentation);
    let kinds: Vec<ClaimKind> = evidence.claims.iter().map(|claim| claim.kind).collect();
    assert_eq!(kinds, [ClaimKind::Region, ClaimKind::Container, ClaimKind::Leaf]);
    let request = &model.requests()[0];
    assert!(request.system.as_deref().unwrap().starts_with("# `screenshots.extract`"));
    assert!(
        request.messages[0].content.contains("screen images"),
        "the binding note names the image-set material"
    );
}
