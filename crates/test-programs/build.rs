//! Compiles every guest the seam suites run to a `wasm32-wasip2` component
//! and generates `gen.rs`, the artifact table the native side of this crate
//! `include!`s, through omnia-test's fixture pipeline: the shipped adapters —
//! every `sources/*` package, the `cdylib` components themselves rather than
//! example stand-ins — as `ADAPTER_<NAME>` with a `foreach_adapter!` arm
//! each, and the scenario programs under `programs/<group>/<scenario>.rs`,
//! one path constant per program plus a `foreach_<group>!` completeness
//! macro per group directory.
//!
//! The nested build compiles this same package for `wasm32`, running this
//! script again; `Components` is a no-op under that target, so the recursion
//! stops there.

fn main() {
    omnia_test::build::Components::in_workspace("../..")
        .scan_packages("sources")
        .group("adapter")
        .package("test-programs")
        .scan("crates/test-programs/programs")
        .sync_examples("crates/test-programs/Cargo.toml")
        // The programs' shared helpers, their manifest, and the lockfile sit
        // outside the dep-info the nested build emits, so watch them
        // explicitly.
        .track(["Cargo.lock", "crates/test-programs/src", "crates/test-programs/Cargo.toml"])
        .build()
        .write_gen("gen.rs");
}
