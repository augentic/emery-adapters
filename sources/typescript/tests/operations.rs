//! TypeScript extract operation behavior over the `Source` capability.

use emery_adapter::types::{Authority, ClaimKind, Context, SourceInput};
use emery_adapter::{Error, Format, Request, SourceAdapter as _, ToolCall};
use emery_prose::registry::Doc;
use omnia_test::guest::{Scripted, function_tools};
use typescript::Adapter;

fn ctx(docs: &'static [Doc]) -> Context<'static> {
    Context {
        adapter_id: "source:typescript",
        docs,
        lend: Some(".".to_string()),
    }
}

fn workspace_input() -> SourceInput {
    SourceInput::workspace("legacy-monolith", ".")
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
            "authority": "behaviour",
            "claims": [
                {"kind": "requirement", "id": "user-registration.email-validation", "path": "src/users/register.ts#L12-L34", "statement": "Registration rejects an email that is not RFC-5322 valid with a 400 response."},
                {"kind": "excerpt", "path": "src/users/register.ts#L12-L34", "excerpt": "Handler validates email against RFC-5322 regex."},
                {"kind": "type", "path": "src/users/repository.ts#L1-L4", "signature": "interface User { id: string; email: string; createdAt: Date }"},
                {"kind": "call", "path": "src/users/register.ts#L31", "callee": "src/users/repository.ts:insertUser"}
            ]
        }"#]);

    let evidence =
        Adapter::extract(&model, &ctx(Adapter::docs()), &workspace_input()).await.unwrap();

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
    assert!(system.contains("bad_request"), "prompt names the fail-closed gate");
    let user = &request.messages[0].content;
    assert!(user.contains("source key `legacy-monolith`"), "passed source key is named");
    assert!(user.contains("$SOURCE_DIR"), "source is mapped onto the prompt's vocabulary");
    assert!(user.contains("extract mines only this source"), "nothing else is reachable");
    assert!(user.contains("`read_doc` tool"), "the reference pull affordance is named");
    assert!(
        user.contains("every spec-worthy behaviour lifted into a `requirement` claim"),
        "the reconciliation-join contract is stated"
    );
    let (name, schema) = schema_format(request);
    assert_eq!(name, "evidence");
    let schema: serde_json::Value = serde_json::from_str(schema).expect("the schema is JSON");
    assert!(
        schema.pointer("/$defs/Claim/properties/id/pattern").is_some(),
        "the claim-id grammar steers the answer"
    );
    assert!(request.check, "acceptance is the SDK's claim gate, not the reply text");
    assert_eq!(request.workspace.as_deref(), Some("."), "the source view is lent");
    let tools: Vec<&str> =
        function_tools(request).into_iter().map(|tool| tool.name.as_str()).collect();
    assert_eq!(tools, ["list_docs", "read_doc"], "the reference tools are declared");
}

// A scripted `read_doc` call round-trips through the judgment's tool
// closure: the answer is the embedded reference body as a JSON object.
#[tokio::test]
async fn ref_pull() {
    let docs = Adapter::docs();
    let doc = docs
        .iter()
        .find(|doc| doc.path.starts_with("references/"))
        .expect("typescript embeds reference documents");
    let model = Scripted::answering([r#"{"authority":"behaviour","claims":[]}"#]).calling(
        0,
        [ToolCall {
            id: "call-1".to_string(),
            name: "read_doc".to_string(),
            arguments: serde_json::json!({ "path": doc.path }).to_string(),
        }],
    );

    Adapter::extract(&model, &ctx(docs), &workspace_input()).await.unwrap();

    let exchanges = model.exchanges();
    assert_eq!(exchanges.len(), 2, "the closure answered the scripted call, then the check ran");
    assert_eq!(exchanges[0].tool, "read_doc");
    let body = exchanges[0].outcome.as_ref().expect("read_doc resolves an embedded path");
    let value: serde_json::Value = serde_json::from_str(body).expect("a JSON-object result");
    assert_eq!(value["path"], doc.path);
    assert_eq!(value["body"], doc.body);
    assert_eq!(exchanges[1].tool, "check");
    assert_eq!(exchanges[1].outcome, Ok(String::new()), "the empty claim set passes the gate");
}

// A candidate the claim gate rejects is corrected in place: the check
// hands the findings back with the rejected answer, and the next
// candidate's clean answer is the result.
#[tokio::test]
async fn extract_repaired() {
    let model = Scripted::answering([
        r#"{"authority":"behaviour","claims":[{"kind":"requirement"}]}"#,
        r#"{"authority":"behaviour","claims":[{"kind":"requirement","id":"session.timeout","statement":"Sessions expire after 15 minutes."}]}"#,
    ]);

    let evidence =
        Adapter::extract(&model, &ctx(&[]), &workspace_input()).await.expect("repaired extract");

    assert_eq!(evidence.claims[0].id.as_deref(), Some("session.timeout"));
    assert_eq!(model.requests().len(), 2, "one scripted answer per attempt");
    let exchanges = model.exchanges();
    assert_eq!(exchanges.len(), 2, "one rejected check, one accepted");
    assert_eq!(exchanges[0].tool, "check");
    let correction = exchanges[0].outcome.as_ref().expect_err("the first candidate is rejected");
    assert!(correction.contains("claims require an id"), "the correction carries the findings");
    assert!(correction.contains("## Previous answer (rejected)"), "and the rejected answer");
    assert_eq!(exchanges[1].outcome, Ok(String::new()));
}

// A backend out of rounds surfaces the last failure — a `bad_request`
// carrying the findings, never an empty success.
#[tokio::test]
async fn spent_budget() {
    let model = Scripted::answering([
        r#"{"authority":"behaviour","claims":[{"kind":"requirement","id":"Not.Valid"}]}"#,
    ]);

    let result = Adapter::extract(&model, &ctx(&[]), &workspace_input()).await;

    match result {
        Err(Error::BadRequest { description, .. }) => {
            assert!(description.contains("`Not.Valid`"), "description: {description}");
        }
        other => panic!("expected the last gate failure, got {other:?}"),
    }
    let exchanges = model.exchanges();
    assert_eq!(exchanges.len(), 1, "one check, rejected");
    let correction = exchanges[0].outcome.as_ref().expect_err("the only candidate is rejected");
    assert!(correction.contains("`Not.Valid`"), "{correction}");
}
