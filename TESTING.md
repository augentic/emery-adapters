# Testing

Integration-first test posture for the `specify-adapters` crates — the wasm-free adapter cores (`specify-<name>-core`) and the shared guest-support crates under `crates/`. It mirrors the engine standard in [`augentic/specify` `engine/docs/standards/testing.md`](https://github.com/augentic/specify/blob/main/engine/docs/standards/testing.md): the unit layer is deliberately thin, integration owns every publicly reachable behavior, and `cargo llvm-cov` is the brake on deletion. A ratchet gate ([`tools/rust-quality`](tools/rust-quality)) holds each adapter's `src` unit-test count down — the lever is designing tests against the public surface, not widening it. Read this before adding a new test or deleting one.

## Posture

Use `cargo make test` rather than `cargo test`. It runs `cargo nextest run --all --all-features --no-tests=pass` under `RUSTFLAGS=-Dwarnings`, matching CI. `nextest` is mandatory: it runs each test in its own process, and that isolation is what lets the CWD/env-mutating suites pass.

Each adapter core consolidates its integration suite into a single `it` binary: `core/tests/it.rs` pulls each area in as a `#[path = "<area>.rs"]` submodule (`mod operations;`, `mod scaffold;`, …) so the crate-under-test links exactly once. The guest shims (`{targets,sources}/<name>/src/`) are hand-written wasm32 export glue over `specify-guest-kit`'s shared WIT bindings and carry no native tests; the composed-deployment seams are covered by `crates/runtime-tests`.

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

## The src unit-test ratchet

A strict ratchet enforces this posture in CI. [`tools/rust-quality`](tools/rust-quality) is a `publish = false` workspace member whose `unit_test_budget_holds` test counts `#[test]` / `#[tokio::test]` declarations under each adapter's `src/` trees (the guest shim and its `crates/*/src/` sub-crates) and holds them to the committed budget in [`tools/rust-quality/rust_quality_budget.toml`](tools/rust-quality/rust_quality_budget.toml). It mirrors the engine gate and runs under `cargo make test`.

- Adding a `src` unit test fails CI unless you raise that adapter's budget — a reviewable edit that must be justified.
- Removing one fails until you ratchet the budget down to the new count.

The lever is to design positive/negative tests against the **existing** public surface (the library API in the crate's `tests/` tree), then re-home already-`pub` kernels, then collapse the private affordability residue in place. Widening production API to test a private kernel is a last resort, not the lever; the target is *near-zero* `src` unit tests, not literal zero. The gate counts tests; `cargo llvm-cov` still guards behavior — both stay. Every adapter currently sits at the implicit budget of 0.

**Phase 2 (deferred):** once the residue is small, the numeric budget is replaced by a per-test marker — every surviving `src` `#[cfg(test)]` carries a `// rust-quality:allow(unit-test) reason: …` line, the gate flags any unmarked `src` `#[test]`, and `rust_quality_budget.toml` retires. Not built yet; until then the budget file is the contract.

## Coverage is the brake on deletion

`cargo llvm-cov` line/region coverage on still-live code is the safety net. The adapters [`Makefile.toml`](Makefile.toml) has no coverage task, so run it directly, per crate, before and after a reduction:

```bash
cargo llvm-cov nextest -p specify-vectis-core --summary-only
cargo llvm-cov nextest -p specify-contracts-core --summary-only
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
