//! Builds adapter and probe components used by integration tests.
//!
//! Every source adapter and program becomes a `wasm32-wasip2` component.
//! `gen.rs` exposes their artifact paths to native tests. The component build
//! is a no-op when this package is itself compiled for WebAssembly, preventing
//! recursion.

fn main() {
    omnia_test::build::Components::in_workspace("../..")
        .scan_packages("sources")
        .group("adapter")
        .package("test-programs")
        .scan("crates/test-programs/programs")
        .sync_examples("crates/test-programs/Cargo.toml")
        // Inputs outside the nested build's dep-info.
        .track(["Cargo.lock", "crates/test-programs/src", "crates/test-programs/Cargo.toml"])
        .build()
        .write_gen("gen.rs");
}
