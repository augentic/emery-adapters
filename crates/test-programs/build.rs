//! Compiles every source adapter, every target adapter, and every guest
//! program under `programs/` to a `wasm32-wasip2` component and generates
//! `gen.rs`: one path constant per component plus a `foreach_<group>!`
//! completeness macro per group.

fn main() {
    omnia_test::build::Components::in_workspace("../..")
        .packages(["emery-documentation", "emery-intent", "emery-typescript", "emery-python"])
        .group("adapter")
        .packages(["emery-node", "emery-omnia"])
        .group("target")
        .package("test-programs")
        .scan("crates/test-programs/programs")
        .sync_examples("crates/test-programs/Cargo.toml")
        // inputs outside the nested build's dep-info
        .track(["Cargo.lock", "crates/test-programs/src", "crates/test-programs/Cargo.toml"])
        .build()
        .write_gen("gen.rs");
}
