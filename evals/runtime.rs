//! Omnia `runtime!` host for adapter-guest evals (RFC-61 Step 2,
//! Milestone E).
//!
//! Binds the live cursor backend behind `wasi-model`: command mode drives
//! the eval guest's `wasi:cli/run` export once and exits with its status,
//! while the HTTP trigger serves the adapter guest's MCP reference route in
//! the background for the spawned `cursor-agent`. Runs via the `eval-contracts`
//! cargo-make task, or by hand:
//!
//! ```text
//! cargo run -p evals --example eval-driver -- run --config <manifest> -- eval <slice> <inputs-dir>
//! ```
//!
//! Requires `cursor-agent` on `PATH`, authenticated via `CURSOR_API_KEY` or a
//! prior `cursor-agent login`.

cfg_if::cfg_if! {
    if #[cfg(not(target_arch = "wasm32"))] {
        use omnia_cursor::Client as Cursor;
        use omnia_wasi_http::{HttpDefault, WasiHttp};
        use omnia_wasi_model::WasiModel;

        omnia::runtime!({
            mode: command,
            hosts: {
                WasiHttp: HttpDefault,
                WasiModel: Cursor,
            }
        });
    } else {
        fn main() {}
    }
}
