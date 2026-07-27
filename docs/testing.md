# Testing

Integration-first test posture for `specify-adapters`: integration owns every publicly reachable behavior, and the unit layer is deliberately thin. Design tests against the public surfaces — the WIT contract, each adapter crate's `tests/` suite, and the operator-invoked wasm example — never against private kernels. Read this before adding or deleting a test.

## The five rungs

Every rung runs from this repository with its own `cargo make` tasks; the engine repository tests itself independently (see [its developer loop guide](https://github.com/augentic/specify/blob/main/docs/contributing/dev-loop.md)). There is no cross-repo command surface.

Fastest feedback first. **Every behavior is asserted on exactly one rung** — duplicating an assertion across rungs is a defect, not extra safety.

| #   | Rung               | Owns                                                         | Entry                                                         |
| --- | ------------------ | ------------------------------------------------------------ | ------------------------------------------------------------- |
| 1   | Native crate tests | Operation behavior, prompt assembly (scripted `Harness`)     | `cargo nextest run -p <adapter>` / `cargo make test`          |
| 2   | Workflow eval cases | Cross-phase integration, real sources → working-tree outputs | [`examples/eval/`](../examples/eval/README.md)                |
| 3   | Build eval cases   | One target build over a refined fixture, prompt quality      | [`examples/eval/`](../examples/eval/README.md)                |
| 4   | Wasm example       | WASM/WIT conformance over the real component seam            | [`examples/wasm/`](../examples/wasm/README.md)                |
| 5   | Consumer project   | Code (not prose) iteration via seeded `.wasm`                | `cargo make adapter [name]` + `specify adapter add`           |

Ownership boundaries: omnia-testkit owns reusable model/runtime test mechanics; adapter `tests/` own operation behavior; the eval composition example owns the live case loop ([repo README](../README.md) for the day-to-day loop; [`examples/eval/`](../examples/eval/) for the case catalog); the wasm example owns component-seam conformance. Generic catalog/provider/command mechanics stay in `specify/crates/native/tests` (case/sandbox mechanics in `specify/crates/probe/tests`).

Sibling co-development: uncomment the `[patch."https://github.com/augentic/specify.git"]` block in the root `Cargo.toml` to resolve engine crates from `../specify` instead of the lockfile-pinned git dependencies.

Testing a brand-new adapter, including its first eval case and catalog wiring: [authoring.md](authoring.md).

### 1. Native crate tests — the inner loop

Each adapter crate is `cdylib` + `rlib`, so its wasm-free logic links natively and tests through `{targets,sources}/<name>/tests/<area>.rs`. Judgment legs use `omnia_testkit::model::Harness` to assert "did my prompt edit land in the assembled text"; adapter crates must not duplicate that model/runtime machinery. The wasm32-only guest shims (inline `mod guest` in each `src/lib.rs`) are single `adapter::source!` / `adapter::target!` invocations and carry no native tests.

```bash
cargo nextest run -p vectis   # one adapter
cargo make test               # the whole workspace, matching CI
```

`nextest` is mandatory: each test runs in its own process, which is what lets CWD/env-mutating suites pass. Never use bare `cargo test`.

### 2–3. Eval — live workflow and build cases

Native catalog, live cursor backend, operator-invoked (never CI). How to run, debug, and iterate: **[README.md](../README.md)**; eval case catalog under [`examples/eval/`](../examples/eval/README.md).

```bash
cargo make eval                              # list the cases
cargo make eval orders-contracts --restart   # a workflow case (rung 2)
cargo make eval omnia-health --restart       # a build case (rung 3)
cargo make lab -- --project-dir <dir> slice list
```

### 4. Wasm example — component seam

[`examples/wasm/`](../examples/wasm/README.md) — shipped `specify` binary + built adapter components over the real WIT seam. Operator-invoked; per-leg ungraded (the graded native workflow case is rung 2).

```bash
cargo make wasm-run
cargo make wasm-clean
```

### 5. Consumer project — seeded components

`cargo make adapter [adapter]` builds with fast profile settings (LTO off, opt-level 1) into `target/wasm32-wasip2/release/<name>.wasm`. Seed into a consumer project with `specify adapter add <path.wasm>` (re-run after each rebuild). Switching between `adapter` and `release` flavors changes the profile fingerprint and forces a rebuild.

```bash
cargo make adapter contracts   # one adapter; no argument builds every adapter
```

## The two layers — minimize the unit layer

Every behavior gets a home in exactly one layer. Decide the layer **before** writing the test. The standing bias is **fewer unit tests**.

| Layer                 | Location                                                       | Required when                                                                                                                                                                                                                                             | Forbidden when                                                                                                                                |
| --------------------- | -------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| **Kernel unit**       | `#[cfg(test)] mod tests` / sibling `tests.rs` next to the code | The branch is genuinely unreachable through the public API (a defensive guard, an error variant no caller triggers), **or** the behavior is a dense pure parse/projection/render-math matrix whose case-per-cell integration port would inflate the suite | The behavior is reachable through the crate's public surface and an integration test already covers it — or could, without a matrix explosion |
| **Crate integration** | `<adapter>/tests/` (one auto-discovered binary per area)       | The behavior is reachable through the library API: engine invariants, filesystem-shape corners, render output, judgment-leg prompts against a mock `Model`                                                                                                | The same observable behavior is already asserted elsewhere and needs no coverage backfill                                                     |

## Triage rules

Applied to every existing `#[cfg(test)]` / `tests.rs`:

- **Delete** — the observable behavior is already asserted by an integration test, or it is tautological, mock-heavy, or an internal snapshot that gives an agent no boundary signal.
- **Collapse (stay unit)** — a dense pure `(input → output/code)` matrix (e.g. `app_icon/canvas` render math, `svg`, `materialize/paths`) becomes one table-driven `#[test]` with a block per case. Coverage-neutral by construction.
- **Re-home** — behavior reachable through the library lands in the crate's `tests/` tree.
- **Keep** — a genuinely unreachable defensive branch or error variant no caller can trigger, with a one-line comment saying why an agent cannot get the same signal from integration.

## Coverage is the brake on deletion

`cargo llvm-cov` line/region coverage on still-live code is the safety net. The [`Makefile.toml`](../Makefile.toml) has no coverage task, so run it directly, per crate, before and after a reduction:

```bash
cargo llvm-cov nextest -p vectis --summary-only
cargo llvm-cov nextest -p contracts --summary-only
```

A `TOTAL` line/region drop on still-live code means real coverage was lost: backfill with an integration assertion (preferred) or revert that specific deletion. A pure collapse of redundant cases is coverage-neutral.

**Watch for non-deterministic coverage.** Code that walks `std::fs::read_dir(...).flatten()` (e.g. `shell/launcher` icon detection) hits a different branch set depending on directory iteration order, so the `TOTAL` can wobble a line or two run-to-run on identical source. When a deletion appears to drop 1–2 lines, re-run the *baseline* too before chasing it — and if results look stale, force a full instrumented rebuild (`rm -rf target/llvm-cov-target`) rather than trusting `cargo llvm-cov clean` alone.

## Test naming

Test function names are identifiers, not sentences. The enclosing `tests/<area>.rs` module already names the subject — don't restate it in every `fn`. Push the narrative into the test body or a `//` comment above the `fn`.

## Definition of done for a reduction

- Source-side `#[test]` count is materially reduced, and every surviving unit test has a clear reason an agent cannot get the same signal from integration.
- `cargo llvm-cov nextest --summary-only` `TOTAL` holds on live code for every touched crate.
- `cargo make ci` is green.
- No `pub` / `pub(crate)` widening solely for tests, and no test-only trait pairs.
