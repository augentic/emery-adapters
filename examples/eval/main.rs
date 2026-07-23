//! The adapters repository's live composition example: native command
//! passthrough over the first-party catalog by default, the live eval
//! client under the `eval` subcommand.
//!
//! The composition root owns what the shared client (`probe::client`)
//! refuses to: the Tokio runtime, `std::env::args` collection, and
//! the first-party catalog and prompt-scenario declarations. It is a
//! development tool, never an install or release artifact. Driven by
//! `cargo make specify` and `cargo make eval`.

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;
#[cfg(not(target_arch = "wasm32"))]
use std::process::ExitCode;

/// Prompt-scenario definitions, per adapter, beside this example.
#[cfg(not(target_arch = "wasm32"))]
const SCENARIOS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/eval/scenarios");

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
    let catalog = adapters::catalog()?;
    probe::client::run(std::env::args().collect(), catalog, Some(Path::new(SCENARIOS))).await
}
