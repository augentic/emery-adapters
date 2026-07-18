//! Composed-deployment smoke tests over built adapter WASM guests — model-free.
//! One shared runtime exercises metadata, guidance, async bridge legs, and MCP routes.

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

const TARGET_INTERFACE: &str = "specify:adapter/target@0.1.0";
const SOURCE_INTERFACE: &str = "specify:adapter/source@0.1.0";

async fn metadata(runtime: &Runtime<Bundle>, guest: &str) -> Result<()> {
    use omnia::wasmtime::component::Val;

    let interface = if guest.starts_with("target:") { TARGET_INTERFACE } else { SOURCE_INTERFACE };
    let results = runtime
        .dispatcher()
        .invoke(
            guest.into(),
            Some(interface.to_string()),
            "metadata".to_string(),
            vec![Val::String(guest.to_string())],
        )
        .await
        .with_context(|| format!("dispatching metadata to {guest}"))?;

    let [Val::Record(fields)] = results.as_slice() else {
        anyhow::bail!("{guest} metadata returned an unexpected shape: {results:?}");
    };
    assert!(
        fields.iter().any(|(key, value)| key == "specify-floor" && *value == Val::Option(None)),
        "{guest} declares no compatibility floor: {fields:?}"
    );

    if guest.starts_with("source:") {
        return Ok(());
    }
    assert_target_metadata(guest, fields)
}

fn assert_target_metadata(
    guest: &str, fields: &[(String, omnia::wasmtime::component::Val)],
) -> Result<()> {
    use omnia::wasmtime::component::Val;

    let field = |name: &str| {
        fields
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value)
            .with_context(|| format!("{guest} metadata carries `{name}`: {fields:?}"))
    };
    let Val::List(inputs) = field("inputs")? else {
        anyhow::bail!("{guest} metadata `inputs` is a list: {fields:?}");
    };
    let declared: Vec<(&str, bool)> = inputs
        .iter()
        .map(|input| {
            let Val::Record(entries) = input else {
                panic!("{guest} build input is a record: {input:?}");
            };
            let path = entries.iter().find_map(|(key, value)| match (key.as_str(), value) {
                ("path", Val::String(path)) => Some(path.as_str()),
                _ => None,
            });
            let required = entries.iter().find_map(|(key, value)| match (key.as_str(), value) {
                ("required", Val::Bool(required)) => Some(*required),
                _ => None,
            });
            (
                path.unwrap_or_else(|| panic!("{guest} build input carries a path: {entries:?}")),
                required
                    .unwrap_or_else(|| panic!("{guest} build input carries required: {entries:?}")),
            )
        })
        .collect();

    match guest {
        "target:contracts" => {
            assert_eq!(declared, [("contracts", false)]);
            assert_eq!(field("platforms")?, &Val::Option(None));
        }
        "target:omnia" => {
            assert!(declared.is_empty());
            assert_eq!(field("platforms")?, &Val::Option(None));
        }
        "target:vectis" => {
            assert_eq!(
                declared,
                [("tokens.yaml", false), ("assets.yaml", false), ("components.yaml", false)]
            );
            let Val::Option(Some(platforms)) = field("platforms")? else {
                anyhow::bail!("vectis declares a platforms capability: {fields:?}");
            };
            let Val::Record(capability) = platforms.as_ref() else {
                anyhow::bail!("vectis platforms capability is a record: {platforms:?}");
            };
            let capability_field = |name: &str| {
                capability
                    .iter()
                    .find(|(key, _)| key == name)
                    .map(|(_, value)| value)
                    .with_context(|| format!("vectis platforms capability carries `{name}`"))
            };
            assert_eq!(capability_field("required")?, &Val::Bool(true));
            let platform_names = |name: &str| -> Result<Vec<&str>> {
                let Val::List(platforms) = capability_field(name)? else {
                    anyhow::bail!("vectis platforms capability `{name}` is a list");
                };
                Ok(platforms
                    .iter()
                    .map(|platform| {
                        let Val::Enum(name) = platform else {
                            panic!("vectis platform is an enum: {platform:?}");
                        };
                        name.as_str()
                    })
                    .collect())
            };
            assert_eq!(platform_names("allowed")?, ["core", "ios", "android", "web", "desktop"]);
            assert_eq!(platform_names("default")?, ["core", "ios", "android"]);
        }
        _ => anyhow::bail!("unexpected target component `{guest}`"),
    }
    Ok(())
}

