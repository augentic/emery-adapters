//! Compiles the conformance caller and every `sources/*` adapter to a
//! `wasm32-wasip2` component and generates `gen.rs`: one path constant per
//! component (`CALLER`, `SOURCE_<NAME>`) plus the `foreach_source!`
//! completeness macro, all through omnia-test's fixture pipeline.

fn main() {
    omnia_test::build::Components::in_workspace("../..")
        .scan_packages("sources")
        .group("source")
        .extra_package("caller")
        .track(["examples/caller", "Cargo.lock"])
        .build()
        .write_gen("gen.rs");
}
