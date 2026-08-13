//! Captures survey / extract operation behavior.

use std::path::Path;

use adapter::Source as _;
use adapter::seam::{
    Authority, ClaimKind, Context, Lead, SourceContent, SourceInput, SourceWorkspace,
};
use captures::Adapter;
use omnia_testkit::model::Harness;

fn ctx() -> Context<'static> {
    Context {
        adapter_id: "source:captures",
        project_root: Path::new("."),
        mcp_url: None,
        lend: Some(".".to_string()),
    }
}

fn workspace_input() -> SourceInput {
    SourceInput {
        key: "runtime".to_string(),
        content: SourceContent::Workspace(SourceWorkspace {
            id: "view-1".to_string(),
            root: ".".to_string(),
        }),
        focus: None,
    }
}

fn lead() -> Lead {
    Lead::new("password-reset", "POST /password-reset handler with three captured scenarios.")
}

#[tokio::test]
async fn survey_focused_children() {
    let model = Harness::answering([
        r#"{"children":[{"lead":"password-reset-expired","synopsis":"Expired-token scenario.","parent":"password-reset","focus":"password-reset"}]}"#,
    ]);
    let mut input = workspace_input();
    input.focus = Some(lead());

    let result = Adapter::survey(&model, &ctx(), &input).await.unwrap();

    assert!(result.leads.is_empty(), "focused returns children only");
    assert_eq!(result.children.len(), 1);
    assert_eq!(result.children[0].lead, "password-reset-expired");
    assert_eq!(result.children[0].parent.as_deref(), Some("password-reset"));
    let user = &model.requests()[0].messages[0].content;
    assert!(user.contains("focused survey"), "user message names the focused path");
    assert!(user.contains("tests/data/replays"), "the note names the capture-tree layout");
}

// The open per-kind body fields (`replay-digest`, `input`, `output`)
// must survive the seam's claim shape.
#[tokio::test]
async fn extract_example_claims() {
    let model = Harness::answering([r#"{
            "authority": "behaviour",
            "claims": [{
                "kind": "example",
                "id": "password-reset.expired-token",
                "path": "tests/data/replays/password-reset/expired-token.json",
                "replay-digest": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
                "input": {"token": "stale"},
                "output": {"status": 410}
            }]
        }"#]);
    let mut input = workspace_input();
    input.focus = Some(lead());

    let evidence = Adapter::extract(&model, &ctx(), &input).await.unwrap();

    assert_eq!(evidence.authority, Authority::Behaviour);
    assert_eq!(evidence.claims.len(), 1);
    assert_eq!(evidence.claims[0].kind, ClaimKind::Example);
    assert_eq!(evidence.claims[0].id.as_deref(), Some("password-reset.expired-token"));
    let request = &model.requests()[0];
    assert!(request.system.as_deref().unwrap().starts_with("# Runtime capture extract"));
    assert!(
        request.messages[0].content.contains("tests/data/replays"),
        "the note names the capture-tree layout"
    );
}
