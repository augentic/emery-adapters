# Wasm Example

End-to-end run of the Emery change workflow over the real WASM component seam: the shipped `emery` binary from the sibling `augentic/emery` checkout (embedded engine guest) plus this repo's `documentation`, `intent`, and `contracts` adapter components.

There is no `omnia.toml`: the example invokes the built binary directly. The run script sandboxes the layout with `EMERY_HOME`, seeds the adapters via `emery adapter add`, and initializes the project by bare target name. Fixture inputs live under [fixture/](fixture/) (shared with the graded `orders-contracts` eval case).

See the [repo README](../../README.md) for the graded native eval-case repair loop; [docs/testing.md](../../docs/testing.md) for the five-rung map.

## Quick start

Login to the Cursor agent:

```bash
agent login
```

or set `CURSOR_API_KEY` in `.env`.

Requires the sibling [`augentic/emery`](https://github.com/augentic/emery) checkout at `../emery` (the example builds and drives that repo's shipped `emery` binary).

Run the example:

```bash
cargo make wasm-run
```

Each run wipes `sandbox/wasm/` then rebuilds only when Cargo says so. To remove leftover artifacts without re-running:

```bash
cargo make wasm-clean
```

Artifacts land under the gitignored `sandbox/wasm/` — the project tree at `sandbox/wasm/project/`, with the store and cache beside it.

`GUEST_TIMEOUT_MS` defaults to one hour (Omnia's per-invocation wall-clock cap; default is 30s). Set `RUST_LOG` yourself when debugging the seam. The runtime may log a non-fatal `no guest exports the http handler; http trigger inert` line per invocation — command mode proceeds without it. `CURSOR_MODEL=<model-id>` sets the default model when a request leaves it unset.

## What it demonstrates

1. Every command runs in the embedded engine guest; bound adapters fault in by routed id through the fail-closed resolver (`source:documentation`, `target:contracts`).
2. The documentation source surveys the fixture under `docs/` and extracts requirements.
3. Emery reconciles them and drives refine → build → merge.
4. The contracts target builds and merges the API contract surface.

After running, inspect the generated result at:

```text
sandbox/wasm/project/contracts/
```
