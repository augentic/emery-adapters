//! The survey / extract judgment operations against the scripted
//! [`MockModel`]: prompt assembly, schema-gated formats, answer
//! deserialization, and the deterministic validation tails.

use std::path::Path;

use adapter::answers::{EVIDENCE_ANSWER_SCHEMA, LEADS_ANSWER_SCHEMA};
use adapter::seam::{Authority, ClaimKind, Context, Lead};
use adapter::{Format, Request};
use documentation::operations::{extract, survey};
use specify_testkit::{MockModel, mcp_grants};

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

#[tokio::test]
async fn survey_leg() {
    let model = MockModel::answering([
        r#"{"leads":[{"lead":"password-reset","synopsis":"Reset flow.","topics":["identity"]}]}"#,
    ]);

    let leads = survey(&model, &ctx(Some("http://references/mcp"))).await.unwrap();

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
    let grants = mcp_grants(request);
    assert_eq!(grants[0].url, "http://references/mcp");
    assert_eq!(grants[0].name, "documentation-references");
}

#[tokio::test]
async fn survey_no_mcp_no_grant() {
    let model = MockModel::answering([r#"{"leads":[]}"#]);

    survey(&model, &ctx(None)).await.unwrap();

    assert!(model.requests()[0].tools.is_empty(), "no URL means no reference grant");
}

#[tokio::test]
async fn extract_leg() {
    let model = MockModel::answering([r#"{
            "authority": "documentation",
            "claims": [
                {"kind": "requirement", "id": "password-reset.request", "path": "password-reset.md#L3"},
                {"kind": "decision", "path": "password-reset.md#L9"}
            ]
        }"#]);

    let evidence = extract(&model, &ctx(Some("http://references/mcp")), &lead()).await.unwrap();

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
    assert_eq!(mcp_grants(request)[0].url, "http://references/mcp");
}
