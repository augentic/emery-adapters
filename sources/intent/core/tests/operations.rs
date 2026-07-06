//! Intent-specific operation behavior: the degenerate inline binding.

use std::path::Path;

use specify_guest_kit::MockModel;
use specify_guest_kit::seam::{Authority, ClaimKind, Context, Lead};
use specify_intent_core::operations::{extract, survey};

fn ctx() -> Context<'static> {
    Context {
        adapter_id: "source:intent",
        project_root: Path::new("."),
        mcp_url: None,
    }
}

// The survey prompt frames intent's degenerate binding: the operator's
// intent string rides inline as the binding's `value`, no source tree is
// bound, and the single lead's id is the plan-derived slice name.
#[tokio::test]
async fn survey_prompt_frames_the_inline_binding() {
    let model = MockModel::answering([
        r#"{"leads":[{"lead":"password-reset","synopsis":"Let users reset passwords by email."}]}"#,
    ]);

    let leads = survey(&model, &ctx()).await.unwrap();

    assert_eq!(leads.len(), 1);
    let request = &model.requests()[0];
    assert!(request.system.as_deref().unwrap().starts_with("# intent.survey"));
    let user = &request.messages[0].content;
    assert!(user.contains("inline `value`"), "prompt names the inline binding");
    assert!(user.contains("`path` is absent"), "prompt says no source tree is bound");
    assert!(user.contains("exactly one lead"), "prompt carries the degenerate cardinality");
}

// The extract answer echoes the operator's intent as the single
// `kind: intent` claim under `authority: intent`.
#[tokio::test]
async fn extract_parses_the_intent_claim() {
    let model = MockModel::answering([
        r#"{"authority":"intent","claims":[{"kind":"intent","id":"password-reset","statement":"Let users reset passwords by email."}]}"#,
    ]);
    let lead = Lead {
        lead: "password-reset".to_string(),
        synopsis: "Let users reset passwords by email.".to_string(),
        topics: Vec::new(),
    };

    let evidence = extract(&model, &ctx(), &lead).await.unwrap();

    assert_eq!(evidence.authority, Authority::Intent);
    assert_eq!(evidence.claims.len(), 1);
    assert_eq!(evidence.claims[0].kind, ClaimKind::Intent);
    assert_eq!(evidence.claims[0].id.as_deref(), Some("password-reset"));
    let request = &model.requests()[0];
    assert!(request.system.as_deref().unwrap().starts_with("# intent.extract"));
}
