//! Every shipped component over `emery:adapter/source`
//!
//! Each `sources/*` adapter runs under the omnia runtime, driven by the
//! `source_extract` program against a scripted host model. The driver
//! asserts what crosses the seam; this side asserts what the host alone
//! sees of the component: `metadata` opened no completion, and each
//! `extract`'s system prompt is the `prompts/extract.md` this build
//! embedded. The SDK's side of the seam is `probe.rs`'s; an adapter's own
//! behaviour is its `tests/extract.rs`'s.

#![cfg(not(target_arch = "wasm32"))]

mod support;

use emery_sdk::Authority;
use omnia_test::host::{Scratch, ScriptedModel, scratch};

// Every `sources/*` component must have a matching test here.
test_programs::foreach_adapter!();

/// A gate-valid answer under `authority` — the adapter's own class, though
/// the gate reads none.
fn evidence(authority: Authority) -> String {
    serde_json::json!({
        "authority": authority,
        "claims": [{
            "kind": "requirement",
            "id": "orders.create",
            "path": "docs/orders.md#L3",
            "statement": "POST /orders creates an order."
        }]
    })
    .to_string()
}

/// The driver's answered legs against `component`, one answer per `extract`.
async fn extract(component: &str, authority: Authority, project: &Scratch) -> ScriptedModel {
    let answer = evidence(authority);
    support::run(component, project, &[], ScriptedModel::answering([&answer, &answer])).await
}

/// `metadata` opened no completion; each `extract` opened one with `prompt`
/// as its system.
fn prompted(model: &ScriptedModel, prompt: &str) {
    let seen = model.seen();
    assert_eq!(seen.len(), 2, "metadata opens no completion; each extract opens one");
    for request in &seen {
        assert_eq!(request.system.as_deref(), Some(prompt), "the compiled-in prompt is the system");
    }
}

#[tokio::test]
async fn documentation() {
    let project = scratch();
    project.write("docs/orders.md", "# Orders\n\nPOST /orders creates an order.\n");

    let model =
        extract(test_programs::ADAPTER_DOCUMENTATION, Authority::Documentation, &project).await;

    prompted(&model, include_str!("../sources/documentation/prose/prompts/extract.md"));
}

// The one adapter that reads its source inside the guest, through the mount.
#[tokio::test]
async fn intent() {
    const BRIEF: &str = "Let users reset passwords by email.";
    let project = scratch();
    project.write("brief.md", BRIEF);

    let model = extract(test_programs::ADAPTER_INTENT, Authority::Intent, &project).await;

    prompted(&model, include_str!("../sources/intent/prose/prompts/extract.md"));
    let turn = &model.seen()[0].messages[0];
    assert!(turn.contains(BRIEF), "the brief read through the mount is the material: {turn}");
}

#[tokio::test]
async fn typescript() {
    let project = scratch();
    project.write("src/index.ts", "export function greet(): string { return 'hello'; }\n");

    let model = extract(test_programs::ADAPTER_TYPESCRIPT, Authority::Behaviour, &project).await;

    prompted(&model, include_str!("../sources/typescript/prose/prompts/extract.md"));
}
