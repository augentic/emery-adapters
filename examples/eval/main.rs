//! The adapters repository's unpublished composition binary: native
//! command passthrough over the first-party catalog by default, the
//! live eval client under the `eval` subcommand.
//!
//! The composition root owns what the shared client (`eval::client`)
//! refuses to: the Tokio runtime, `std::env::args` collection, and
//! the first-party catalog and prompt-scenario declarations. It is a
//! development tool, never an install or release artifact.

use std::path::Path;
use std::process::ExitCode;

/// Prompt-scenario definitions, per adapter, under the lab's own tree.
const SCENARIOS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/scenarios");

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

async fn entry() -> anyhow::Result<ExitCode> {
    let catalog = lab::catalog()?;
    eval::client::run(std::env::args().collect(), catalog, Some(Path::new(SCENARIOS))).await
}
