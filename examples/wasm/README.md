# WebAssembly (Wasm) Examples

These examples run the full Emery workflow end-to-end using real WebAssembly components. They combine the `emery` CLI tool (from the sibling `augentic/emery` repository) with the adapter components built in this repository.

## Quick start

Login to the Cursor agent:

```bash
agent login
```

or set `CURSOR_API_KEY` in `.env`.

Requires the sibling `[augentic/emery](https://github.com/augentic/emery)` checkout at `../emery` (the examples build and drive that repo's shipped `emery` binary).


| Scenario                           | Task                        | Graded native twin |
| ---------------------------------- | --------------------------- | ------------------ |
| documentation → contracts (orders) | `cargo make wasm-contracts` | `orders-contracts` |
| typescript → omnia (r9k migration) | `cargo make wasm-omnia-r9k` | `omnia-r9k`        |


```bash
cargo make wasm-contracts
cargo make wasm-omnia-r9k
```

Each run wipes its own sandbox then rebuilds only when Cargo says so. To remove leftover artifacts without re-running:

```bash
cargo make wasm-clean
```



## About

These examples run the `emery` binary directly (no `omnia.toml` required). The run scripts isolate the environment using `EMERY_HOME`, load local adapters via `emery adapter add`, and initialize the project.

We use `emery adapter add` to ensure the examples run against your freshly built, local `.wasm` components. If we didn't do this, Emery would automatically download and use the published adapter versions from GHCR.

**Key details:**
- **Artifacts:** Output files are saved in `sandbox/wasm-contracts/` and `sandbox/wasm-omnia-r9k/`. These git-ignored folders contain the project files, store, and cache.
- **Timeouts:** Per-spawn `cursor-agent` wall-clock uses the Cursor backend default (600s); raise `CURSOR_TIMEOUT_SECS` in `.env` if a leg needs longer.
- **Logging:** Set the `RUST_LOG` environment variable if you need to debug. You can safely ignore the `no guest exports the http handler` warning.
- **Model Selection:** Set `CURSOR_MODEL=<model-id>` to override the default AI model.
- **Further Reading:** See the [repo README](../../README.md) for the evaluation loop and [docs/testing.md](../../docs/testing.md) for the testing strategy.

## wasm-contracts

Fixture inputs live under [fixture/](fixture/) (shared with the graded `orders-contracts` eval case).

1. Bound adapters fault in by routed id (`source:documentation`, `target:contracts`).
2. The documentation source surveys the fixture under `docs/` and extracts requirements.
3. Emery reconciles them and drives refine → build → merge.
4. The contracts target builds and merges the API contract surface.

After running, inspect:

```text
sandbox/wasm-contracts/project/contracts/
```



## wasm-omnia-r9k

Migrates Propellerhead's `at_r9k_position_adapter` TypeScript tree into an Omnia WASM crate (`typescript` → `omnia`). Same rhythm as the graded `omnia-r9k` eval case, over the real component seam.

The upstream is `UNLICENSED`, so the script reuses the eval case's gitignored fixture cache (`examples/eval/cases/omnia-r9k/fixture/`). On miss it shallow-clones from Bitbucket and strips `.git`; later runs (and `cargo make eval omnia-r9k`) reuse that cache offline. Refresh with `rm -rf examples/eval/cases/omnia-r9k/fixture`.

1. Bound adapters fault in by routed id (`source:typescript`, `target:omnia`).
2. The typescript source surveys `legacy/at_r9k_position_adapter` and extracts behaviour.
3. Emery reconciles and drives refine → build → merge.
4. The omnia target builds the guest, crate, and tests, then merges.

After running, inspect the generated result under:

```text
sandbox/wasm-omnia-r9k/project/
```

Expect tens of minutes of live model time — the same cost class as `cargo make eval omnia-r9k`.