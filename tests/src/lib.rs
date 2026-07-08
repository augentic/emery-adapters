//! Shared helpers for the composed-deployment integration tests.
//!
//! Owns guest-artifact building/locating and the `wasi:http`-backed
//! store bundle a host binary's `runtime!` macro would generate;
//! manifest rendering and the cargo runner come from the shared
//! `harness` crate (`crates/harness`).

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
///
/// # Errors
///
/// Returns an error when the deployment cannot be built or the backends
/// cannot connect.
pub async fn runtime(mount: &Path) -> Result<Runtime<Bundle>> {
    assemble(manifest(CONTRACTS, mount)?).await
}

/// Assemble the multi-guest deployment (contracts + omnia + vectis +
/// documentation) into a runtime, with `"."` mounted at `mount`.
///
/// # Errors
///
/// As [`runtime`].
pub async fn composed_runtime(mount: &Path) -> Result<Runtime<Bundle>> {
    assemble(manifest(COMPOSED, mount)?).await
}

/// Assemble the source-guest deployment (intent + typescript +
/// screenshots + captures) into a runtime, with `"."` mounted at `mount`.
///
/// # Errors
///
/// As [`runtime`].
pub async fn source_guests_runtime(mount: &Path) -> Result<Runtime<Bundle>> {
    assemble(manifest(SOURCES, mount)?).await
}

/// A deployment manifest over `guests`: each guest's MCP references routed at
/// `/mcp/<name>`, sharing one writable `"."` mount — the shared project
/// tree every guest opens through its own preopen.
fn manifest(guests: &[Guest], mount: &Path) -> Result<TempManifest> {
    let entries: Vec<harness::Guest> = guests
        .iter()
        .map(|(id, file)| {
            let name = id.split_once(':').expect("guest id is `<axis>:<name>`").1;
            harness::Guest {
                id: (*id).to_owned(),
                wasm: guest_wasm(file),
                link: Vec::new(),
                route: Some(format!("/mcp/{name}")),
            }
        })
        .collect();
    temp_manifest(&harness::manifest(&entries, mount))
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
            .expect("tests manifest dir is <workspace>/tests");
        // `--workspace` rather than a `-p` list: the bare spec `omnia` is
        // ambiguous between the guest crate and the runtime dependency
        // of the same name. Host-side members compile to empty crates on
        // wasm32, so the whole-workspace build is equivalent.
        let args = ["build", "--workspace", "--target", "wasm32-wasip2"];
        harness::cargo(&args, workspace_root, &target_dir()).expect("guest build");
    });
}

fn target_dir() -> PathBuf {
    harness::target_dir().expect("test exe sits at <target>/<profile>/deps/<exe>")
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
pub struct Bundle {
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
