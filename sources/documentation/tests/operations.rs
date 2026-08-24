//! Documentation extract operation behavior over the `Source` capability.

use std::path::Path;

use documentation::Adapter;
use emery_adapter::answers::evidence_schema;
use emery_adapter::types::{
    Authority, ClaimKind, Context, Error, SourceContent, SourceInput, SourceWorkspace,
};
use emery_adapter::{Format, MAX_REPAIRS, Request, SourceAdapter as _};
use emery_testkit::{Scripted, mcp_grants};

fn ctx(mcp_url: Option<&str>) -> Context<'static> {
    Context {
        adapter_id: "source:documentation",
        project_root: Path::new("."),
        mcp_url: mcp_url.map(str::to_owned),
        lend: Some(".".to_string()),
    }
}

fn workspace_input() -> SourceInput {
    SourceInput {
        key: "docs".to_string(),
        content: SourceContent::Workspace(SourceWorkspace {
            id: "view-1".to_string(),
            root: ".".to_string(),
        }),
    }
}

fn schema_format(request: &Request) -> (&str, &str) {
    match &request.format {
        Format::Schema(schema) => (&schema.name, &schema.schema),
        other => panic!("expected schema format, got {other:?}"),
    }
}

#[tokio::test]
async fn extract_leg() {
    let model = Scripted::answering([r#"{
            "authority": "documentation",
            "claims": [
                {"kind": "requirement", "id": "password-reset.request", "path": "password-reset.md#L3", "statement": "The account service should let a registered user request a password reset link by email."},
                {"kind": "criterion", "id": "password-reset.request.expiry", "path": "password-reset.md#L7", "criterion": "Reset links expire after 30 minutes."},
                {"kind": "decision", "path": "password-reset.md#L9", "decision": "Use the existing transactional email provider."}
            ]
        }"#]);

    let evidence =
        Adapter::extract(&model, &ctx(Some("http://references/mcp")), &workspace_input())
            .await
            .unwrap();

    assert_eq!(evidence.authority, Authority::Documentation);
    assert_eq!(evidence.claims.len(), 3);
    assert_eq!(evidence.claims[0].kind, ClaimKind::Requirement);
    assert_eq!(evidence.claims[0].id.as_deref(), Some("password-reset.request"));

    // Required extras arrive verbatim: the engine's fail-closed load
    // gate and synthesis both read exactly these keys.
    assert_eq!(
        evidence.claims[0].extras.get("statement").and_then(|value| value.as_str()),
        Some(
            "The account service should let a registered user request a password reset link \
             by email."
        ),
    );
    assert_eq!(
        evidence.claims[1].extras.get("criterion").and_then(|value| value.as_str()),
        Some("Reset links expire after 30 minutes."),
    );
    assert_eq!(
        evidence.claims[2].extras.get("decision").and_then(|value| value.as_str()),
        Some("Use the existing transactional email provider."),
    );

    let requests = model.requests();
    assert_eq!(requests.len(), 1, "extract is a single judgment leg");
    let request = &requests[0];
    let system = request.system.as_deref().unwrap();
    assert!(
        system.starts_with("# `documentation.extract`"),
        "extract prompt is the system channel"
    );
    assert!(system.contains("claim-extras-missing"), "prompt names the fail-closed gate");
    let user = &request.messages[0].content;
    assert!(user.contains("source key `docs`"), "passed source key is named");
    assert!(user.contains("$SOURCE_DIR"), "binding is mapped onto the prompt's vocabulary");
    assert!(user.contains("extract mines only this source"), "nothing else is reachable");
    let (name, schema) = schema_format(request);
    assert_eq!(name, "evidence");
    assert_eq!(schema, evidence_schema());
    assert_eq!(request.workspace.as_deref(), Some("."), "the source view is lent");
    assert_eq!(mcp_grants(request)[0].url, "http://references/mcp");
    assert_eq!(mcp_grants(request)[0].name, "documentation-references");
}

// An inline `value:` binding lends no workspace: the material rides in
// the user message and the judgment leg gets no filesystem grant.
#[tokio::test]
async fn extract_value_no_lend() {
    let model = Scripted::answering([r#"{"authority":"documentation","claims":[]}"#]);
    let input = SourceInput::value("notes", "Reset links expire after 30 minutes.");

    let evidence = Adapter::extract(&model, &ctx(None).without_lend(), &input).await.unwrap();

    assert!(evidence.claims.is_empty());
    let requests = model.requests();
    let request = &requests[0];
    assert_eq!(request.workspace, None, "no lend for an inline value");
    let user = &request.messages[0].content;
    assert!(user.contains("Reset links expire after 30 minutes."), "the value rides inline");
    assert!(user.contains("no `$SOURCE_DIR` is lent"));
}

// A tail-invalid extract answer is repaired: the second leg carries
// the findings and its clean answer is the result.
#[tokio::test]
async fn extract_repaired() {
    let model = Scripted::answering([
        r#"{"authority":"documentation","claims":[{"kind":"requirement"}]}"#,
        r#"{"authority":"documentation","claims":[{"kind":"requirement","id":"password-reset.request","statement":"..."}]}"#,
    ]);

    let evidence =
        Adapter::extract(&model, &ctx(None), &workspace_input()).await.expect("repaired extract");

    assert_eq!(evidence.claims[0].id.as_deref(), Some("password-reset.request"));
    let requests = model.requests();
    assert_eq!(requests.len(), 2, "one repair after the failed tail");
    let repair = &requests[1].messages[0].content;
    assert!(repair.contains("claims require an id"), "repair prompt carries the findings");
    assert!(repair.contains("## Previous answer"), "and the rejected answer");
}

// Exhausting the repair budget surfaces the last failure — a typed
// error, never an empty success.
#[tokio::test]
async fn extract_budget_exhausted() {
    let model = Scripted::answering(
        [r#"{"authority":"documentation","claims":[{"kind":"criterion","id":"Not.Valid"}]}"#;
            1 + MAX_REPAIRS],
    );

    let result = Adapter::extract(&model, &ctx(None), &workspace_input()).await;

    match result {
        Err(Error::Internal(detail)) => {
            assert!(detail.contains("`Not.Valid`"), "detail: {detail}");
        }
        other => panic!("expected the last tail failure, got {other:?}"),
    }
    assert_eq!(model.requests().len(), 1 + MAX_REPAIRS, "initial answer plus the repair budget");
}

#[tokio::test]
async fn extract_no_mcp_no_grant() {
    let model = Scripted::answering([r#"{"authority":"documentation","claims":[]}"#]);

    Adapter::extract(&model, &ctx(None), &workspace_input()).await.unwrap();

    assert!(model.requests()[0].tools.is_empty(), "no URL means no reference grant");
}
