//! Composed-deployment tests hosting the built adapter guest components
//! on the Omnia runtime — model-free by design.
//!
//! Each module deploys one manifest shape — the single-guest contracts
//! deployment, the multi-guest composed deployment (three targets plus
//! the documentation source), or the remaining source-guest set — and
//! exercises the deterministic seams: `metadata` / `guidance` through
//! host-mediated dispatch, the async-lifted judgment legs against the
//! stub model backend (which must come back as the WIT error variant,
//! not a trap), and each guest's MCP references over `wasi:http` on its
//! own route.
//!
//! The judgment legs themselves are covered natively in each adapter
//! crate against `MockModel`, and live against the cursor backend by
//! the `live` test target beside this one. The model backend here is a
//! stub that fails every completion: these tests are model-free, so any
//! completion is a test bug.

#![cfg(not(target_arch = "wasm32"))]

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use anyhow::{Context as _, Result};
use omnia::futures::FutureExt as _;
use omnia::wasmtime_wasi::ResourceTable;
use omnia::{Backend as _, Backends as _, DeploymentBuilder, HasHttp, Runtime, StoreCtx};
use omnia_testkit::{TempManifest, temp_manifest};
use omnia_wasi_http::{HttpDefault, WasiHttp, WasiHttpCtxView};
use omnia_wasi_model::{
    Answer, FutureResult, HasModel, Request, ToolHost, WasiModel, WasiModelCtx,
};

/// The versioned interface name the target-adapter world exports.
const TARGET_INTERFACE: &str = "specify:adapter/target@0.1.0";

/// The versioned interface name the source-adapter world exports.
const SOURCE_INTERFACE: &str = "specify:adapter/source@0.1.0";

/// Composed-deployment tests for the contracts adapter guest: the
/// `guidance` seam through host-mediated dispatch, and the MCP reference
/// references over `wasi:http` — including the build-time-resolved
/// `references/spec-runtime` symlink content.
mod contracts {
    use anyhow::{Context as _, Result};
    use omnia::Runtime;
    use omnia::wasmtime::component::Val;
    use omnia_testkit::http;
    use serde_json::{Value, json};

