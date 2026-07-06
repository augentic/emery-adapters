//! Shared helpers for the composed-deployment integration tests.
//!
//! Owns guest-artifact building/locating (this workspace's counterpart
//! to the specify engine's `crates/runtime/tests/common.rs`), the
//! `wasi:http`-backed store bundle a host binary's `runtime!` macro
//! would generate, and the deployment manifests the tests deploy.

use std::path::{Path, PathBuf};
use std::process::Command;
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

/// A deployment manifest over `guests`: each guest's MCP shelf routed at
/// `/mcp/<name>`, sharing one writable `"."` mount — the shared project
/// tree every guest opens through its own preopen.
fn manifest(guests: &[Guest], mount: &Path) -> Result<TempManifest> {
    use std::fmt::Write as _;

    let mut doc = String::new();
    for (id, file) in guests {
        let wasm = guest_wasm(file);
        writeln!(doc, "[[guest]]\nid = \"{id}\"\nsource.path = \"{}\"\n", wasm.display())?;
    }
    writeln!(doc, "[[mount]]\nname = \".\"\npath = \"{}\"\nwritable = true\n", mount.display())?;
    for (id, _) in guests {
        let name = id.split_once(':').expect("guest id is `<axis>:<name>`").1;
        writeln!(doc, "[[route.http]]\nprefix = \"/mcp/{name}\"\nguest = \"{id}\"\n")?;
    }
    doc.push_str("[transport]\ndefault = \"in-process\"\n");
    temp_manifest(&doc)
}

/// Locate a built wasm32-wasip2 guest component, building the guest
/// crates on first use (a fast no-op when fresh). Panics when the
/// artifact is still absent after the build, pointing the developer at
/// `cargo make build-guests`.
fn guest_wasm(file: &str) -> PathBuf {
    build_guests();

    let path = target_dir().join("wasm32-wasip2").join("debug").join(file);
    assert!(
        path.exists(),
        "guest `{file}` not found at {path}; run `cargo make build-guests`",
        path = path.display()
    );
    path
}

// Build the guest crates once per test process; cargo's own build lock
// serializes concurrent invocations across test binaries. The package
// list mirrors the Makefile's `GUEST_PACKAGES`.
fn build_guests() {
    static GUESTS: OnceLock<()> = OnceLock::new();
    GUESTS.get_or_init(|| {
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(1)
            .expect("tests manifest dir is <workspace>/tests")
            .to_path_buf();
        let packages = [
            "contracts",
            "omnia-adapter",
            "vectis",
            "captures",
            "documentation",
            "intent",
            "screenshots",
            "typescript",
        ];
        let status = Command::new("cargo")
            .env("CARGO_TARGET_DIR", target_dir())
            .arg("build")
            .args(packages.iter().flat_map(|package| ["-p", package]))
            .args(["--target", "wasm32-wasip2"])
            .current_dir(workspace_root)
            .status()
            .expect("spawning guest build");
        assert!(status.success(), "guest build failed with status {status}");
    });
}

// The cargo target dir this test binary was built into (testkit's
// convention: the test exe sits at `<target>/<profile>/deps/<exe>`).
fn target_dir() -> PathBuf {
    let test_exe = std::env::current_exe().expect("test executable has a path");
    test_exe
        .ancestors()
        .nth(3)
        .expect("test exe sits at <target>/<profile>/deps/<exe>")
        .to_path_buf()
}

// Deploy a temp manifest onto the runtime with the test backend bundle.
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

/// The backend bundle a host binary's `runtime!` macro would generate for
/// `hosts: { WasiHttp: HttpDefault, WasiModel: … }` — with the model
/// backend stubbed: these composed tests are model-free (judgment legs are
/// covered natively in the core crates), so any completion is a test bug.
#[derive(Clone)]
pub struct Bundle {
    http: HttpDefault,
    model: NoModel,
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
