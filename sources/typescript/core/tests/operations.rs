//! Typescript-specific operation behavior: the source-tree binding note,
//! the framework-grammar survey framing, and the reference-shelf pointer.

use std::path::Path;

use specify_guest_kit::MockModel;
use specify_guest_kit::seam::{Authority, ClaimKind, Context, Lead};
use specify_typescript_core::operations::{describe, extract, survey};

fn ctx() -> Context<'static> {
    Context {
        adapter_id: "source:typescript",
        project_root: Path::new("."),
        mcp_url: None,
    }
}

// The survey call embeds the typescript prompt as the system channel and
// frames the call around the prompt's framework grammar and the read-only
// TS / JS source-tree binding.
#[tokio::test]
async fn survey_frames_the_framework_grammar() {
    let model = MockModel::answering([
        r#"{"leads":[{"lead":"task-service","synopsis":"Task CRUD service module."}]}"#,
    ]);

    let leads = survey(&model, &ctx()).await.unwrap();

    assert_eq!(leads[0].lead, "task-service");
    let request = &model.requests()[0];
    assert!(request.system.as_deref().unwrap().starts_with("# TypeScript / JavaScript source"));
    let user = &request.messages[0].content;
    assert!(user.contains("framework grammar"), "survey framing names the prompt's grammar");
    assert!(
        user.contains("TypeScript / JavaScript source tree"),
        "the binding note names the TS / JS tree"
    );
    assert!(user.contains("read-only"), "the binding note marks the tree read-only");
}

// The extract call points the agent at the reference shelf over the MCP
// grant, and the code-shaped claim kinds parse through the shared shape.
#[tokio::test]
async fn extract_points_at_the_reference_shelf() {
    let model = MockModel::answering([r#"{
            "authority": "behaviour",
            "claims": [
                {"kind": "type", "path": "src/tasks/model.ts#L4-L18"},
                {"kind": "call", "path": "src/tasks/service.ts#L42"},
                {"kind": "excerpt", "path": "src/tasks/service.ts#L40-L55"}
            ]
        }"#]);
    let lead = Lead {
        lead: "task-service".to_string(),
        synopsis: "Task CRUD service module.".to_string(),
        topics: Vec::new(),
    };

    let evidence = extract(&model, &ctx(), &lead).await.unwrap();

    assert_eq!(evidence.authority, Authority::Behaviour);
    let kinds: Vec<ClaimKind> = evidence.claims.iter().map(|claim| claim.kind).collect();
    assert_eq!(kinds, [ClaimKind::Type, ClaimKind::Call, ClaimKind::Excerpt]);
    let request = &model.requests()[0];
    assert!(request.system.as_deref().unwrap().starts_with("# TypeScript / JavaScript source"));
    let user = &request.messages[0].content;
    assert!(user.contains("reference shelf"), "the prompt points at the MCP-served references");
    assert!(user.contains("- lead: task-service"), "the lead renders as the prompt's block shape");
}

// The RFC-64 self-description is answerable without a model or a
// filesystem: no compatibility floor is declared.
#[test]
fn describe_declares_no_floor() {
    assert_eq!(describe().specify_floor, None);
}
