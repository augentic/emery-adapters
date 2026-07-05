//! Composed-deployment tests for the vectis adapter guest: the `guidance`
//! seam through host-mediated dispatch, and the ~600 KB embedded
//! reference shelf served over `wasi:http` on the guest's own
//! `/mcp/vectis` route — including the nested per-platform build
//! sub-briefs and the build-time-resolved `references/agent-teams.md`
//! symlink content.
//!
//! Model-free by design, like the contracts and omnia tests: the
//! judgment legs (`build` / `merge`) and the absorbed validate /
//! materialize / prepare libraries are covered natively in
//! `specify-vectis-core` against `MockModel`.

use anyhow::{Context as _, Result};
use omnia::wasmtime::component::Val;
use omnia::{Dispatcher as _, Runtime};
use omnia_testkit::http;
use serde_json::{Value, json};

use crate::common::{self, Bundle};

/// The versioned interface name the target-adapter world exports.
const TARGET_INTERFACE: &str = "augentic:specify/target@0.1.0";

// guidance("target:vectis") through host-mediated dispatch in the composed
// deployment returns the embedded guidance brief — the core registry riding
// inside the component, beside the other guests.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guidance_through_dispatch() -> Result<()> {
    let mount = tempfile::tempdir()?;
    let runtime = common::composed_runtime(mount.path()).await?;

    let results = runtime
        .invoke(
            "target:vectis".into(),
            Some(TARGET_INTERFACE.to_string()),
            "guidance".to_string(),
            vec![Val::String("target:vectis".to_string())],
        )
        .await
        .context("dispatching guidance")?;

    let [Val::Result(Ok(Some(payload)))] = results.as_slice() else {
        anyhow::bail!("guidance returned an unexpected shape: {results:?}");
    };
    let Val::String(brief) = payload.as_ref() else {
        anyhow::bail!("guidance payload is not a string: {payload:?}");
    };
    assert!(
        brief.starts_with("# Vectis target — `guidance`"),
        "guidance returns the embedded guidance brief: {}",
        &brief[..brief.len().min(80)]
    );
    Ok(())
}

// POST one JSON-RPC message to /mcp/vectis and parse the reply.
async fn post(runtime: &Runtime<Bundle>, message: &Value) -> Result<Value> {
    let response = http::post_json(runtime, "/mcp/vectis", message.to_string()).await?;
    assert!(response.status().is_success(), "MCP POST replies 2xx: {}", response.status());
    serde_json::from_slice(response.body()).context("MCP reply is JSON")
}

// The route serves the embedded prose registry as an MCP shelf: initialize
// identifies the server, read_doc returns a reference body, and a nested
// per-platform build sub-brief is served under its full path.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shelf() -> Result<()> {
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
    assert_eq!(init["result"]["serverInfo"]["name"], "specify-vectis-references");

    let reference = post(
        &runtime,
        &json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": { "name": "read_doc", "arguments": { "path": "references/hard-rules-core.md" } }
        }),
    )
    .await?;
    let text = reference["result"]["content"][0]["text"].as_str().unwrap_or_default();
    assert!(!text.is_empty(), "read_doc returns the reference body: {reference}");

    let sub_brief = post(
        &runtime,
        &json!({
            "jsonrpc": "2.0", "id": 3, "method": "resources/read",
            "params": { "uri": "doc://briefs/build/ios/write.md" }
        }),
    )
    .await?;
    let text = sub_brief["result"]["contents"][0]["text"].as_str().unwrap_or_default();
    assert!(
        text.starts_with("# Vectis build — iOS shell"),
        "nested build sub-brief body is embedded: {sub_brief}"
    );

    Ok(())
}
