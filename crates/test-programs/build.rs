//! Compiles every `sources/*` adapter and every `programs/<group>/<scenario>.rs`
//! to a `wasm32-wasip2` component and writes `gen.rs`, the artifact table the
//! native side `include!`s. The nested build compiles this package for
//! `wasm32` too, where `Components` is a no-op, so the recursion stops.

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
