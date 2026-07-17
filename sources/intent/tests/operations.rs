//! Intent survey / extract operation behavior.

use std::path::Path;

use adapter::Source as _;
use adapter::seam::{Authority, ClaimKind, Context, Lead};
use intent::Adapter;
use testkit::Harness;

fn ctx() -> Context<'static> {
    Context {
        adapter_id: "source:intent",
        project_root: Path::new("."),
        mcp_url: None,
    }
}

#[tokio::test]
async fn survey_inline_binding() {
    let model = Harness::answering([
        r#"{"leads":[{"lead":"password-reset","synopsis":"Let users reset passwords by email."}]}"#,
    ]);

    let leads = Adapter::survey(&model, &ctx()).await.unwrap();

    assert_eq!(leads.len(), 1);
    let request = &model.requests()[0];
    assert!(request.system.as_deref().unwrap().starts_with("# intent.survey"));
    let user = &request.messages[0].content;
    assert!(user.contains("inline `value`"), "prompt names the inline binding");
    assert!(user.contains("`path` is absent"), "prompt says no source tree is bound");
    assert!(user.contains("exactly one lead"), "prompt carries the degenerate cardinality");
}

#[tokio::test]
async fn extract_intent_claim() {
    let model = Harness::answering([
        r#"{"authority":"intent","claims":[{"kind":"intent","id":"password-reset","statement":"Let users reset passwords by email."}]}"#,
    ]);
    let lead = Lead {
        lead: "password-reset".to_string(),
        synopsis: "Let users reset passwords by email.".to_string(),
        topics: Vec::new(),
    };

    let evidence = Adapter::extract(&model, &ctx(), &lead).await.unwrap();

    assert_eq!(evidence.authority, Authority::Intent);
    assert_eq!(evidence.claims.len(), 1);
    assert_eq!(evidence.claims[0].kind, ClaimKind::Intent);
    assert_eq!(evidence.claims[0].id.as_deref(), Some("password-reset"));
    let request = &model.requests()[0];
    assert!(request.system.as_deref().unwrap().starts_with("# intent.extract"));
}
