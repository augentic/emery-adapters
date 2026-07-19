# Native example

This example runs the Specify command router in one process with the `documentation`, `intent`, and `contracts` adapters linked into the binary. It uses `native::Catalog` for static adapter composition and `native::command::run` for the CLI boundary; no Wasm components are built or loaded.

## Quick start

Requires authenticated `cursor-agent` on `PATH` (`cursor-agent login` or `CURSOR_API_KEY`).

```bash
cargo run --example native -- --help
cargo build --release --example native
```

The release binary is written to `target/release/examples/native`. Run it from the project directory it should operate on.

The native host executes trusted adapter code in-process. It does not provide component isolation, dynamic component loading, adapter-store lookup, or digest verification; use the Wasm deployment when those properties are required.
