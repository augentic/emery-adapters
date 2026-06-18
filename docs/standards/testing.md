# Testing

Integration-first test posture for the `specify-adapters` extension crates (`specify-vectis`, `specify-contract`). It mirrors the engine standard in [`augentic/specify` `engine/docs/standards/testing.md`](https://github.com/augentic/specify/blob/main/engine/docs/standards/testing.md): the unit layer is deliberately thin, integration owns every CLI-reachable behavior, and `cargo llvm-cov` is the brake on deletion. Read this before adding a new test or deleting one.

## Posture

Use `cargo make test` rather than `cargo test`. It runs `cargo nextest run --all --all-features --no-tests=pass` under `RUSTFLAGS=-Dwarnings`, matching CI. `nextest` is mandatory: each extension's [`.config/nextest.toml`](../../.config/nextest.toml) caps fan-out at four subprocesses locally, and process isolation is what lets the CWD/env-mutating suites pass.

Each extension splits its integration suite into two binaries, by reach:

- **`tests/cli.rs`** — black-box. Drives the built extension through `assert_cmd` / WASI and asserts exit codes, stdout JSON, and filesystem effects. This is the CLI wire contract.
- **`tests/engine.rs`** — in-process. Pulls each area in as a `#[path = "engine/<area>.rs"]` submodule (`mod verify;`, `mod materialize;`, …) and calls the library API directly, with shared helpers in `tests/engine_support/`.

## The three layers — minimize the unit layer

Every behavior gets a home in exactly one layer. Decide the layer **before** writing the test; duplicating an assertion across layers is a defect, not extra safety. The standing bias is **fewer unit tests**.

| Layer | Location | Required when | Forbidden when |
| ----- | -------- | ------------- | -------------- |
| **Kernel unit** | `#[cfg(test)] mod tests` / sibling `tests.rs` next to the code | The branch is genuinely unreachable through the CLI (a defensive guard, an error variant no flag triggers), **or** the behavior is a dense pure parse/projection / render-math edge matrix whose case-per-cell integration port would inflate the 4-wide subprocess pool | The behavior is reachable through the extension and an integration test already covers it — or could, without a matrix explosion |
| **Extension integration** | `tests/engine.rs` (`tests/engine/<area>.rs`) | The behavior spans modules and is reachable through the library API but impractical to assert black-box — internal invariants, filesystem-shape corners, render output | The same observable behavior is already asserted black-box and needs no coverage backfill |
| **CLI black-box** | `tests/cli.rs` | The behavior is part of the wire contract: arg parsing, exit codes (0/1/2), stdout JSON shape, filesystem effects of a verb | The assertion re-tests kernel logic already covered elsewhere — black-box tests buy wiring confidence, not rule-by-rule matrices |

## Triage rubric (applied to every `#[cfg(test)]` / `tests.rs`)

- **Delete** — the observable behavior is already asserted by an integration test, or it is tautological / mock-heavy / an internal snapshot that gives an agent no boundary signal.
- **Collapse (stay unit)** — a dense pure `(input → output/code)` matrix (e.g. `app_icon/canvas` render math, `svg`, `materialize/paths`) becomes one table-driven `#[test]` with a block per case. Coverage-neutral by construction.
- **Re-home** — behavior reachable through the library lands in `tests/engine.rs` (in-process); behavior that is part of the CLI wire contract lands in `tests/cli.rs`.
- **Keep** — a genuinely CLI-unreachable defensive branch / error variant no flag can trigger, with a one-line comment saying why an agent cannot get the same signal from integration.

## Coverage is the brake on deletion

`cargo llvm-cov` line/region coverage on still-live code is the safety net. The adapters [`Makefile.toml`](../../Makefile.toml) has no coverage task, so run it directly, per crate, before and after a reduction:

```bash
cargo llvm-cov nextest -p specify-vectis --summary-only
cargo llvm-cov nextest -p specify-contract --summary-only
```

A `TOTAL` line/region drop on still-live code means real coverage was lost: backfill with an integration assertion (preferred) or revert that specific deletion. A pure collapse of redundant cases is coverage-neutral.

### Both test binaries contribute to coverage — but watch for non-determinism

`cargo llvm-cov nextest` **does** capture coverage from the `assert_cmd::Command::cargo_bin(...)` subprocesses `tests/cli.rs` spawns: the instrumented binary writes its own profile (proof: `specify-contract`'s `main.rs`, reachable only through the spawned binary, sits at ~97% under the gate). So both `cli.rs` and `engine.rs` count — pick the layer by *reach* (wire contract vs. library branch), not by a coverage-attribution myth.

The real trap is **non-deterministic coverage**: code that walks `std::fs::read_dir(...).flatten()` (e.g. `shell/launcher` icon detection) hits a different branch set depending on directory iteration order, so the `TOTAL` can wobble a line or two run-to-run on identical source. When a deletion appears to drop 1–2 lines, re-run the *baseline* too before chasing it — and if results look stale, force a full instrumented rebuild (`rm -rf target/llvm-cov-target`) rather than trusting `cargo llvm-cov clean` alone.

Worked example — the verify re-home: the 17 `src/verify/tests.rs` unit tests were deleted; their wire-contract behavior was already covered by `tests/cli.rs`, and the genuinely-uncovered library branches (the `render_json` error path, bootstrap app-icon modes, Android shell-resident launcher detection, catalog early-returns) were re-homed to `tests/engine/verify.rs` so the whole-workspace `TOTAL` held.

## Test naming

Test function names are identifiers, not sentences. The enclosing `tests/engine/<area>.rs` module already names the subject — don't restate it in every `fn`. Push the narrative into the test body or a `//` comment above the `fn`.

## Definition of done for a reduction

- Source-side `#[test]` count is materially reduced, and every surviving unit test has a clear reason an agent cannot get the same signal from integration.
- `cargo llvm-cov nextest --summary-only` `TOTAL` holds on live code for every touched crate.
- `cargo make ci` is green.
- No `pub` / `pub(crate)` widening solely for tests, and no test-only trait pairs.
