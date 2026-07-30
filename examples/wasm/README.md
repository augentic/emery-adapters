# Wasm Examples

End-to-end runs of the Emery change workflow over the real WASM component seam: the shipped `emery` binary from the sibling `augentic/emery` checkout (embedded engine guest) plus this repo's built adapter components.

There is no `omnia.toml`: each scenario invokes the built binary directly. The run scripts sandbox the layout with `EMERY_HOME`, seed adapters via `emery adapter add`, and initialize the project by bare target name.

`adapter add` remains the local-bytes path, and cache hits always win: a bare name whose component is seeded in the project cache stays bare and resolves the seed. In production (no seed), a bare first-party name auto-pins to the host binary's embedded adapter train (`emery:<name>@<train>`, shown by `emery --version`) and pulls from GHCR — which is why the `adapter add` lines in the example Makefile stay: they pin the runs to the freshly built debug `.wasm` components instead of the published train.

See the [repo README](../../README.md) for the graded native eval-case repair loop; [docs/testing.md](../../docs/testing.md) for the five-rung map.

## Quick start

Login to the Cursor agent:

```bash
agent login
```

or set `CURSOR_API_KEY` in `.env`.

Requires the sibling [`augentic/emery`](https://github.com/augentic/emery) checkout at `../emery` (the examples build and drive that repo's shipped `emery` binary).

| Scenario | Task | Graded native twin |
| --- | --- | --- |
| documentation → contracts (orders) | `cargo make wasm-contracts` | `orders-contracts` |
| typescript → omnia (r9k migration) | `cargo make wasm-omnia-r9k` | `omnia-r9k` |

```bash
cargo make wasm-contracts
cargo make wasm-omnia-r9k
```

Each run wipes its own sandbox then rebuilds only when Cargo says so. To remove leftover artifacts without re-running:

```bash
cargo make wasm-clean
```

Artifacts land under the gitignored sandboxes — `sandbox/wasm-contracts/` and `sandbox/wasm-omnia-r9k/` — each with a `project/` tree and the store/cache beside it.

`GUEST_TIMEOUT_MS` defaults to one hour (Omnia's per-invocation wall-clock cap; default is 30s). Set `RUST_LOG` yourself when debugging the seam. The runtime may log a non-fatal `no guest exports the http handler; http trigger inert` line per invocation — command mode proceeds without it. `CURSOR_MODEL=<model-id>` sets the default model when a request leaves it unset.

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