    use super::{Bundle, TARGET_INTERFACE};

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn guidance() -> Result<()> {
        let mount = tempfile::tempdir()?;
        let runtime = super::runtime(mount.path()).await?;

        let results = runtime
            .dispatcher()
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
    async fn build_bridge() -> Result<()> {
        let mount = tempfile::tempdir()?;
        let runtime = super::runtime(mount.path()).await?;

        let results = runtime
            .dispatcher()
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
            anyhow::bail!(
                "build against the stub backend returned an unexpected shape: {results:?}"
            );
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
        let runtime = super::runtime(mount.path()).await?;

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
            post(&runtime, &json!({ "jsonrpc": "2.0", "id": 2, "method": "resources/list" }))
                .await?;
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
        assert!(
            text.starts_with("# contracts.build"),
            "read_doc returns the prompt body: {prompt}"
        );

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
}

/// Composed-deployment tests for the omnia adapter guest: the `guidance`
/// seam through host-mediated dispatch, and the ~700 KB embedded
/// references served over `wasi:http` on the guest's own `/mcp/omnia`
/// route — including the build-time-resolved `references/spec-runtime`
/// symlink content.
mod omnia_guest {
    use anyhow::{Context as _, Result};
    use omnia::Runtime;
    use omnia::wasmtime::component::Val;
    use omnia_testkit::http;
    use serde_json::{Value, json};

    use super::{Bundle, TARGET_INTERFACE};

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn guidance() -> Result<()> {
        let mount = tempfile::tempdir()?;
        let runtime = super::composed_runtime(mount.path()).await?;

        let results = runtime
            .dispatcher()
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
        let runtime = super::composed_runtime(mount.path()).await?;

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
        assert!(
            text.starts_with("# Guardrails"),
            "read_doc returns the reference body: {reference}"
        );

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
}

/// Composed-deployment tests for the vectis adapter guest: the `metadata`
/// and `guidance` seams through host-mediated dispatch, and the ~600 KB
/// embedded references served over `wasi:http` on the guest's own
/// `/mcp/vectis` route — including the nested per-platform build prompts
/// and the build-time-resolved `references/agent-teams.md` symlink content.
mod vectis {
    use anyhow::{Context as _, Result};
    use omnia::Runtime;
    use omnia::wasmtime::component::Val;
    use omnia_testkit::http;
    use serde_json::{Value, json};

    use super::{Bundle, TARGET_INTERFACE};

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn metadata() -> Result<()> {
        let mount = tempfile::tempdir()?;
        let runtime = super::composed_runtime(mount.path()).await?;

        let results = runtime
            .dispatcher()
            .invoke(
                "target:vectis".into(),
                Some(TARGET_INTERFACE.to_string()),
                "metadata".to_string(),
                vec![Val::String("target:vectis".to_string())],
            )
            .await
            .context("dispatching metadata")?;

        let [Val::Record(fields)] = results.as_slice() else {
            anyhow::bail!("metadata returned an unexpected shape: {results:?}");
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
        let runtime = super::composed_runtime(mount.path()).await?;

        let results = runtime
            .dispatcher()
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
        let runtime = super::composed_runtime(mount.path()).await?;

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
}

/// Composed multi-guest deployment tests: the contracts target guest and
/// the documentation source guest side by side, proving the source axis
/// rides the same seams — `survey` through host-mediated dispatch, and
/// the source guest's own MCP references on its own HTTP route.
mod documentation {
    use anyhow::{Context as _, Result};
    use omnia::Runtime;
    use omnia::wasmtime::component::Val;
    use omnia_testkit::http;
    use serde_json::{Value, json};

    use super::{Bundle, SOURCE_INTERFACE};

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn metadata() -> Result<()> {
        let mount = tempfile::tempdir()?;
        let runtime = super::composed_runtime(mount.path()).await?;

        let results = runtime
            .dispatcher()
            .invoke(
                "source:documentation".into(),
                Some(SOURCE_INTERFACE.to_string()),
                "metadata".to_string(),
                vec![Val::String("source:documentation".to_string())],
            )
            .await
            .context("dispatching metadata")?;

        let [Val::Record(fields)] = results.as_slice() else {
            anyhow::bail!("metadata returned an unexpected shape: {results:?}");
        };
        assert!(
            fields.iter().any(|(key, value)| key == "specify-floor" && *value == Val::Option(None)),
            "documentation declares no compatibility floor: {fields:?}"
        );
        Ok(())
    }

    // The async-lifted `survey` export awaits `omnia:model/completion.create`;
    // the stub backend pends then fails, so the leg must come back as the WIT
    // error variant — not a trap — proving the source axis survives
    // host-mediated dispatch in a multi-guest deployment.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn survey_bridge() -> Result<()> {
        let mount = tempfile::tempdir()?;
        let runtime = super::composed_runtime(mount.path()).await?;

        let results = runtime
            .dispatcher()
            .invoke(
                "source:documentation".into(),
                Some(SOURCE_INTERFACE.to_string()),
                "survey".to_string(),
                vec![Val::String("source:documentation".to_string())],
            )
            .await
            .context("dispatching survey")?;

        let [Val::Result(Err(Some(payload)))] = results.as_slice() else {
            anyhow::bail!(
                "survey against the stub backend returned an unexpected shape: {results:?}"
            );
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

    async fn post(runtime: &Runtime<Bundle>, route: &str, message: &Value) -> Result<Value> {
        let response = http::post_json(runtime, route, message.to_string()).await?;
        assert!(response.status().is_success(), "MCP POST replies 2xx: {}", response.status());
        serde_json::from_slice(response.body()).context("MCP reply is JSON")
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn per_guest_shelves() -> Result<()> {
        let mount = tempfile::tempdir()?;
        let runtime = super::composed_runtime(mount.path()).await?;

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
            "the contracts references keeps its own identity beside the source guest"
        );

        Ok(())
    }
}

/// Composed multi-guest deployment tests for the remaining four source
/// guests — intent, typescript, screenshots, and captures — side by side
/// in one deployment, proving each rides the same seams as documentation:
/// `survey` through host-mediated dispatch, and each guest's own MCP
/// references on its own HTTP route.
mod sources {
    use anyhow::{Context as _, Result};
    use omnia::Runtime;
    use omnia::wasmtime::component::Val;
    use omnia_testkit::http;
    use serde_json::{Value, json};

    use super::{Bundle, SOURCE_INTERFACE};

    /// The four source guests this deployment composes: guest id, MCP route,
    /// references server identity, and the survey prompt's opening heading.
    const GUESTS: [(&str, &str, &str, &str); 4] = [
        ("source:intent", "/mcp/intent", "intent-references", "# intent.survey"),
        (
            "source:typescript",
            "/mcp/typescript",
            "typescript-references",
            "# TypeScript / JavaScript source survey",
        ),
        (
            "source:screenshots",
            "/mcp/screenshots",
            "screenshots-references",
            "# `screenshots.survey`",
        ),
        ("source:captures", "/mcp/captures", "captures-references", "# Runtime capture survey"),
    ];

    // Each async-lifted `survey` export awaits `omnia:model/completion.create`;
    // the stub backend pends then fails, so each leg must come back as the WIT
    // error variant — not a trap — for all four guests in one deployment.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn survey_bridges() -> Result<()> {
        let mount = tempfile::tempdir()?;
        let runtime = super::source_guests_runtime(mount.path()).await?;

        for (guest, _, _, _) in GUESTS {
            let results = runtime
                .dispatcher()
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
                    "{guest} survey against the stub backend returned an unexpected shape: \
                     {results:?}"
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

    async fn post(runtime: &Runtime<Bundle>, route: &str, message: &Value) -> Result<Value> {
        let response = http::post_json(runtime, route, message.to_string()).await?;
        assert!(response.status().is_success(), "MCP POST replies 2xx: {}", response.status());
        serde_json::from_slice(response.body()).context("MCP reply is JSON")
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn per_guest_shelves() -> Result<()> {
        let mount = tempfile::tempdir()?;
        let runtime = super::source_guests_runtime(mount.path()).await?;

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
                "{guest}: references server identifies its own server"
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
}

/// One deployed guest: its manifest id and built artifact name.
type Guest = (&'static str, &'static str);

/// The single-guest contracts deployment.
const CONTRACTS: &[Guest] = &[("target:contracts", "contracts.wasm")];

/// The multi-guest composed deployment: three target guests plus one
/// source guest.
const COMPOSED: &[Guest] = &[
    ("target:contracts", "contracts.wasm"),
    ("target:omnia", "omnia.wasm"),
    ("target:vectis", "vectis.wasm"),
    ("source:documentation", "documentation.wasm"),
];

/// The remaining source guests, composed together.
const SOURCES: &[Guest] = &[
    ("source:intent", "intent.wasm"),
    ("source:typescript", "typescript.wasm"),
    ("source:screenshots", "screenshots.wasm"),
    ("source:captures", "captures.wasm"),
];

/// Assemble the contracts deployment into a runtime the tests can
/// dispatch into and serve HTTP through, with `"."` mounted at `mount`.
async fn runtime(mount: &Path) -> Result<Runtime<Bundle>> {
    assemble(manifest(CONTRACTS, mount)?).await
}

/// Assemble the multi-guest deployment (contracts + omnia + vectis +
/// documentation) into a runtime, with `"."` mounted at `mount`.
async fn composed_runtime(mount: &Path) -> Result<Runtime<Bundle>> {
    assemble(manifest(COMPOSED, mount)?).await
}

/// Assemble the source-guest deployment (intent + typescript +
/// screenshots + captures) into a runtime, with `"."` mounted at `mount`.
async fn source_guests_runtime(mount: &Path) -> Result<Runtime<Bundle>> {
    assemble(manifest(SOURCES, mount)?).await
}

/// A deployment manifest over `guests`: each guest's MCP references routed at
/// `/mcp/<name>`, sharing one writable `"."` mount — the shared project
/// tree every guest opens through its own preopen.
fn manifest(guests: &[Guest], mount: &Path) -> Result<TempManifest> {
    let entries: Vec<evals::Guest> = guests
        .iter()
        .map(|(id, file)| {
            let name = id.split_once(':').expect("guest id is `<axis>:<name>`").1;
            evals::Guest {
                id: (*id).to_owned(),
                wasm: guest_wasm(file),
                link: Vec::new(),
                route: Some(format!("/mcp/{name}")),
            }
        })
        .collect();
    temp_manifest(&evals::manifest(&entries, mount))
}

/// Locate a built wasm32-wasip2 guest component, building on first use.
fn guest_wasm(file: &str) -> PathBuf {
    build_guests();

    let path = target_dir().join("wasm32-wasip2").join("debug").join(file);
    assert!(
        path.exists(),
        "guest `{file}` not found at {path}; run `cargo build --workspace --target wasm32-wasip2`",
        path = path.display()
    );
    path
}

fn build_guests() {
    static GUESTS: OnceLock<()> = OnceLock::new();
    GUESTS.get_or_init(|| {
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("evals manifest dir is <workspace>/evals");
        // `--workspace` rather than a `-p` list: the bare spec `omnia` is
        // ambiguous between the guest crate and the runtime dependency
        // of the same name. Host-side members compile to empty crates on
        // wasm32, so the whole-workspace build is equivalent.
        let args = ["build", "--workspace", "--target", "wasm32-wasip2"];
        evals::cargo(&args, workspace_root, &target_dir()).expect("guest build");
    });
}

fn target_dir() -> PathBuf {
    evals::target_dir().expect("test exe sits at <target>/<profile>/deps/<exe>")
}

async fn assemble(manifest: TempManifest) -> Result<Runtime<Bundle>> {
    let mut deployment = DeploymentBuilder::new()
        .config(manifest.path().to_path_buf())
        .build::<StoreCtx<Bundle>>()
        .await
        .context("building deployment")?;
    deployment.host::<WasiHttp, Bundle>().context("linking http host")?;
    deployment.host::<WasiModel, Bundle>().context("linking model host")?;
    let mounts = deployment.mounts();
    let registry = deployment.into_registry().context("assembling registry")?;

    Ok(Runtime::from_parts(
        Arc::new(registry),
        Vec::new(),
        mounts,
        Bundle::connect().await.context("connecting backends")?,
    ))
}

/// The backend bundle a host binary's `runtime!` macro would generate.
///
/// Covers `hosts: { WasiHttp: HttpDefault, WasiModel: … }` — with the model
/// backend stubbed: these composed tests are model-free (judgment legs are
/// covered natively in each adapter crate), so any completion is a test bug.
#[derive(Clone)]
struct Bundle {
    http: HttpDefault,
    model: NoModel,
}

impl std::fmt::Debug for Bundle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bundle").finish_non_exhaustive()
    }
}

impl omnia::Backends for Bundle {
    async fn connect() -> Result<Self> {
        Ok(Self {
            http: HttpDefault::connect().await?,
            model: NoModel,
        })
    }
}

impl HasHttp for Bundle {
    fn http_view<'a>(&'a mut self, table: &'a mut ResourceTable) -> WasiHttpCtxView<'a> {
        self.http.as_view(table)
    }
}

impl HasModel for Bundle {
    fn model_ctx(&mut self) -> &mut dyn WasiModelCtx {
        &mut self.model
    }
}

/// A model backend that fails every completion: linked so the guest's
/// `omnia:model/completion` import resolves, never legitimately reached.
#[derive(Clone, Debug)]
struct NoModel;

impl WasiModelCtx for NoModel {
    fn complete(&self, _request: Request, _tool_host: Arc<dyn ToolHost>) -> FutureResult<Answer> {
        async {
            // Yield through the reactor before failing so the guest's
            // async-lifted export genuinely parks awaiting the import — the
            // probe must prove the seam survives a pending host future, not
            // just an immediately-ready one.
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            anyhow::bail!("model-free composed test: completion must not be called")
        }
        .boxed()
    }
}
