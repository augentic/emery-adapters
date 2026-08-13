//! Captures survey / extract operation behavior.

use std::path::Path;

use adapter::Source as _;
use adapter::seam::{Authority, ClaimKind, Context, Lead, SourceInput};
use captures::Adapter;
use omnia_testkit::model::Harness;

fn ctx() -> Context<'static> {
    Context {
        adapter_id: "source:captures",
        project_root: Path::new("."),
        mcp_url: None,
        lend: "/prepared/captures".to_string(),
        source_key: Some("runtime".to_string()),
    }
}

fn input() -> SourceInput {
    SourceInput::Workspace("/prepared/captures".to_string())
}

fn lead() -> Lead {
    Lead {
        lead: "password-reset".to_string(),
        synopsis: "POST /password-reset handler with three captured scenarios.".to_string(),
        topics: Vec::new(),
    }
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

    let evidence = Adapter::extract(&model, &ctx(), &input(), &lead()).await.unwrap();

    assert_eq!(evidence.authority, Authority::Behaviour);
    assert_eq!(evidence.claims.len(), 1);
    assert_eq!(evidence.claims[0].kind, ClaimKind::Example);
    assert_eq!(evidence.claims[0].id.as_deref(), Some("password-reset.expired-token"));
    let request = &model.requests()[0];
    assert!(request.system.as_deref().unwrap().starts_with("# Runtime capture extract"));
    let user = &request.messages[0].content;
    assert!(user.contains("tests/data/replays"), "the binding note names the capture-tree layout");
    assert!(user.contains("Source binding key: `runtime`"), "and the binding key");
    assert!(user.contains("working directory"), "the lent tree is the agent's workspace");
}
