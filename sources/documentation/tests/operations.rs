//! Documentation extract operation behavior over the `Source` capability.

use documentation::Adapter;
use emery_adapter::SourceAdapter as _;
use emery_adapter::types::{Authority, ClaimKind, Context, SourceInput};
use emery_prose::registry::Doc;
use omnia_test::SeenFormat;
use omnia_test::guest::Scripted;

const fn ctx(docs: &'static [Doc]) -> Context<'static> {
    Context {
        adapter_id: "source:documentation",
        docs,
        lend: Some("."),
    }
}

fn workspace_input() -> SourceInput {
    SourceInput::workspace("docs", ".")
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
        Adapter::extract(&model, &ctx(Adapter::docs()), &workspace_input()).await.unwrap();

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

    let seen = model.seen();
    assert_eq!(seen.len(), 1, "extract is a single judgment leg");
    let request = &seen[0];
    let system = request.system.as_deref().unwrap();
    assert!(
        system.starts_with("# `documentation.extract`"),
        "extract prompt is the system channel"
    );
    assert!(system.contains("bad_request"), "prompt names the fail-closed gate");
    let user = &request.messages[0];
    assert!(user.contains("source key `docs`"), "passed source key is named");
    assert!(user.contains("$SOURCE_DIR"), "source is mapped onto the prompt's vocabulary");
    assert!(user.contains("extract mines only this source"), "nothing else is reachable");
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

// An inline `value:` source lends no workspace: the material rides in
// the user message and the judgment leg gets no filesystem grant.
#[tokio::test]
async fn no_lend() {
    let model = Scripted::answering([r#"{"authority":"documentation","claims":[]}"#]);
    let input = SourceInput::value("notes", "Reset links expire after 30 minutes.");

    let ctx = Context {
        lend: None,
        ..ctx(&[])
    };
    let evidence = Adapter::extract(&model, &ctx, &input).await.unwrap();

    assert!(evidence.claims.is_empty());
    let seen = model.seen();
    let request = &seen[0];
    assert_eq!(request.workspace, None, "no lend for an inline value");
    let user = &request.messages[0];
    assert!(user.contains("Reset links expire after 30 minutes."), "the value rides inline");
    assert!(user.contains("no `$SOURCE_DIR` is lent"));
}

// A docs-free context declares no tools: the judgment stays single-shot.
#[tokio::test]
async fn no_docs() {
    let model = Scripted::answering([r#"{"authority":"documentation","claims":[]}"#]);

    Adapter::extract(&model, &ctx(&[]), &workspace_input()).await.unwrap();

    assert!(model.seen()[0].tools.is_empty(), "no docs means no reference tools");
}
