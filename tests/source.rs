//! Every shipped component over `emery:adapter/source`: each `sources/*`
//! adapter runs through the omnia runtime, driven by the `source_extract`
//! program from `crates/test-programs` over the same seam the engine uses,
//! against a scripted host-side model. The driver asserts what crosses the
//! boundary (and traps on failure); this side asserts what the host alone
//! sees of *this* component: its `metadata` opened no completion, and the
//! system prompt of each `extract` is the `prompts/extract.md` this build
//! embedded. What every component shares over the seam — the SDK's request
//! shape, the reference tools, the lend, the claim gate's refusal — is
//! proved once over the `gated` probe in `tests/probe.rs`; an adapter's own
//! behaviour is its `tests/extract.rs`'s.
//!
//! `foreach_adapter!` is the orphan guard for the adapter set: a new
//! `sources/<name>` component fails to compile here until a test of that
//! name exists.

#![cfg(not(target_arch = "wasm32"))]

use omnia::ExitStatus;
use omnia_test::host::{Backends, Deployment, Scratch, ScriptedModel, scratch};
use omnia_wasi_model::WasiModel;

// Every `sources/*` component `crates/test-programs` builds must have a
// matching test here; a new adapter without one fails to compile.
test_programs::foreach_adapter!();

/// An answer the claim gate accepts; the gate reads no adapter's authority.
const EVIDENCE: &str = r#"{"authority":"documentation","claims":[
    {"kind":"requirement","id":"orders.create","path":"docs/orders.md#L3","statement":"POST /orders creates an order."}
]}"#;

/// Runs the driver's answered legs against `component` with `project`
/// mounted read-only as `.`, one scripted answer per `extract`; requires a
/// clean exit and the script exactly consumed, and returns the model's
/// record.
async fn extract(component: &str, project: &Scratch) -> ScriptedModel {
    let model = ScriptedModel::answering([EVIDENCE, EVIDENCE]);
    let backends = Backends::defaults().await.model(model.clone());
    let status = Deployment::new()
        .link(["emery:adapter/source@0.1.0"])
        .guest("caller", test_programs::SOURCE_EXTRACT)
        .guest(test_programs::ADAPTER, component)
        .command("caller")
        .mount(project.mount(false))
        .run_host::<WasiModel, _>(backends)
        .await
        .expect("the caller runs");
    assert_eq!(status, ExitStatus::SUCCESS, "the driver's checks failed");
    model.assert_exhausted();
    model
}

/// What the host sees of every component: `metadata` opened no completion,
/// each `extract` opened one, and its system prompt is `prompt`.
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

// The one adapter that reads its source inside the guest: the brief reaches
// the turn through the mounted tree, not the lend.
#[tokio::test]
async fn intent() {
    const BRIEF: &str = "Let users reset passwords by email.";
    let project = scratch();
    project.write("brief.md", BRIEF);

    let model = extract(test_programs::ADAPTER_INTENT, &project).await;

    prompted(&model, include_str!("../sources/intent/prose/prompts/extract.md"));
    let turn = &model.seen()[0].messages[0];
    assert!(turn.contains(BRIEF), "the brief read through the mount is the material: {turn}");
}

#[tokio::test]
async fn typescript() {
    let project = scratch();
    project.write("src/index.ts", "export function greet(): string { return 'hello'; }\n");

    let model = extract(test_programs::ADAPTER_TYPESCRIPT, &project).await;

    prompted(&model, include_str!("../sources/typescript/prose/prompts/extract.md"));
}
