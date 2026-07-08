//! Captures-specific operation behavior: `kind: example` claims with the
//! open `replay-digest` / `input` / `output` body fields.

use captures_core as core;
use std::path::Path;

use adapter::seam::{Authority, ClaimKind, Context, Error, Lead};
use core::operations::{describe, extract};
use testkit::MockModel;

fn ctx() -> Context<'static> {
    Context {
        adapter_id: "source:captures",
        project_root: Path::new("."),
        mcp_url: None,
    }
}

fn lead() -> Lead {
    Lead {
        lead: "password-reset".to_string(),
        synopsis: "POST /password-reset handler with three captured scenarios.".to_string(),
        topics: Vec::new(),
    }
}

// The extract answer's `kind: example` claims carry open per-kind body
// fields (`replay-digest`, `input`, `output`) the seam tolerates, under
// the `behaviour` authority the adapter emits.
#[tokio::test]
async fn extract_parses_example_claims_with_replay_digests() {
    let model = MockModel::answering([r#"{
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

    let evidence = extract(&model, &ctx(), &lead()).await.unwrap();

    assert_eq!(evidence.authority, Authority::Behaviour);
    assert_eq!(evidence.claims.len(), 1);
    assert_eq!(evidence.claims[0].kind, ClaimKind::Example);
    assert_eq!(evidence.claims[0].id.as_deref(), Some("password-reset.expired-token"));
    let request = &model.requests()[0];
    assert!(request.system.as_deref().unwrap().starts_with("# Runtime capture extract"));
    assert!(
        request.messages[0].content.contains("tests/data/replays"),
        "the binding note names the capture-tree layout"
    );
}

// `example` claims are id-required by the evidence schema's conditional;
// the deterministic tail enforces the same rule after the answer lands.
#[tokio::test]
async fn extract_tail_rejects_idless_example_claims() {
    let model =
        MockModel::answering([r#"{"authority":"behaviour","claims":[{"kind":"example"}]}"#]);

    let err = extract(&model, &ctx(), &lead()).await.unwrap_err();

    assert!(matches!(err, Error::Internal(detail) if detail.contains("require an id")));
}

// No model call.
#[test]
fn describe_declares_no_floor() {
    assert_eq!(describe().specify_floor, None);
}
