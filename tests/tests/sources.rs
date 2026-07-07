//! Composed multi-guest deployment tests: the contracts target guest and
//! the documentation source guest side by side, proving the source axis
//! rides the same seams — `survey` through host-mediated dispatch, and
//! the source guest's own MCP reference shelf on its own HTTP route.
//!
//! Model-free by design, like the contracts tests: the judgment legs are
//! covered natively in each `<name>-core` against `MockModel`.

use anyhow::{Context as _, Result};
use omnia::wasmtime::component::Val;
use omnia::{Dispatcher as _, Runtime};
use omnia_testkit::http;
use serde_json::{Value, json};

use crate::common::{self, Bundle};

/// The versioned interface name the source-adapter world exports.
const SOURCE_INTERFACE: &str = "specify:adapter/source@0.1.0";

// describe("source:documentation") through host-mediated dispatch returns
// the compiled-in manifest record — on the source axis just the
// compatibility floor, absent here.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn describe_through_dispatch() -> Result<()> {
    let mount = tempfile::tempdir()?;
    let runtime = common::composed_runtime(mount.path()).await?;

    let results = runtime
        .invoke(
            "source:documentation".into(),
            Some(SOURCE_INTERFACE.to_string()),
            "describe".to_string(),
            vec![Val::String("source:documentation".to_string())],
        )
        .await
        .context("dispatching describe")?;

    let [Val::Record(fields)] = results.as_slice() else {
        anyhow::bail!("describe returned an unexpected shape: {results:?}");
    };
    assert!(
        fields.iter().any(|(key, value)| key == "specify-floor" && *value == Val::Option(None)),
        "documentation declares no compatibility floor: {fields:?}"
    );
    Ok(())
}

// survey through dispatch exercises the source guest's async-lifted
// judgment leg (the `async func` export awaiting
// `omnia:model/completion.create`): the stub backend pends and then fails
// every completion, so the leg must come back as the WIT error variant —
// not a trap — proving the source axis survives host-mediated dispatch
// in a multi-guest deployment.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn survey_bridge_survives_dispatch() -> Result<()> {
    let mount = tempfile::tempdir()?;
    let runtime = common::composed_runtime(mount.path()).await?;

    let results = runtime
        .invoke(
            "source:documentation".into(),
            Some(SOURCE_INTERFACE.to_string()),
            "survey".to_string(),
            vec![Val::String("source:documentation".to_string())],
        )
        .await
        .context("dispatching survey")?;

    let [Val::Result(Err(Some(payload)))] = results.as_slice() else {
        anyhow::bail!("survey against the stub backend returned an unexpected shape: {results:?}");
    };
    let Val::Variant(case, Some(detail)) = payload.as_ref() else {
        anyhow::bail!("survey error payload is not a variant: {payload:?}");
    };
    let Val::String(detail) = detail.as_ref() else {
        anyhow::bail!("survey error detail is not a string: {payload:?}");
    };
    assert_eq!(case, "internal", "stub-backend failure maps to the internal error case");
    assert!(
        detail.contains("completion must not be called"),
        "error carries the stub backend's message: {detail}"
    );
    Ok(())
}

// POST one JSON-RPC message to a route and parse the reply.
async fn post(runtime: &Runtime<Bundle>, route: &str, message: &Value) -> Result<Value> {
    let response = http::post_json(runtime, route, message.to_string()).await?;
    assert!(response.status().is_success(), "MCP POST replies 2xx: {}", response.status());
    serde_json::from_slice(response.body()).context("MCP reply is JSON")
}

// Each guest in the composed deployment serves its own embedded prose
// registry on its own route: the documentation shelf identifies itself
// and serves the survey prompt, while the contracts shelf next door keeps
// serving the contracts registry.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn per_guest_shelves() -> Result<()> {
    let mount = tempfile::tempdir()?;
    let runtime = common::composed_runtime(mount.path()).await?;

    let init = post(
        &runtime,
        "/mcp/documentation",
        &json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "protocolVersion": "2025-06-18" }
        }),
    )
    .await?;
    assert_eq!(init["result"]["serverInfo"]["name"], "documentation-references");

    let prompt = post(
        &runtime,
        "/mcp/documentation",
        &json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": { "name": "read_doc", "arguments": { "path": "prompts/survey.md" } }
        }),
    )
    .await?;
    let text = prompt["result"]["content"][0]["text"].as_str().unwrap_or_default();
    assert!(
        text.starts_with("# `documentation.survey`"),
        "read_doc returns the survey prompt body: {prompt}"
    );

    let contracts_init = post(
        &runtime,
        "/mcp/contracts",
        &json!({
            "jsonrpc": "2.0", "id": 3, "method": "initialize",
            "params": { "protocolVersion": "2025-06-18" }
        }),
    )
    .await?;
    assert_eq!(
        contracts_init["result"]["serverInfo"]["name"], "contracts-references",
        "the contracts shelf keeps its own identity beside the source guest"
    );

    Ok(())
}
