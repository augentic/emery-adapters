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
async fn survey_single_file_workspace() {
    let model = Harness::answering([
        r#"{"leads":[{"lead":"password-reset","synopsis":"Let users reset passwords by email."}]}"#,
    ]);
    let root = tempfile::tempdir().unwrap();
    let nested = root.path().join("nested");
    std::fs::create_dir(&nested).unwrap();
    std::fs::write(nested.join("intent.md"), "Let users reset passwords by email.").unwrap();
    let tree = SourceInput::Workspace(root.path().display().to_string());

    let leads = Adapter::survey(&model, &ctx(), &tree).await.unwrap();

    assert_eq!(leads.len(), 1);
    let user = &model.requests()[0].messages[0].content;
    assert!(
        user.contains("Let users reset passwords by email."),
        "the located file's contents are interpolated as the intent string"
    );
}

#[tokio::test]
async fn survey_rejects_multi_file_workspace() {
    let model = Harness::answering([r#"{"leads":[]}"#]);
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("one.md"), "first").unwrap();
    std::fs::write(root.path().join("two.md"), "second").unwrap();
    let tree = SourceInput::Workspace(root.path().display().to_string());

    let result = Adapter::survey(&model, &ctx(), &tree).await;

    assert!(matches!(result, Err(Error::InvalidRequest(_))), "got {result:?}");
    assert!(model.requests().is_empty(), "no judgment leg runs on a malformed input");
}

#[tokio::test]
async fn survey_rejects_empty_workspace() {
    let model = Harness::answering([r#"{"leads":[]}"#]);
    let root = tempfile::tempdir().unwrap();
    let tree = SourceInput::Workspace(root.path().display().to_string());

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
