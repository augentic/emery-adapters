//! Composed-deployment tests for the contracts adapter guest: the
//! `guidance` seam through host-mediated dispatch, and the MCP reference
//! references over `wasi:http` — including the build-time-resolved
//! `references/spec-runtime` symlink content.
//!
//! Model-free by design: the judgment legs (`build` / `merge`) are covered
//! natively in the `contracts` crate against `MockModel`, and live
//! against the cursor backend by the eval harness.

use adapter_tests::{self as common, Bundle};
use anyhow::{Context as _, Result};
use omnia::wasmtime::component::Val;
use omnia::{Dispatcher as _, Runtime};
use omnia_testkit::http;
use serde_json::{Value, json};

/// The versioned interface name the target-adapter world exports.
const TARGET_INTERFACE: &str = "specify:adapter/target@0.1.0";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guidance_through_dispatch() -> Result<()> {
    let mount = tempfile::tempdir()?;
    let runtime = common::runtime(mount.path()).await?;

    let results = runtime
        .invoke(
            "target:contracts".into(),
            Some(TARGET_INTERFACE.to_string()),
            "guidance".to_string(),
            vec![Val::String("target:contracts".to_string())],
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
        prompt.starts_with("# contracts.guidance"),
        "guidance returns the embedded guidance prompt: {}",
        &prompt[..prompt.len().min(80)]
    );
    Ok(())
}

// The async-lifted `build` export awaits `omnia:model/completion.create`;
// the stub backend pends then fails, so the leg must come back as the WIT
// error variant — not a trap — proving a pending host future survives
// host-mediated dispatch.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn build_bridge_survives_dispatch() -> Result<()> {
    let mount = tempfile::tempdir()?;
    let runtime = common::runtime(mount.path()).await?;

    let results = runtime
        .invoke(
            "target:contracts".into(),
            Some(TARGET_INTERFACE.to_string()),
            "build".to_string(),
            vec![
                Val::String("target:contracts".to_string()),
                Val::String("bridge-probe".to_string()),
                Val::List(Vec::new()),
                Val::Record(vec![
                    ("base".to_string(), Val::String("eval".to_string())),
                    ("subpath".to_string(), Val::Option(None)),
                ]),
            ],
        )
        .await
        .context("dispatching build")?;

    let [Val::Result(Err(Some(payload)))] = results.as_slice() else {
        anyhow::bail!("build against the stub backend returned an unexpected shape: {results:?}");
    };
    let Val::Variant(case, Some(detail)) = payload.as_ref() else {
        anyhow::bail!("build error payload is not a variant: {payload:?}");
    };
    let Val::String(detail) = detail.as_ref() else {
        anyhow::bail!("build error detail is not a string: {payload:?}");
    };
    assert_eq!(case, "internal", "stub-backend failure maps to the internal error case");
    assert!(
        detail.contains("completion must not be called"),
        "error carries the stub backend's message: {detail}"
    );
    Ok(())
}

async fn post(runtime: &Runtime<Bundle>, message: &Value) -> Result<Value> {
    let response = http::post_json(runtime, "/mcp/contracts", message.to_string()).await?;
    assert!(response.status().is_success(), "MCP POST replies 2xx: {}", response.status());
    serde_json::from_slice(response.body()).context("MCP reply is JSON")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn references() -> Result<()> {
    let mount = tempfile::tempdir()?;
    let runtime = common::runtime(mount.path()).await?;

    let init = post(
        &runtime,
        &json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "protocolVersion": "2025-06-18" }
        }),
    )
    .await?;
    assert_eq!(init["result"]["serverInfo"]["name"], "contracts-references");

    let resources =
        post(&runtime, &json!({ "jsonrpc": "2.0", "id": 2, "method": "resources/list" })).await?;
    let uris: Vec<&str> = resources["result"]["resources"]
        .as_array()
        .context("resources is an array")?
        .iter()
        .filter_map(|resource| resource["uri"].as_str())
        .collect();
    assert!(
        uris.contains(&"doc://prompts/build.md"),
        "references lists the build prompt: {uris:?}"
    );
    assert!(
        uris.contains(&"doc://references/spec-runtime/phase-outcome-contract.md"),
        "references lists the resolved spec-runtime symlink content: {uris:?}"
    );

    let prompt = post(
        &runtime,
        &json!({
            "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": { "name": "read_doc", "arguments": { "path": "prompts/build.md" } }
        }),
    )
    .await?;
    let text = prompt["result"]["content"][0]["text"].as_str().unwrap_or_default();
    assert!(text.starts_with("# contracts.build"), "read_doc returns the prompt body: {prompt}");

    let runtime_doc = post(
        &runtime,
        &json!({
            "jsonrpc": "2.0", "id": 4, "method": "resources/read",
            "params": { "uri": "doc://references/spec-runtime/phase-outcome-contract.md" }
        }),
    )
    .await?;
    let text = runtime_doc["result"]["contents"][0]["text"].as_str().unwrap_or_default();
    assert!(!text.is_empty(), "symlinked runtime reference body is embedded: {runtime_doc}");

    Ok(())
}
