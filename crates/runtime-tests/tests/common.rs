//! Shared helpers for the composed-deployment integration tests.
//!
//! Owns guest-artifact building/locating (this workspace's counterpart to
//! the specify engine's `crates/runtime/tests/common.rs`, pointed at the
//! `specify-*-guest` crates), the `wasi:http`-backed store bundle a host
//! binary's `runtime!` macro would generate, and the contracts deployment
//! manifest the tests deploy.

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

/// Built artifact name of the contracts target-adapter guest.
pub const CONTRACTS_WASM: &str = "specify_contracts.wasm";

/// The adapters workspace root (`<root>/crates/runtime-tests` is this crate).
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("runtime-tests manifest dir is <workspace>/crates/runtime-tests")
        .to_path_buf()
}

/// Locate a built wasm32-wasip2 guest component, building the guest crates
/// on first use (a fast no-op when fresh).
///
/// # Panics
///
/// Panics when the artifact is still absent after the build, pointing the
/// developer at `cargo make build-guests`.
pub fn guest_wasm(file: &str) -> PathBuf {
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
// serializes concurrent invocations across test binaries.
fn build_guests() {
    static GUESTS: OnceLock<()> = OnceLock::new();
    GUESTS.get_or_init(|| {
        let status = Command::new("cargo")
            .env("CARGO_TARGET_DIR", target_dir())
            .args(["build", "-p", "specify-contracts", "--target", "wasm32-wasip2"])
            .current_dir(workspace_root())
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

/// The contracts deployment manifest: the guest registered under
/// `target:contracts`, its MCP shelf routed at `/mcp/contracts`, and a
/// writable `"."` mount — the shared project tree every guest opens
/// through its own preopen.
///
/// # Errors
///
/// Returns an error when the temp manifest cannot be written.
pub fn contracts_manifest(mount: &Path) -> Result<TempManifest> {
    let contracts = guest_wasm(CONTRACTS_WASM);

    temp_manifest(&format!(
        "[[guest]]\n\
         id = \"target:contracts\"\n\
         source.path = \"{contracts}\"\n\n\
         [[mount]]\n\
         name = \".\"\n\
         path = \"{mount}\"\n\
         writable = true\n\n\
         [[route.http]]\n\
         prefix = \"/mcp/contracts\"\n\
         guest = \"target:contracts\"\n\n\
         [transport]\n\
         default = \"in-process\"\n",
        contracts = contracts.display(),
        mount = mount.display(),
    ))
}

/// Assemble the contracts deployment into a runtime the tests can dispatch
/// into and serve HTTP through, with `"."` mounted at `mount`.
///
/// # Errors
///
/// Returns an error when the deployment cannot be built or the backends
/// cannot connect.
pub async fn runtime(mount: &Path) -> Result<Runtime<Bundle>> {
    let manifest = contracts_manifest(mount)?;
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
/// covered natively in `specify-contracts-core` and live by the Milestone
/// E proof), so any completion is a test bug.
pub struct Bundle {
    http: HttpDefault,
    model: NoModel,
}

impl Clone for Bundle {
    fn clone(&self) -> Self {
        Self {
            http: self.http.clone(),
            model: NoModel,
        }
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
#[derive(Debug)]
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
