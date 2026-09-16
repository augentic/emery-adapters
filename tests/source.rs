//! Runs every shipped component over `emery:adapter/source` under the runtime.
//!
//! Each `sources/*` adapter runs under the omnia runtime, driven by the
//! `source_extract` program against a scripted host model. The driver
//! asserts what crosses the boundary; this side asserts what the host alone
//! sees of the component: `metadata` opened no completion, and each
//! `extract`'s system prompt is the `prompts/extract.md` this build
//! embedded. The SDK's side of the boundary is `probe.rs`'s; an adapter's own
//! behaviour is its `tests/survey.rs`'s.

#![cfg(not(target_arch = "wasm32"))]

mod support;

use omnia_test::host::{Scratch, ScriptedModel, scratch};

// Every `sources/*` component must have a matching test here.
test_programs::foreach_adapter!();

/// A gate-valid answer of claims alone.
///
/// The kind of source rides `metadata`, never the document.
fn answer() -> String {
    serde_json::json!({
        "claims": [{
            "kind": "requirement",
            "id": "orders.create",
            "path": "docs/orders.md#L3",
            "statement": "POST /orders creates an order."
        }]
    })
    .to_string()
}

/// Runs the driver's answered legs against `component`, one answer per `extract`.
async fn extract(component: &str, project: &Scratch) -> ScriptedModel {
    let answer = answer();
    support::run(component, project, &[], ScriptedModel::answering([&answer, &answer])).await
}

/// Asserts the completions the host saw: none from `metadata`, one per `extract`.
///
/// Each `extract`'s completion carries `prompt` as its system.
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

    let model = extract(test_programs::ADAPTER_DOCUMENTATION, &project).await;

    prompted(&model, include_str!("../sources/documentation/prose/prompts/extract.md"));
}

// The one adapter that reads its source inside the guest, through the mount.
#[tokio::test]
async fn intent() {
    const BRIEF: &str = "Let users reset passwords by email.";
    let project = scratch();
    project.write("brief.md", BRIEF);

    let model = extract(test_programs::ADAPTER_INTENT, &project).await;

    prompted(&model, include_str!("../sources/intent/prose/prompts/extract.md"));
    let turn = &model.seen()[0].messages[0];
    assert!(turn.contains(BRIEF), "the brief read through the mount is the seam: {turn}");
}

// The one adapter that surveys by model: its workspace `extract` opens a
// survey completion first, under the embedded survey prompt, then one extract
// completion per surface the inventory names — here one — before the inline
// arm's own; the surface and its entry cross the boundary into the extract
// turn. The inventory enters at the fixture's one module, so the check
// accepts it and the run keeps to the script.
#[tokio::test]
async fn typescript() {
    let project = scratch();
    project.write("src/index.ts", "export function greet(): string { return 'hello'; }\n");
    let inventory = r#"{"surfaces":[{"name":"greet export","entry":"src/index.ts"}]}"#;
    let answer = answer();

    let model = support::run(
        test_programs::ADAPTER_TYPESCRIPT,
        &project,
        &[],
        ScriptedModel::answering([inventory, answer.as_str(), answer.as_str()]),
    )
    .await;

    let seen = model.seen();
    assert_eq!(seen.len(), 3, "one survey, one extract per surface, one for the inline value");
    let survey = include_str!("../sources/typescript/prose/prompts/survey.md");
    let extract = include_str!("../sources/typescript/prose/prompts/extract.md");
    assert_eq!(
        seen[0].system.as_deref(),
        Some(survey),
        "the compiled-in survey prompt is the system"
    );
    for request in &seen[1..] {
        assert_eq!(
            request.system.as_deref(),
            Some(extract),
            "the compiled-in prompt is the system"
        );
    }
    let turn = &seen[1].messages[0];
    assert!(
        turn.contains("- surface: greet export"),
        "the surface reaches its extract turn: {turn}"
    );
    assert!(turn.contains("- entry: `src/index.ts`"), "with its entry: {turn}");
}
