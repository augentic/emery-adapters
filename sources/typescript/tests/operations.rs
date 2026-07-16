//! Typescript-specific operation behavior: the source-tree binding note,
//! the framework-grammar survey framing, and the references pointer.

use std::path::Path;

use adapter::seam::{Authority, ClaimKind, Context, Lead};
use testkit::Harness;
use typescript::operations::{extract, survey};

fn ctx() -> Context<'static> {
    Context {
        adapter_id: "source:typescript",
        project_root: Path::new("."),
        mcp_url: None,
    }
}

#[tokio::test]
async fn survey_framework_grammar() {
    let model = Harness::answering([
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
    assert!(user.contains("references"), "the prompt points at the MCP-served references");
    assert!(user.contains("- lead: task-service"), "the lead renders as the prompt's block shape");
}
