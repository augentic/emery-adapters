//! Composed-deployment tests for the omnia adapter guest: the `guidance`
//! seam through host-mediated dispatch, and the ~700 KB embedded
//! references served over `wasi:http` on the guest's own
//! `/mcp/omnia` route — including the build-time-resolved
//! `references/spec-runtime` symlink content.
//!
//! Model-free by design, like the contracts tests: the judgment legs
//! (`build` / `merge`) are covered natively in the `omnia` crate
//! against `MockModel`.

use anyhow::{Context as _, Result};
use omnia::wasmtime::component::Val;
use omnia::{Dispatcher as _, Runtime};
use omnia_testkit::http;
use serde_json::{Value, json};

use crate::common::{self, Bundle};

/// The versioned interface name the target-adapter world exports.
const TARGET_INTERFACE: &str = "specify:adapter/target@0.1.0";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guidance_through_dispatch() -> Result<()> {
    let mount = tempfile::tempdir()?;
    let runtime = common::composed_runtime(mount.path()).await?;

    let results = runtime
        .invoke(
            "target:omnia".into(),
            Some(TARGET_INTERFACE.to_string()),
            "guidance".to_string(),
            vec![Val::String("target:omnia".to_string())],
        )
        .await
        .context("dispatching guidance")?;

    let [Val::Result(Ok(Some(payload)))] = results.as_slice() else {
        anyhow::bail!("guidance returned an unexpected shape: {results:?}");
    };
    let Val::String(prompt) = payload.as_ref() else {
        anyhow::bail!("guidance payload is not a string: {payload:?}");
    };
    assert!(
        prompt.starts_with("# Omnia target — guidance prompt"),
        "guidance returns the embedded guidance prompt: {}",
        &prompt[..prompt.len().min(80)]
    );
    Ok(())
}

async fn post(runtime: &Runtime<Bundle>, message: &Value) -> Result<Value> {
    let response = http::post_json(runtime, "/mcp/omnia", message.to_string()).await?;
    assert!(response.status().is_success(), "MCP POST replies 2xx: {}", response.status());
    serde_json::from_slice(response.body()).context("MCP reply is JSON")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn references() -> Result<()> {
    let mount = tempfile::tempdir()?;
    let runtime = common::composed_runtime(mount.path()).await?;

    let init = post(
        &runtime,
        &json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "protocolVersion": "2025-06-18" }
        }),
    )
    .await?;
    assert_eq!(init["result"]["serverInfo"]["name"], "omnia-references");

    let reference = post(
        &runtime,
        &json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": { "name": "read_doc", "arguments": { "path": "references/guardrails.md" } }
        }),
    )
    .await?;
    let text = reference["result"]["content"][0]["text"].as_str().unwrap_or_default();
    assert!(text.starts_with("# Guardrails"), "read_doc returns the reference body: {reference}");

    let runtime_doc = post(
        &runtime,
        &json!({
            "jsonrpc": "2.0", "id": 3, "method": "resources/read",
            "params": { "uri": "doc://references/spec-runtime/phase-outcome-contract.md" }
        }),
    )
    .await?;
    let text = runtime_doc["result"]["contents"][0]["text"].as_str().unwrap_or_default();
    assert!(!text.is_empty(), "symlinked runtime reference body is embedded: {runtime_doc}");

    Ok(())
}
