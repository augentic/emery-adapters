//! Composed multi-guest deployment tests for the remaining four source
//! guests — intent, typescript, screenshots, and captures — side by side
//! in one deployment, proving each rides the same seams as documentation:
//! `survey` through host-mediated dispatch, and each guest's own MCP
//! reference shelf on its own HTTP route.
//!
//! Model-free by design, like the other composed tests: the judgment legs
//! are covered natively in each `specify-<name>-core` against `MockModel`.

use anyhow::{Context as _, Result};
use omnia::wasmtime::component::Val;
use omnia::{Dispatcher as _, Runtime};
use omnia_testkit::http;
use serde_json::{Value, json};

use crate::common::{self, Bundle};

/// The versioned interface name the source-adapter world exports.
const SOURCE_INTERFACE: &str = "augentic:specify/source@0.1.0";

/// The four source guests this deployment composes: guest id, MCP route,
/// shelf server identity, and the survey prompt's opening heading.
const GUESTS: [(&str, &str, &str, &str); 4] = [
    ("source:intent", "/mcp/intent", "specify-intent-references", "# intent.survey"),
    (
        "source:typescript",
        "/mcp/typescript",
        "specify-typescript-references",
        "# TypeScript / JavaScript source survey",
    ),
    (
        "source:screenshots",
        "/mcp/screenshots",
        "specify-screenshots-references",
        "# `screenshots.survey`",
    ),
    ("source:captures", "/mcp/captures", "specify-captures-references", "# Runtime capture survey"),
];

// survey through dispatch exercises each source guest's async-lifted
// judgment leg (the `async func` export awaiting
// `omnia:model/completion.create`): the stub backend pends and then fails
// every completion, so each leg must come back as the WIT error variant —
// not a trap — proving all four remaining source guests survive
// host-mediated dispatch in one composed deployment.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn survey_bridges_survive_dispatch() -> Result<()> {
    let mount = tempfile::tempdir()?;
    let runtime = common::source_guests_runtime(mount.path()).await?;

    for (guest, _, _, _) in GUESTS {
        let results = runtime
            .invoke(
                guest.into(),
                Some(SOURCE_INTERFACE.to_string()),
                "survey".to_string(),
                vec![Val::String(guest.to_string())],
            )
            .await
            .with_context(|| format!("dispatching survey to {guest}"))?;

        let [Val::Result(Err(Some(payload)))] = results.as_slice() else {
            anyhow::bail!(
                "{guest} survey against the stub backend returned an unexpected shape: {results:?}"
            );
        };
        let Val::Variant(case, Some(detail)) = payload.as_ref() else {
            anyhow::bail!("{guest} survey error payload is not a variant: {payload:?}");
        };
        let Val::String(detail) = detail.as_ref() else {
            anyhow::bail!("{guest} survey error detail is not a string: {payload:?}");
        };
        assert_eq!(
            case, "internal",
            "{guest}: stub-backend failure maps to the internal error case"
        );
        assert!(
            detail.contains("completion must not be called"),
            "{guest}: error carries the stub backend's message: {detail}"
        );
    }
    Ok(())
}

// POST one JSON-RPC message to a route and parse the reply.
async fn post(runtime: &Runtime<Bundle>, route: &str, message: &Value) -> Result<Value> {
    let response = http::post_json(runtime, route, message.to_string()).await?;
    assert!(response.status().is_success(), "MCP POST replies 2xx: {}", response.status());
    serde_json::from_slice(response.body()).context("MCP reply is JSON")
}

// Each guest in the composed deployment serves its own embedded prose
// registry on its own route: every shelf identifies itself with its own
// server name and serves its own survey prompt.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn per_guest_shelves() -> Result<()> {
    let mount = tempfile::tempdir()?;
    let runtime = common::source_guests_runtime(mount.path()).await?;

    for (guest, route, server, heading) in GUESTS {
        let init = post(
            &runtime,
            route,
            &json!({
                "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": "2025-06-18" }
            }),
        )
        .await?;
        assert_eq!(
            init["result"]["serverInfo"]["name"], server,
            "{guest}: shelf identifies its own server"
        );

        let prompt = post(
            &runtime,
            route,
            &json!({
                "jsonrpc": "2.0", "id": 2, "method": "tools/call",
                "params": { "name": "read_doc", "arguments": { "path": "prompts/survey.md" } }
            }),
        )
        .await?;
        let text = prompt["result"]["content"][0]["text"].as_str().unwrap_or_default();
        assert!(
            text.starts_with(heading),
            "{guest}: read_doc returns its own survey prompt body: {prompt}"
        );
    }
    Ok(())
}
