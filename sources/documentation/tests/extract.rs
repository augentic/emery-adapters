//! Documentation's own extract behaviour, natively over a scripted model:
//! the one thing the adapter states itself — the noun its bound tree goes by
//! in the turn. The SDK's suite owns the request shape every adapter shares;
//! the root seam suites own what crosses the component boundary and the
//! corpus it embeds.

use documentation::Adapter;
use emery_sdk::{Context, SourceAdapter as _, SourceContent, SourceInput};
use omnia_test::guest::Scripted;

// A bound tree is put to the model as the documentation source tree.
#[tokio::test]
async fn bound_tree() {
    let model = Scripted::answering([r#"{"authority":"documentation","claims":[]}"#]);
    let input = SourceInput {
        key: "docs".to_string(),
        content: SourceContent::Workspace(".".to_string()),
    };
    let ctx = Context {
        adapter_id: "source:documentation",
        input: &input,
    };

    Adapter::extract(&model, &ctx).await.expect("the scripted answer is accepted");

    let turn = &model.seen()[0].messages[0];
    assert!(turn.contains("the documentation source tree"), "{turn}");
}
