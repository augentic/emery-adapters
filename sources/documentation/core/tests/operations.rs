//! The survey / extract judgment operations against the scripted
//! [`MockModel`]: prompt assembly, schema-gated formats, answer
//! deserialization, and the deterministic validation tails.

use std::path::Path;

use adapter::answers::{EVIDENCE_ANSWER_SCHEMA, LEADS_ANSWER_SCHEMA};
use adapter::seam::{Authority, ClaimKind, Context, Error, Lead};
use adapter::{Error as ModelError, Format, MockModel, Request};
use documentation_core::operations::{describe, extract, survey};

fn ctx(mcp_url: Option<&str>) -> Context<'_> {
    Context {
        adapter_id: "source:documentation",
        project_root: Path::new("."),
        mcp_url,
    }
}

fn lead() -> Lead {
    Lead {
        lead: "password-reset".to_string(),
        synopsis: "Reset flow with expiring links.".to_string(),
        topics: vec!["identity".to_string()],
    }
}

fn schema_format(request: &Request) -> (&str, &str) {
    match &request.format {
        Format::Schema(schema) => (&schema.name, &schema.schema),
        other => panic!("expected schema format, got {other:?}"),
    }
}

// One leg: the embedded survey prompt is the system channel, the user
// message carries the call context (adapter id, plan.yaml binding
// resolution, the re-survey note, the JSON envelope instruction), and the
// request rides the leads schema pin, the adapter's own MCP grant, and
// the workspace lend.
#[tokio::test]
async fn survey_assembles_prompt_and_parses() {
    let model = MockModel::answering([
        r#"{"leads":[{"lead":"password-reset","synopsis":"Reset flow.","topics":["identity"]}]}"#,
    ]);

    let leads = survey(&model, &ctx(Some("http://shelf/mcp"))).await.unwrap();

    assert_eq!(leads.len(), 1);
    assert_eq!(leads[0].lead, "password-reset");
    assert_eq!(leads[0].topics, vec!["identity"]);

    let requests = model.requests();
    assert_eq!(requests.len(), 1, "survey is a single judgment leg");
    let request = &requests[0];
    let system = request.system.as_deref().unwrap();
    assert!(system.starts_with("# `documentation.survey`"), "survey prompt is the system channel");
    let user = &request.messages[0].content;
    assert!(user.contains("source:documentation"), "user message names the adapter id");
    assert!(user.contains("plan.yaml") && user.contains("sources.<key>"), "binding resolution");
    assert!(user.contains("$SOURCE_DIR"), "binding is mapped onto the prompt's vocabulary");
    assert!(user.contains("re-survey"), "re-survey framing is carried");
    let (name, schema) = schema_format(request);
    assert_eq!(name, "leads");
    assert_eq!(schema, LEADS_ANSWER_SCHEMA);
    assert!(request.lend_workspace);
    assert_eq!(request.mcp[0].url, "http://shelf/mcp");
    assert_eq!(request.mcp[0].name, "documentation-references");
}

