//! Composed-deployment tests for the vectis adapter guest: the `guidance`
//! seam through host-mediated dispatch, and the ~600 KB embedded
//! references served over `wasi:http` on the guest's own
//! `/mcp/vectis` route — including the nested per-platform build
//! per-platform prompts and the build-time-resolved `references/agent-teams.md`
//! symlink content.
//!
//! Model-free by design, like the contracts and omnia tests: the
//! judgment legs (`build` / `merge`) and the absorbed validate /
//! materialize / prepare libraries are covered natively in
//! the `vectis` crate against `MockModel`.

use adapter_tests::{self as common, Bundle};
use anyhow::{Context as _, Result};
use omnia::wasmtime::component::Val;
use omnia::{Dispatcher as _, Runtime};
use omnia_testkit::http;
use serde_json::{Value, json};

/// The versioned interface name the target-adapter world exports.
const TARGET_INTERFACE: &str = "specify:adapter/target@0.1.0";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn describe() -> Result<()> {
    let mount = tempfile::tempdir()?;
    let runtime = common::composed_runtime(mount.path()).await?;

    let results = runtime
        .invoke(
            "target:vectis".into(),
            Some(TARGET_INTERFACE.to_string()),
            "describe".to_string(),
            vec![Val::String("target:vectis".to_string())],
        )
        .await
        .context("dispatching describe")?;

    let [Val::Record(fields)] = results.as_slice() else {
        anyhow::bail!("describe returned an unexpected shape: {results:?}");
    };
    let field = |name: &str| {
        fields
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value)
            .with_context(|| format!("manifest record carries `{name}`: {fields:?}"))
    };

    let Val::Option(None) = field("specify-floor")? else {
        anyhow::bail!("vectis declares no compatibility floor: {fields:?}");
    };

    let Val::List(inputs) = field("inputs")? else {
        anyhow::bail!("manifest `inputs` is a list: {fields:?}");
    };
    let paths: Vec<&str> = inputs
        .iter()
        .filter_map(|input| {
            let Val::Record(entries) = input else { return None };
            entries.iter().find_map(|(key, value)| {
                if let ("path", Val::String(path)) = (key.as_str(), value) {
                    Some(path.as_str())
                } else {
                    None
                }
            })
        })
        .collect();
    assert_eq!(
        paths,
        ["tokens.yaml", "assets.yaml", "components.yaml"],
        "vectis declares the three optional design-system inputs"
    );

    let Val::Option(Some(platforms)) = field("platforms")? else {
        anyhow::bail!("vectis declares a platforms capability: {fields:?}");
    };
    let Val::Record(capability) = platforms.as_ref() else {
        anyhow::bail!("platforms capability is a record: {platforms:?}");
    };
    assert!(
        capability.iter().any(|(key, value)| key == "required" && *value == Val::Bool(true)),
        "vectis requires a declared platform set: {capability:?}"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guidance() -> Result<()> {
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
    let Val::String(prompt) = payload.as_ref() else {
        anyhow::bail!("guidance payload is not a string: {payload:?}");
    };
    assert!(
        prompt.starts_with("# Vectis target — `guidance`"),
        "guidance returns the embedded guidance prompt: {}",
        &prompt[..prompt.len().min(80)]
    );
    Ok(())
}

async fn post(runtime: &Runtime<Bundle>, message: &Value) -> Result<Value> {
    let response = http::post_json(runtime, "/mcp/vectis", message.to_string()).await?;
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
    assert_eq!(init["result"]["serverInfo"]["name"], "vectis-references");

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

    let leg_prompt = post(
        &runtime,
        &json!({
            "jsonrpc": "2.0", "id": 3, "method": "resources/read",
            "params": { "uri": "doc://prompts/build/ios/write.md" }
        }),
    )
    .await?;
    let text = leg_prompt["result"]["contents"][0]["text"].as_str().unwrap_or_default();
    assert!(
        text.starts_with("# Vectis build — iOS shell"),
        "nested build prompt body is embedded: {leg_prompt}"
    );

    Ok(())
}
