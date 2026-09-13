//! Example runtime
//!
//! The host every example runs under: command mode over the live Cursor
//! model. It compiles nothing in — each example's `omnia.toml` declares the
//! driver guest, the adapter component, the `emery:adapter/source` link, and
//! the tree it lends — so one host serves every adapter.
//!
//! Run it with `run --config examples/<adapter>/omnia.toml`.

cfg_if::cfg_if! {
    if #[cfg(target_arch = "wasm32")] {
        fn main() {}
    } else {
        use omnia_cursor::Client as Cursor;
        use omnia_wasi_model::WasiModel;
        use omnia_wasi_otel::{OtelDefault, WasiOtel};

        omnia::runtime!({
            mode: command,
            hosts: {
                WasiOtel: OtelDefault,
                WasiModel: Cursor,
            }
        });
    }
}
