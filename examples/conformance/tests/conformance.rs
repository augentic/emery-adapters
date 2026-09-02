//! Component conformance: every `sources/*` adapter component instantiated
//! under the omnia runtime with a scripted model, driven by the caller
//! guest over the `emery:adapter/source` seam. This rung owns
//! instantiation, an effect-free `metadata`, the reference-tool
//! round-trip across real wasi-model streams, the wire lowering of
//! evidence and of the typed `error` — never prompt text or extraction
//! quality, which the native suites and the live eval own.

#![cfg(not(target_arch = "wasm32"))]

use conformance::{
    Backends, Call, SOURCE_DOCUMENTATION, SOURCE_INTENT, SOURCE_TYPESCRIPT, ScriptedModel, scratch,
};
use omnia::ExitStatus;
use serde_json::{Value, json};

// Every adapter crate under `sources/` must have a matching test here; a
// new adapter without one fails to compile.
conformance::foreach_source!();

/// One adapter's conformance fixture.
struct Case {
    /// The routed adapter id the caller dispatches to.
    id: &'static str,
    /// The built component.
    wasm: &'static str,
    /// The authority the scripted evidence declares.
    authority: &'static str,
    /// The source tree staged in the project scratch.
    files: &'static [(&'static str, &'static str)],
    /// The adapter's embedded extract prompt, as compiled into it.
    prompt: &'static str,
}

const DOCUMENTATION: Case = Case {
    id: "source:documentation",
    wasm: SOURCE_DOCUMENTATION,
    authority: "documentation",
    files: &[("docs/orders.md", "# Orders\n\nPOST /orders creates an order.\n")],
    prompt: include_str!("../../../sources/documentation/prose/prompts/extract.md"),
};

const INTENT: Case = Case {
    id: "source:intent",
    wasm: SOURCE_INTENT,
    authority: "intent",
    files: &[("brief.md", "Let users reset passwords by email.\n")],
    prompt: include_str!("../../../sources/intent/prose/prompts/extract.md"),
};

const TYPESCRIPT: Case = Case {
    id: "source:typescript",
    wasm: SOURCE_TYPESCRIPT,
    authority: "behaviour",
    files: &[("src/index.ts", "export function greet(): string { return 'hello'; }\n")],
    prompt: include_str!("../../../sources/typescript/prose/prompts/extract.md"),
};

/// A minimal evidence answer every adapter's schema gate accepts.
fn evidence(authority: &str) -> Value {
    json!({
        "authority": authority,
        "claims": [{
            "kind": "requirement",
            "id": "orders.create",
            "statement": "POST /orders creates an order."
        }]
    })
}

// The shared happy path: the component instantiates under the runtime;
// `metadata` answers without touching the model; `extract` opens exactly
// one completion whose system prompt is the embedded `prompts/extract.md`,
// whose declared tools are the reference tools, and whose `read_doc` call
// comes back across the tool streams with that same embedded body; the
// evidence lowers back to the caller with its required extras intact.
async fn conforms(case: Case) {
    // --------------------------------------------------
    // Arrange.
    // --------------------------------------------------
    let project = scratch();
    for (path, body) in case.files {
        project.write(path, body);
    }
    let model = ScriptedModel::answering([evidence(case.authority)])
        .calling(0, [("read_doc", r#"{"path":"prompts/extract.md"}"#)]);
    let backends = Backends::scripted(model);

    // --------------------------------------------------
    // Act.
    // --------------------------------------------------
    let status = conformance::run(
        Call {
            id: case.id,
            wasm: case.wasm,
            argv: &["source", "workspace"],
            project: &project,
        },
        backends.clone(),
    )
    .await
    .expect("deployment runs");

    // --------------------------------------------------
    // Observe.
    // --------------------------------------------------
    assert_eq!(status, ExitStatus::SUCCESS, "the caller's assertions held");
    backends.model.assert_exhausted();
    let requests = backends.model.requests();
    assert_eq!(requests.len(), 1, "metadata is effect-free; extract is one judgment");
    let request = &requests[0];
    assert_eq!(
        request.system.as_deref(),
        Some(case.prompt),
        "the compiled-in extract prompt is the system prompt"
    );
    assert_eq!(request.tools, ["list_docs", "read_doc"], "the reference tools are declared");
    assert!(request.messages[0].contains("source key `source`"), "{:?}", request.messages);

    let exchanges = backends.model.exchanges();
    assert_eq!(exchanges.len(), 1, "one driven tool call");
    let answer: Value =
        serde_json::from_str(exchanges[0].outcome.as_ref().expect("read_doc answered"))
            .expect("a JSON answer");
    assert_eq!(answer["path"], "prompts/extract.md");
    assert_eq!(answer["body"], case.prompt, "the embedded document body crosses the seam");
}

#[tokio::test]
async fn documentation() {
    conforms(DOCUMENTATION).await;
}

#[tokio::test]
async fn intent() {
    conforms(INTENT).await;
}

#[tokio::test]
async fn typescript() {
    conforms(TYPESCRIPT).await;
}

// A fail-closed refusal crosses the seam as the typed WIT `error`
// variant, before any model call: the intent adapter reads a one-file
// tree, so a two-file tree is `invalid-request` on the wire.
#[tokio::test]
async fn typed_error() {
    let project = scratch();
    project.write("a.md", "one\n");
    project.write("b.md", "two\n");
    let backends = Backends::scripted(ScriptedModel::answering([]));

    let status = conformance::run(
        Call {
            id: INTENT.id,
            wasm: INTENT.wasm,
            argv: &["source", "workspace", "expect-error:invalid-request"],
            project: &project,
        },
        backends.clone(),
    )
    .await
    .expect("deployment runs");

    assert_eq!(status, ExitStatus::SUCCESS, "the caller saw `invalid-request`");
    assert!(backends.model.requests().is_empty(), "no model call precedes the refusal");
}
