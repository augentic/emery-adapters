//! TypeScript extract operation behavior over the `Source` capability.

use emery_sdk::model::ToolCall;
use emery_sdk::{Authority, ClaimKind, Context, SourceAdapter as _, SourceContent, SourceInput};
use omnia_test::SeenFormat;
use omnia_test::guest::Scripted;
use typescript::Adapter;

const fn ctx(input: &SourceInput) -> Context<'_> {
    Context {
        adapter_id: "source:typescript",
        input,
    }
}

fn workspace_input() -> SourceInput {
    SourceInput {
        key: "legacy-monolith".to_string(),
        content: SourceContent::Workspace(".".to_string()),
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

    let input = workspace_input();
    let evidence = Adapter::extract(&model, &ctx(&input)).await.unwrap();

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

    let seen = model.seen();
    assert_eq!(seen.len(), 1, "extract is a single judgment leg");
    let request = &seen[0];
    let system = request.system.as_deref().unwrap();
    assert!(
        system.starts_with("# TypeScript / JavaScript source extract"),
        "extract prompt is the system channel"
    );
    assert!(system.contains("bad_request"), "prompt names the fail-closed gate");
    assert!(
        system.contains("Every behavioural fact worth a spec block"),
        "the reconciliation-join contract is stated"
    );
    let user = &request.messages[0];
    assert!(user.contains("source key `legacy-monolith`"), "passed source key is named");
    assert!(user.contains("$SOURCE_DIR"), "source is mapped onto the prompt's vocabulary");
    assert!(
        user.contains("the TypeScript / JavaScript source tree"),
        "the tree is named by its source"
    );
    assert!(user.contains("extract mines only this source"), "nothing else is reachable");
    assert!(user.contains("`read_doc` tool"), "the reference pull affordance is named");
    let SeenFormat::Schema { name, schema } = &request.format else {
        panic!("expected schema format, got {:?}", request.format)
    };
    assert_eq!(name, "evidence");
    let schema: serde_json::Value = serde_json::from_str(schema).expect("the schema is JSON");
    assert!(
        schema.pointer("/$defs/Claim/properties/id/pattern").is_some(),
        "the claim-id grammar steers the answer"
    );
    assert!(request.check, "acceptance is the SDK's claim gate, not the reply text");
    assert_eq!(request.workspace.as_deref(), Some("."), "the source view is lent");
    assert_eq!(request.tools, ["list_docs", "read_doc"], "the reference tools are declared");
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

    let input = workspace_input();
    Adapter::extract(&model, &ctx(&input)).await.unwrap();

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
