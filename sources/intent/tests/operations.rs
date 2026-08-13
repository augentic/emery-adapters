//! Intent survey / extract operation behavior.

use std::path::Path;

use adapter::Source as _;
use adapter::seam::{Authority, ClaimKind, Context, Error, Lead, SourceInput};
use intent::Adapter;
use omnia_testkit::model::Harness;

fn ctx() -> Context<'static> {
    Context {
        adapter_id: "source:intent",
        project_root: Path::new("."),
        mcp_url: None,
        lend: ".".to_string(),
        source_key: Some("intent".to_string()),
    }
}

fn input() -> SourceInput {
    SourceInput::Inline("Let users reset passwords by email.".to_string())
}

#[tokio::test]
async fn survey_inline_binding() {
    let model = Harness::answering([
        r#"{"leads":[{"lead":"password-reset","synopsis":"Let users reset passwords by email."}]}"#,
    ]);

    let leads = Adapter::survey(&model, &ctx(), &input()).await.unwrap();

    assert_eq!(leads.len(), 1);
    let request = &model.requests()[0];
    assert!(request.system.as_deref().unwrap().starts_with("# intent.survey"));
    let user = &request.messages[0].content;
    assert!(user.contains("Source binding key: `intent`"), "prompt names the binding key");
    assert!(
        user.contains("Let users reset passwords by email."),
        "the inline value is interpolated verbatim"
    );
    assert!(user.contains("slug derived from the intent string"), "lead id is slug-from-intent");
    assert!(user.contains("exactly one lead"), "prompt carries the degenerate cardinality");
}

#[tokio::test]
async fn survey_rejects_tree_input() {
    let model = Harness::answering([r#"{"leads":[]}"#]);
    let tree = SourceInput::Workspace("/prepared".to_string());

    let result = Adapter::survey(&model, &ctx(), &tree).await;

    assert!(matches!(result, Err(Error::InvalidRequest(_))), "got {result:?}");
    assert!(model.requests().is_empty(), "no judgment leg runs on a malformed input");
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

    let evidence = Adapter::extract(&model, &ctx(), &input(), &lead).await.unwrap();

    assert_eq!(evidence.authority, Authority::Intent);
    assert_eq!(evidence.claims.len(), 1);
    assert_eq!(evidence.claims[0].kind, ClaimKind::Intent);
    assert_eq!(evidence.claims[0].id.as_deref(), Some("password-reset"));
    let request = &model.requests()[0];
    assert!(request.system.as_deref().unwrap().starts_with("# intent.extract"));
    let user = &request.messages[0].content;
    assert!(user.contains("Source binding key: `intent`"), "prompt names the binding key");
    assert!(
        user.contains("Let users reset passwords by email."),
        "the inline value is interpolated verbatim"
    );
}
