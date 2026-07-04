# Vendored `augentic:specify` WIT pin

`specify.wit` is a **vendored copy** of the adapter contract published by [augentic/specify](https://github.com/augentic/specify) at `wit/specify.wit`. Adapter guest crates in this repo generate their bindings from it with `wit_bindgen::generate!({ world: "source-adapter" | "target-adapter", path: "../../../wit" })`.

This copy is a temporary pin: once the `augentic:specify` package is published via `wkg` (operator-handled, per RFC-61's cross-repo landing order), guest crates consume the published package and this directory is deleted. Until then, keep it byte-identical to the upstream file — `cargo make check-pins` compares against a sibling `../specify` checkout when one is present, and any contract change lands in the specify repo first.