mod contracts {
    use anyhow::{Context as _, Result};
    use omnia::Runtime;
    use omnia::wasmtime::component::Val;
    use omnia_testkit::http;
    use serde_json::{Value, json};

    use super::{Bundle, TARGET_INTERFACE};

    pub async fn guidance(runtime: &Runtime<Bundle>) -> Result<()> {
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

    // Stub model pends then fails: the leg must return a WIT error, not trap.
    pub async fn build_bridge(runtime: &Runtime<Bundle>) -> Result<()> {
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

    pub async fn references(runtime: &Runtime<Bundle>) -> Result<()> {
        let init = post(
            runtime,
            &json!({
                "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": "2025-06-18" }
            }),
        )
        .await?;
        assert_eq!(init["result"]["serverInfo"]["name"], "contracts-references");

        let resources =
            post(runtime, &json!({ "jsonrpc": "2.0", "id": 2, "method": "resources/list" }))
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
            runtime,
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
            runtime,
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

mod omnia_guest {
    use anyhow::{Context as _, Result};
    use omnia::Runtime;
    use omnia::wasmtime::component::Val;
    use omnia_testkit::http;
    use serde_json::{Value, json};

    use super::{Bundle, TARGET_INTERFACE};

    pub async fn guidance(runtime: &Runtime<Bundle>) -> Result<()> {
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

    pub async fn references(runtime: &Runtime<Bundle>) -> Result<()> {
        let init = post(
            runtime,
            &json!({
                "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": "2025-06-18" }
            }),
        )
        .await?;
        assert_eq!(init["result"]["serverInfo"]["name"], "omnia-references");

        let reference = post(
            runtime,
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
            runtime,
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

mod vectis {
    use anyhow::{Context as _, Result};
    use omnia::Runtime;
    use omnia::wasmtime::component::Val;
    use omnia_testkit::http;
    use serde_json::{Value, json};

    use super::{Bundle, TARGET_INTERFACE};

    pub async fn guidance(runtime: &Runtime<Bundle>) -> Result<()> {
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

    pub async fn references(runtime: &Runtime<Bundle>) -> Result<()> {
        let init = post(
            runtime,
            &json!({
                "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": "2025-06-18" }
            }),
        )
        .await?;
        assert_eq!(init["result"]["serverInfo"]["name"], "vectis-references");

        let reference = post(
            runtime,
            &json!({
                "jsonrpc": "2.0", "id": 2, "method": "tools/call",
                "params": { "name": "read_doc", "arguments": { "path": "references/hard-rules-core.md" } }
            }),
        )
        .await?;
        let text = reference["result"]["content"][0]["text"].as_str().unwrap_or_default();
        assert!(!text.is_empty(), "read_doc returns the reference body: {reference}");

        let leg_prompt = post(
            runtime,
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

mod documentation {
    use anyhow::{Context as _, Result};
    use omnia::Runtime;
    use omnia::wasmtime::component::Val;
    use omnia_testkit::http;
    use serde_json::{Value, json};

    use super::{Bundle, SOURCE_INTERFACE};

    // Stub model pends then fails: the leg must return a WIT error, not trap.
    pub async fn survey_bridge(runtime: &Runtime<Bundle>) -> Result<()> {
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

    pub async fn per_guest_shelves(runtime: &Runtime<Bundle>) -> Result<()> {
        let init = post(
            runtime,
            "/mcp/documentation",
            &json!({
                "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": "2025-06-18" }
            }),
        )
        .await?;
        assert_eq!(init["result"]["serverInfo"]["name"], "documentation-references");

        let prompt = post(
            runtime,
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
            runtime,
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

mod sources {
    use anyhow::{Context as _, Result};
    use omnia::Runtime;
    use omnia::wasmtime::component::Val;
    use omnia_testkit::http;
    use serde_json::{Value, json};

    use super::{Bundle, SOURCE_INTERFACE};

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

    // Stub model pends then fails: each leg must return a WIT error, not trap.
    pub async fn survey_bridges(runtime: &Runtime<Bundle>) -> Result<()> {
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

    pub async fn per_guest_shelves(runtime: &Runtime<Bundle>) -> Result<()> {
        for (guest, route, server, heading) in GUESTS {
            let init = post(
                runtime,
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
                runtime,
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn component_smoke() -> Result<()> {
    let mount = tempfile::tempdir()?;
    let runtime = component_runtime(mount.path()).await?;

    for (guest, _) in COMPONENTS {
        metadata(&runtime, guest).await?;
    }
    contracts::guidance(&runtime).await?;
    contracts::build_bridge(&runtime).await?;
    contracts::references(&runtime).await?;
    omnia_guest::guidance(&runtime).await?;
    omnia_guest::references(&runtime).await?;
    vectis::guidance(&runtime).await?;
    vectis::references(&runtime).await?;
    documentation::survey_bridge(&runtime).await?;
    documentation::per_guest_shelves(&runtime).await?;
    sources::survey_bridges(&runtime).await?;
    sources::per_guest_shelves(&runtime).await
}

type Guest = (&'static str, &'static str);

const COMPONENTS: &[Guest] = &[
    ("target:contracts", "contracts.wasm"),
    ("target:omnia", "omnia.wasm"),
    ("target:vectis", "vectis.wasm"),
    ("source:documentation", "documentation.wasm"),
    ("source:intent", "intent.wasm"),
    ("source:typescript", "typescript.wasm"),
    ("source:screenshots", "screenshots.wasm"),
    ("source:captures", "captures.wasm"),
];

async fn component_runtime(mount: &Path) -> Result<Runtime<Bundle>> {
    assemble(manifest(COMPONENTS, mount)?).await
}

fn manifest(guests: &[Guest], mount: &Path) -> Result<TempManifest> {
    let entries: Vec<composed::Guest> = guests
        .iter()
        .map(|(id, file)| {
            let name = id.split_once(':').expect("guest id is `<axis>:<name>`").1;
            composed::Guest {
                id: (*id).to_owned(),
                wasm: guest_wasm(file),
                link: Vec::new(),
                route: Some(format!("/mcp/{name}")),
            }
        })
        .collect();
    temp_manifest(&composed::manifest(&entries, mount))
}

fn guest_wasm(file: &str) -> PathBuf {
    build_guests();

    let path = target_dir().join("wasm32-wasip2").join("debug").join(file);
    assert!(
        path.exists(),
        "guest `{file}` not found at {path}; run `cargo build --workspace --exclude lab --target wasm32-wasip2`",
        path = path.display()
    );
    path
}

fn build_guests() {
    static GUESTS: OnceLock<()> = OnceLock::new();
    GUESTS.get_or_init(|| {
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("composed manifest dir is at the workspace root");
        // `--workspace` avoids `-p omnia` ambiguity between guest crate and
        // runtime dep; `lab` is the native-only composition binary.
        let args = ["build", "--workspace", "--exclude", "lab", "--target", "wasm32-wasip2"];
        composed::cargo(&args, workspace_root, &target_dir()).expect("guest build");
    });
}

fn target_dir() -> PathBuf {
    composed::target_dir().expect("test exe sits at <target>/<profile>/deps/<exe>")
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

#[derive(Clone, Debug)]
struct NoModel;

impl WasiModelCtx for NoModel {
    fn complete(&self, _request: Request, _tool_host: Arc<dyn ToolHost>) -> FutureResult<Answer> {
        async {
            // Yield so the guest parks on a pending host future before failing.
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            anyhow::bail!("model-free composed test: completion must not be called")
        }
        .boxed()
    }
}
