# Testing

Integration-first test posture for the `specify-adapters` crates — the wasm-free adapter cores (`<name>-core`) and the shared guest-support crates under `crates/`. It mirrors the engine standard in [`augentic/specify` `engine/docs/standards/testing.md`](https://github.com/augentic/specify/blob/main/engine/docs/standards/testing.md): the unit layer is deliberately thin, integration owns every publicly reachable behavior, and `cargo llvm-cov` is the brake on deletion. The WIT contract, `core/tests/` integration suites, and the root `tests/` composed-deployment tests (the `adapter-tests` package) are the guardrails — design tests against those public surfaces, not private kernels. Read this before adding a new test or deleting one.

## Posture

Use `cargo make test` rather than `cargo test`. It runs `cargo nextest run --all --all-features --no-tests=pass` under `RUSTFLAGS=-Dwarnings`, matching CI. `nextest` is mandatory: it runs each test in its own process, and that isolation is what lets the CWD/env-mutating suites pass.

Each adapter core consolidates its integration suite into a single `it` binary: `core/tests/it.rs` pulls each area in as a `#[path = "<area>.rs"]` submodule (`mod operations;`, `mod scaffold;`, …) so the crate-under-test links exactly once. The guest shims (`{targets,sources}/<name>/src/`) are hand-written wasm32 export glue over `adapter`'s shared WIT bindings and carry no native tests; the composed-deployment seams are covered by the root `tests/` package (`adapter-tests`). Both it and the `evals/` live tests share the host-side harness crate at `crates/harness` (manifest rendering, the cargo runner, target-dir discovery, tree copying).

## The two layers — minimize the unit layer

Every behavior gets a home in exactly one layer. Decide the layer **before** writing the test; duplicating an assertion across layers is a defect, not extra safety. The standing bias is **fewer unit tests**.

| Layer | Location | Required when | Forbidden when |
| ----- | -------- | ------------- | -------------- |
| **Kernel unit** | `#[cfg(test)] mod tests` / sibling `tests.rs` next to the code | The branch is genuinely unreachable through the public API (a defensive guard, an error variant no caller triggers), **or** the behavior is a dense pure parse/projection / render-math edge matrix whose case-per-cell integration port would inflate the suite | The behavior is reachable through the crate's public surface and an integration test already covers it — or could, without a matrix explosion |
| **Crate integration** | `core/tests/` (via the consolidated `it` binary) | The behavior is reachable through the library API: engine invariants, filesystem-shape corners, render output, judgment-leg prompts against a mock `Model` | The same observable behavior is already asserted elsewhere and needs no coverage backfill |

## Triage rubric (applied to every `#[cfg(test)]` / `tests.rs`)

- **Delete** — the observable behavior is already asserted by an integration test, or it is tautological / mock-heavy / an internal snapshot that gives an agent no boundary signal.
- **Collapse (stay unit)** — a dense pure `(input → output/code)` matrix (e.g. `app_icon/canvas` render math, `svg`, `materialize/paths`) becomes one table-driven `#[test]` with a block per case. Coverage-neutral by construction.
- **Re-home** — behavior reachable through the library lands in the crate's `tests/` tree.
- **Keep** — a genuinely unreachable defensive branch / error variant no caller can trigger, with a one-line comment saying why an agent cannot get the same signal from integration.

## Coverage is the brake on deletion

`cargo llvm-cov` line/region coverage on still-live code is the safety net. The adapters [`Makefile.toml`](Makefile.toml) has no coverage task, so run it directly, per crate, before and after a reduction:

```bash
cargo llvm-cov nextest -p vectis-core --summary-only
cargo llvm-cov nextest -p contracts-core --summary-only
```

A `TOTAL` line/region drop on still-live code means real coverage was lost: backfill with an integration assertion (preferred) or revert that specific deletion. A pure collapse of redundant cases is coverage-neutral.

### Watch for non-deterministic coverage

The real trap is **non-deterministic coverage**: code that walks `std::fs::read_dir(...).flatten()` (e.g. `shell/launcher` icon detection) hits a different branch set depending on directory iteration order, so the `TOTAL` can wobble a line or two run-to-run on identical source. When a deletion appears to drop 1–2 lines, re-run the *baseline* too before chasing it — and if results look stale, force a full instrumented rebuild (`rm -rf target/llvm-cov-target`) rather than trusting `cargo llvm-cov clean` alone.

## Test naming

Test function names are identifiers, not sentences. The enclosing `tests/<area>.rs` module already names the subject — don't restate it in every `fn`. Push the narrative into the test body or a `//` comment above the `fn`.

## Definition of done for a reduction

- Source-side `#[test]` count is materially reduced, and every surviving unit test has a clear reason an agent cannot get the same signal from integration.
- `cargo llvm-cov nextest --summary-only` `TOTAL` holds on live code for every touched crate.
- `cargo make ci` is green.
- No `pub` / `pub(crate)` widening solely for tests, and no test-only trait pairs.
