//! Builds adapter and test-program `wasm32-wasip2` components and generates `gen.rs`.

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
