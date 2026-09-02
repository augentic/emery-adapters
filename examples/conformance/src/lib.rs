//! Harness for the component conformance suite (`tests/conformance.rs`).
//!
//! The build script compiles the conformance caller and every `sources/*`
//! adapter to `wasm32-wasip2` and generates one `pub const` path per
//! component plus `foreach_source!`. [`run`] assembles a command-mode
//! deployment the way the engine's runtime does — the caller as the
//! `wasi:cli/run` guest, one adapter declared under its routed id, the
//! `emery:adapter/source` seam, a read-only `.` mount over the scratch
//! project — over a scripted host-side model, so a scenario observes the
//! caller's exit status and the recorded model traffic.

#![cfg(not(target_arch = "wasm32"))]

mod model;

use std::fs;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context as _, Result};
use omnia::{
    DeploymentBuilder, ExitStatus, GuestEntry, Manifest, Mode, Mount, Provides, Runtime, StoreCtx,
    serve_links,
};
use omnia_wasi_model::{WasiModel, WasiModelCtx};
use tempfile::TempDir;

pub use self::model::{Exchange, ScriptedModel, Seen};

include!(concat!(env!("OUT_DIR"), "/gen.rs"));

/// The versioned `source` interface the deployment declares as its plugin
/// seam; tracks the `emery:adapter` WIT package the SDK embeds.
pub const SOURCE_INTERFACE: &str = "emery:adapter/source@0.1.0";

/// The store's backend bundle: a scripted model. A clone held by the
/// scenario reads the recorded traffic back after the run.
#[derive(Clone, Debug)]
pub struct Backends {
    /// The scripted model backend.
    pub model: ScriptedModel,
}

impl Backends {
    /// A bundle answering with `model`.
    #[must_use]
    pub const fn scripted(model: ScriptedModel) -> Self {
        Self { model }
    }
}

impl Provides<WasiModel> for Backends {
    fn borrow(&mut self) -> &mut dyn WasiModelCtx {
        &mut self.model
    }
}

/// One caller run against one adapter component.
#[derive(Clone, Copy, Debug)]
pub struct Call<'a> {
    /// The routed adapter id the caller dispatches to (`source:<name>`).
    pub id: &'a str,
    /// Path to the adapter component.
    pub wasm: &'a str,
    /// Caller arguments after the adapter id: `[key, content, flags...]`.
    pub argv: &'a [&'a str],
    /// The project directory, mounted read-only as `.`.
    pub project: &'a Scratch,
}

/// Drive the caller once against `call.wasm` over `backends`, returning
/// the caller's exit status.
///
/// # Errors
///
/// Returns an error if the deployment cannot be built or linked, or the
/// caller traps without exiting.
pub async fn run(call: Call<'_>, backends: Backends) -> Result<ExitStatus> {
    let manifest = Manifest::new()
        .plugins([SOURCE_INTERFACE])
        .guest(GuestEntry::new("caller", CALLER))
        .guest(GuestEntry::new(call.id, call.wasm))
        .mounts([call.project.mount()]);
    // The runtime supplies argv[0]; the adapter id leads the caller's own.
    let args: Vec<String> =
        std::iter::once(call.id).chain(call.argv.iter().copied()).map(String::from).collect();

    let mut built = DeploymentBuilder::new()
        .manifest(manifest)
        .mode(Mode::Command)
        .args(args)
        .build::<StoreCtx<Backends>>()
        .await
        .context("building deployment")?;
    built.host::<WasiModel, Backends>()?;

    let mounts = built.mounts();
    let args = built.args().to_vec();
    let registry = Arc::new(built.into_registry().context("assembling registry")?);
    let runtime = Runtime::from_parts(registry, args, mounts, backends);
    serve_links(&runtime).await.context("wiring host-mediated dispatch")?;

    let status = runtime.run_command().await;
    runtime.shutdown();
    status
}

/// A per-test project directory, removed on drop.
#[derive(Debug)]
pub struct Scratch(TempDir);

impl Scratch {
    /// The directory path.
    #[must_use]
    pub fn path(&self) -> &Path {
        self.0.path()
    }

    /// A read-only [`Mount`] preopening this directory as `.`.
    #[must_use]
    pub fn mount(&self) -> Mount {
        Mount {
            name: ".".to_owned(),
            path: self.path().to_path_buf(),
            writable: false,
        }
    }

    /// Writes `contents` at `relative`, creating parent directories.
    ///
    /// # Panics
    ///
    /// Panics if the file cannot be written.
    pub fn write(&self, relative: &str, contents: impl AsRef<[u8]>) {
        let target = self.path().join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).expect("creating scratch subdirectory");
        }
        fs::write(&target, contents).expect("writing scratch file");
    }
}

/// Create a fresh [`Scratch`] directory.
///
/// # Panics
///
/// Panics if the directory cannot be created.
#[must_use]
pub fn scratch() -> Scratch {
    Scratch(tempfile::tempdir().expect("creating scratch dir"))
}
