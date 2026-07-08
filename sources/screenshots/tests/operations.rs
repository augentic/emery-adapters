//! Screenshots-specific operation behavior: the spatial claim kinds.

use screenshots_core as core;
use std::path::Path;

use adapter::seam::{Authority, ClaimKind, Context, Lead};
use core::operations::{describe, extract};
use testkit::MockModel;

// The extract answer's spatial claim kinds — `region` / `container` /
// `leaf` — parse through the shared Evidence shape.
#[tokio::test]
async fn extract_parses_the_spatial_kinds() {
    let model = MockModel::answering([r#"{
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

    let evidence = extract(&model, &ctx, &lead).await.unwrap();

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

// No model call.
#[test]
fn describe_declares_no_floor() {
    assert_eq!(describe().specify_floor, None);
}
