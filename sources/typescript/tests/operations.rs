//! TypeScript extract operation behavior over the `Source` capability.

use std::path::Path;

use emery_adapter::answers::evidence_schema;
use emery_adapter::types::{
    Authority, ClaimKind, Context, Error, SourceContent, SourceInput, SourceWorkspace,
};
use emery_adapter::{Format, MAX_REPAIRS, Request, SourceAdapter as _};
use omnia_testkit::model::{Harness, mcp_grants};
use typescript::Adapter;

fn ctx(mcp_url: Option<&str>) -> Context<'static> {
    Context {
        adapter_id: "source:typescript",
        project_root: Path::new("."),
        mcp_url: mcp_url.map(str::to_owned),
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
    let model = Harness::answering([r#"{
            "authority": "behaviour",
            "claims": [
                {"kind": "requirement", "id": "user-registration.email-validation", "path": "src/users/register.ts#L12-L34", "statement": "Registration rejects an email that is not RFC-5322 valid with a 400 response."},
                {"kind": "excerpt", "path": "src/users/register.ts#L12-L34", "excerpt": "Handler validates email against RFC-5322 regex."},
                {"kind": "type", "path": "src/users/repository.ts#L1-L4", "signature": "interface User { id: string; email: string; createdAt: Date }"},
                {"kind": "call", "path": "src/users/register.ts#L31", "callee": "src/users/repository.ts:insertUser"}
            ]
        }"#]);

    let evidence =
        Adapter::extract(&model, &ctx(Some("http://references/mcp")), &workspace_input())
            .await
            .unwrap();

    assert_eq!(evidence.authority, Authority::Behaviour);
    assert_eq!(evidence.claims.len(), 4);

    // The behavioural requirement is the reconciliation currency: its
    // required `statement` extra arrives verbatim.
    assert_eq!(evidence.claims[0].kind, ClaimKind::Requirement);
    assert_eq!(evidence.claims[0].id.as_deref(), Some("user-registration.email-validation"));
    assert_eq!(
        evidence.claims[0].extras.get("statement").and_then(|value| value.as_str()),
        Some("Registration rejects an email that is not RFC-5322 valid with a 400 response."),
    );

    assert_eq!(evidence.claims[1].kind, ClaimKind::Excerpt);
    assert_eq!(
        evidence.claims[1].extras.get("excerpt").and_then(|value| value.as_str()),
        Some("Handler validates email against RFC-5322 regex."),
    );
    assert_eq!(evidence.claims[2].kind, ClaimKind::Type);
    assert_eq!(
        evidence.claims[2].extras.get("signature").and_then(|value| value.as_str()),
        Some("interface User { id: string; email: string; createdAt: Date }"),
    );
    assert_eq!(evidence.claims[3].kind, ClaimKind::Call);
    assert_eq!(
        evidence.claims[3].extras.get("callee").and_then(|value| value.as_str()),
        Some("src/users/repository.ts:insertUser"),
    );

    let requests = model.requests();
    assert_eq!(requests.len(), 1, "extract is a single judgment leg");
    let request = &requests[0];
    let system = request.system.as_deref().unwrap();
    assert!(
        system.starts_with("# TypeScript / JavaScript source extract"),
        "extract prompt is the system channel"
    );
    assert!(system.contains("claim-extras-missing"), "prompt names the fail-closed gate");
    let user = &request.messages[0].content;
    assert!(user.contains("source key `legacy-monolith`"), "passed source key is named");
    assert!(user.contains("$SOURCE_DIR"), "binding is mapped onto the prompt's vocabulary");
    assert!(user.contains("extract mines only this source"), "nothing else is reachable");
    assert!(
        user.contains("every spec-worthy behaviour lifted into a `requirement` claim"),
        "the reconciliation-join contract is stated"
    );
    let (name, schema) = schema_format(request);
    assert_eq!(name, "evidence");
    assert_eq!(schema, evidence_schema());
    assert_eq!(request.workspace.as_deref(), Some("."), "the source view is lent");
    let grants = mcp_grants(request);
    assert_eq!(grants[0].url, "http://references/mcp");
    assert_eq!(grants[0].name, "typescript-references");
}

// A tail-invalid extract answer is repaired: the second leg carries
// the findings and its clean answer is the result.
#[tokio::test]
async fn extract_repaired() {
    let model = Harness::answering([
        r#"{"authority":"behaviour","claims":[{"kind":"requirement"}]}"#,
        r#"{"authority":"behaviour","claims":[{"kind":"requirement","id":"session.timeout","statement":"Sessions expire after 15 minutes."}]}"#,
    ]);

    let evidence =
        Adapter::extract(&model, &ctx(None), &workspace_input()).await.expect("repaired extract");

    assert_eq!(evidence.claims[0].id.as_deref(), Some("session.timeout"));
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
    let model = Harness::answering(
        [r#"{"authority":"behaviour","claims":[{"kind":"requirement","id":"Not.Valid"}]}"#;
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
