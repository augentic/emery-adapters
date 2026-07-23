//! Documentation survey / extract operation behavior.

use std::path::Path;

use adapter::answers::{EVIDENCE_ANSWER_SCHEMA, LEADS_ANSWER_SCHEMA};
use adapter::seam::{Authority, ClaimKind, Context, Error, Lead};
use adapter::{Format, MAX_REPAIRS, Request, Source as _};
use documentation::Adapter;
use omnia_testkit::model::{Harness, mcp_grants};

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
    let model = Harness::answering([
        r#"{"leads":[{"lead":"password-reset","synopsis":"Reset flow.","topics":["identity"]}]}"#,
    ]);

    let leads = Adapter::survey(&model, &ctx(Some("http://references/mcp"))).await.unwrap();

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

// A tail-invalid survey answer is repaired: the second leg carries the
// findings and its clean answer is the result.
#[tokio::test]
async fn survey_repaired() {
    let model = Harness::answering([
        r#"{"leads":[{"lead":"Bad_Id","synopsis":"Casing violates the kebab grammar."}]}"#,
        r#"{"leads":[{"lead":"password-reset","synopsis":"Reset flow."}]}"#,
    ]);

    let leads = Adapter::survey(&model, &ctx(None)).await.expect("repaired survey succeeds");

    assert_eq!(leads.len(), 1);
    assert_eq!(leads[0].lead, "password-reset");
    let requests = model.requests();
    assert_eq!(requests.len(), 2, "one repair after the failed tail");
    let repair = &requests[1].messages[0].content;
    assert!(repair.contains("lead `Bad_Id`"), "repair prompt carries the findings: {repair}");
    assert!(repair.contains("## Previous answer"), "and the rejected answer");
}

// A survey answer that never passes the tail exhausts the repair
// budget and surfaces the last failure.
#[tokio::test]
async fn survey_budget_exhausted() {
    let model = Harness::answering(
        [r#"{"leads":[{"lead":"still-bad","synopsis":"   "}]}"#; 1 + MAX_REPAIRS],
    );

    let result = Adapter::survey(&model, &ctx(None)).await;

    match result {
        Err(Error::Internal(detail)) => {
            assert!(detail.contains("synopsis is empty"), "detail: {detail}");
        }
        other => panic!("expected the last tail failure, got {other:?}"),
    }
    assert_eq!(model.requests().len(), 1 + MAX_REPAIRS, "initial answer plus the repair budget");
}

#[tokio::test]
async fn survey_no_mcp_no_grant() {
    let model = Harness::answering([r#"{"leads":[]}"#]);

    Adapter::survey(&model, &ctx(None)).await.unwrap();

    assert!(model.requests()[0].tools.is_empty(), "no URL means no reference grant");
}

#[tokio::test]
async fn extract_leg() {
    let model = Harness::answering([r#"{
            "authority": "documentation",
            "claims": [
                {"kind": "requirement", "id": "password-reset.request", "path": "password-reset.md#L3"},
                {"kind": "decision", "path": "password-reset.md#L9"}
            ]
        }"#]);

    let evidence =
        Adapter::extract(&model, &ctx(Some("http://references/mcp")), &lead()).await.unwrap();

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

// A tail-invalid extract answer is repaired: the second leg carries
// the findings and its clean answer is the result.
#[tokio::test]
async fn extract_repaired() {
    let model = Harness::answering([
        r#"{"authority":"documentation","claims":[{"kind":"requirement"}]}"#,
        r#"{"authority":"documentation","claims":[{"kind":"requirement","id":"password-reset.request"}]}"#,
    ]);

    let evidence =
        Adapter::extract(&model, &ctx(None), &lead()).await.expect("repaired extract succeeds");

    assert_eq!(evidence.claims[0].id.as_deref(), Some("password-reset.request"));
    let requests = model.requests();
    assert_eq!(requests.len(), 2, "one repair after the failed tail");
    let repair = &requests[1].messages[0].content;
    assert!(repair.contains("claims require an id"), "repair prompt carries the findings");
    assert!(repair.contains("## Previous answer"), "and the rejected answer");
}

// An extract answer that never passes the tail exhausts the repair
// budget and surfaces the last failure.
#[tokio::test]
async fn extract_budget_exhausted() {
    let model = Harness::answering(
        [r#"{"authority":"documentation","claims":[{"kind":"criterion","id":"Not.Valid"}]}"#;
            1 + MAX_REPAIRS],
    );

    let result = Adapter::extract(&model, &ctx(None), &lead()).await;

    match result {
        Err(Error::Internal(detail)) => {
            assert!(detail.contains("`Not.Valid`"), "detail: {detail}");
        }
        other => panic!("expected the last tail failure, got {other:?}"),
    }
    assert_eq!(model.requests().len(), 1 + MAX_REPAIRS, "initial answer plus the repair budget");
}
