//! Harness for the component conformance suite (`tests/conformance.rs`).
//!
//! The build script compiles the conformance caller and every `sources/*`
//! adapter to `wasm32-wasip2` and generates one `pub const` path per
//! component plus `foreach_source!`. [`run`] describes a command-mode
//! deployment the way the engine's runtime does — the caller as the
//! `wasi:cli/run` guest, one adapter declared under its routed id, the
//! `emery:adapter/source` seam, a read-only `.` mount over the scratch
//! project — through `omnia_test::host::Deployment`, over omnia-test's
//! scripted host-side model, so a scenario observes the caller's exit
//! status and the recorded model traffic.

#![cfg(not(target_arch = "wasm32"))]

use anyhow::Result;
use omnia::ExitStatus;
use omnia_test::host::Deployment;
pub use omnia_test::host::{Backends, Scratch, ScriptedModel, scratch};
pub use omnia_test::{Exchange, Seen};
use omnia_wasi_model::WasiModel;

include!(concat!(env!("OUT_DIR"), "/gen.rs"));

/// The versioned `source` interface the deployment declares as its plugin
/// seam; tracks the `emery:adapter` WIT package the SDK embeds.
pub const SOURCE_INTERFACE: &str = "emery:adapter/source@0.1.0";

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
pub async fn run(call: Call<'_>, backends: Backends<ScriptedModel>) -> Result<ExitStatus> {
    // The runtime supplies argv[0]; the adapter id leads the caller's own.
    Deployment::new()
        .plugins([SOURCE_INTERFACE])
        .guest("caller", CALLER)
        .guest(call.id, call.wasm)
        .command("caller")
        .mount(call.project.mount(false))
        .args(std::iter::once(call.id).chain(call.argv.iter().copied()))
        .run_host::<WasiModel, _>(backends)
        .await
}
