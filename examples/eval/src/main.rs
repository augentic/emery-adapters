//! The adapters repository's live composition example: native command
//! passthrough over the first-party catalog by default, the live eval
//! client under the `eval` subcommand.
//!
//! The composition root owns what the shared client (`probe::client`)
//! refuses to: the Tokio runtime, `std::env::args` collection, and
//! the first-party catalog and case declarations. It is a
//! development tool, never an install or release artifact. Driven by
//! `cargo make specify` and `cargo make eval`.

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;
#[cfg(not(target_arch = "wasm32"))]
use std::process::ExitCode;

/// Eval case data directories beside this example; the retained
/// per-case sandboxes live in the sibling `sandbox/` tree.
#[cfg(not(target_arch = "wasm32"))]
const CASES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/cases");

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() -> ExitCode {
    match entry().await {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
async fn entry() -> anyhow::Result<ExitCode> {
    probe::client::run(std::env::args().collect(), catalog()?, Some(Path::new(CASES))).await
}

/// Every first-party source and target adapter, linked once on its
/// axis and validated by [`native::Catalog::builder`].
#[cfg(not(target_arch = "wasm32"))]
fn catalog() -> Result<native::Catalog, native::Error> {
    native::Catalog::builder()
        .source::<captures::Adapter>()
        .source::<documentation::Adapter>()
        .source::<intent::Adapter>()
        .source::<screenshots::Adapter>()
        .source::<typescript::Adapter>()
        .target::<contracts::Adapter>()
        .target::<omnia_target::Adapter>()
        .target::<vectis::Adapter>()
        .build()
}
