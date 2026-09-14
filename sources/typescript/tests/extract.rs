//! TypeScript's own extract behaviour
//!
//! The one thing the adapter states itself: the noun its bound tree goes by
//! in the turn.

use emery_sdk::{Context, SourceAdapter as _, SourceContent, SourceInput};
use omnia_test::guest::Scripted;
use typescript::Adapter;

#[tokio::test]
async fn bound_tree() {
    let model = Scripted::answering([r#"{"claims":[]}"#]);
    let input = SourceInput {
        key: "legacy-monolith".to_string(),
        content: SourceContent::Workspace(".".to_string()),
    };
    let ctx = Context {
        adapter_id: "source:typescript",
        input: &input,
    };

    let evidence = Adapter::extract(&model, &ctx).await.expect("the scripted answer is accepted");
    assert_eq!(evidence.kind, Adapter::KIND);

    let turn = &model.seen()[0].messages[0];
    assert!(turn.contains("the TypeScript / JavaScript source tree"), "{turn}");
}