// Without a resolved MCP endpoint the leg still runs — just grant-free.
#[tokio::test]
async fn survey_without_mcp_url_offers_no_grant() {
    let model = MockModel::answering([r#"{"leads":[]}"#]);

    survey(&model, &ctx(None)).await.unwrap();

    assert!(model.requests()[0].mcp.is_empty(), "no URL means no reference grant");
}

// The deterministic tail re-checks the kebab-case id grammar after the
// answer lands: a malformed lead id fails as a findings-style internal
// error even though it deserialized.
#[tokio::test]
async fn survey_tail_rejects_malformed_lead_id() {
    let model =
        MockModel::answering([r#"{"leads":[{"lead":"Bad_Id","synopsis":"Casing violates."}]}"#]);

    let err = survey(&model, &ctx(None)).await.unwrap_err();

    match err {
        Error::Internal(detail) => {
            assert!(detail.contains("lead `Bad_Id`"), "finding names the malformed id: {detail}");
        }
        other => panic!("expected internal error, got {other:?}"),
    }
}

// A whitespace-only synopsis fails the tail the same way.
#[tokio::test]
async fn survey_tail_rejects_empty_synopsis() {
    let model = MockModel::answering([r#"{"leads":[{"lead":"account","synopsis":"  "}]}"#]);

    let err = survey(&model, &ctx(None)).await.unwrap_err();

    assert!(matches!(err, Error::Internal(detail) if detail.contains("synopsis is empty")));
}

// One leg: the embedded extract prompt is the system channel, the user
// message carries the lead block plus the binding resolution, and the
// answer deserializes into the Evidence shape through the evidence
// schema pin.
#[tokio::test]
async fn extract_assembles_prompt_and_parses() {
    let model = MockModel::answering([r#"{
            "authority": "documentation",
            "claims": [
                {"kind": "requirement", "id": "password-reset.request", "path": "password-reset.md#L3"},
                {"kind": "decision", "path": "password-reset.md#L9"}
            ]
        }"#]);

    let evidence = extract(&model, &ctx(Some("http://shelf/mcp")), &lead()).await.unwrap();

    assert_eq!(evidence.authority, Authority::Documentation);
    assert_eq!(evidence.claims.len(), 2);
    assert_eq!(evidence.claims[0].kind, ClaimKind::Requirement);
    assert_eq!(evidence.claims[0].id.as_deref(), Some("password-reset.request"));

    let requests = model.requests();
    assert_eq!(requests.len(), 1, "extract is a single judgment leg");
    let request = &requests[0];
    let system = request.system.as_deref().unwrap();
    assert!(
        system.starts_with("# `documentation.extract`"),
        "extract prompt is the system channel"
    );
    let user = &request.messages[0].content;
    assert!(user.contains("- lead: password-reset"), "user message carries the lead id");
    assert!(user.contains("Reset flow with expiring links."), "and its synopsis");
    assert!(user.contains("- topics: [identity]"), "and its topics");
    assert!(user.contains("plan.yaml"), "binding resolution is carried");
    let (name, schema) = schema_format(request);
    assert_eq!(name, "evidence");
    assert_eq!(schema, EVIDENCE_ANSWER_SCHEMA);
    assert!(request.lend_workspace);
    assert_eq!(request.mcp[0].url, "http://shelf/mcp");
}

// The deterministic tail mirrors the evidence schema's conditional id
// requirement: a requirement claim without an id fails even though the
// answer deserialized.
#[tokio::test]
async fn extract_tail_rejects_missing_claim_id() {
    let model = MockModel::answering([
        r#"{"authority":"documentation","claims":[{"kind":"requirement"}]}"#,
    ]);

    let err = extract(&model, &ctx(None), &lead()).await.unwrap_err();

    assert!(matches!(err, Error::Internal(detail) if detail.contains("require an id")));
}

// A claim id outside the dotted-kebab pattern fails the tail.
#[tokio::test]
async fn extract_tail_rejects_malformed_claim_id() {
    let model = MockModel::answering([
        r#"{"authority":"documentation","claims":[{"kind":"criterion","id":"Not.Valid"}]}"#,
    ]);

    let err = extract(&model, &ctx(None), &lead()).await.unwrap_err();

    assert!(matches!(err, Error::Internal(detail) if detail.contains("`Not.Valid`")));
}

// Model failures map through the seam error vocabulary.
#[tokio::test]
async fn model_invalid_request_maps_through() {
    let model =
        MockModel::scripted([Err(ModelError::InvalidRequest("messages must not be empty".into()))]);

    let err = survey(&model, &ctx(None)).await.unwrap_err();

    assert!(matches!(err, Error::InvalidRequest(_)));
}

// The RFC-64 self-description is answerable without a model or a
// filesystem: no compatibility floor is declared.
#[test]
fn describe_declares_no_floor() {
    assert_eq!(describe().specify_floor, None);
}
