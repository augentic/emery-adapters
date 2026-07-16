//! `specify-dev` — the Rust-native dev shim and eval harness.
//!
//! Three modes over the same handler layer the wasm guest serves:
//!
//! - **CLI mode** (default, [`command`]): the shared typed command
//!   router against the native provider, plus an ephemeral MCP shelf.
//!   A leading shim-global `--project-dir <path>` anchors the provider
//!   (and the model's lent workspace) at another project root.
//! - **`serve` mode** ([`http`]): the shared typed HTTP router merged
//!   with the `/mcp/<name>` shelves on one `TcpListener`; carries its
//!   own `--project-dir` flag.
//! - **`eval` mode** ([`trial`], [`eval::scenario`]): the live-model rungs,
//!   mirroring the engine's `crates/eval`. The trial runs the operator
//!   rhythm over a persistent `sandbox/eval/` project with the linked
//!   adapters, graded by deterministic validators only; `eval scenario`
//!   drives one adapter operation over a seeded scratch tree for fast
//!   prompt iteration.

mod command;
mod grade;
mod http;
mod telemetry;
mod trial;

use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().collect();
    match argv.get(1).map(String::as_str) {
        Some("serve") => match http::serve(&argv[1..]).await {
            Ok(code) => code,
            Err(err) => {
                eprintln!("specify-dev: {err:#}");
                ExitCode::FAILURE
            }
        },
        Some("eval") => match trial::run(&argv[1..]).await {
            Ok(code) => code,
            Err(err) => {
                eprintln!("specify-dev: {err:#}");
                ExitCode::FAILURE
            }
        },
        _ => ExitCode::from(command::run(argv).await),
    }
}
