# Testing

Integration-first test posture for `specify-adapters`: integration owns every publicly reachable behavior, and the unit layer is deliberately thin. Design tests against the public surfaces — the WIT contract, each adapter crate's `tests/` suite, and the composed-deployment tests — never against private kernels. Read this before adding or deleting a test.

## The development loop

Every rung runs from this repository with its own `cargo make` tasks; the engine repository tests itself independently (see [its developer loop guide](https://github.com/augentic/specify/blob/main/docs/contributing/dev-loop.md)). There is no cross-repo command surface — the rungs below are this repo's own test strata.

Six rungs, fastest feedback first. Every behavior is asserted on exactly one rung — duplicating an assertion across rungs is a defect, not extra safety. Omnia-testkit owns reusable model and runtime test mechanics (the scripted double, runtime assembly); the repo's own `crates/testkit` owns the request-recording harness the adapter suites assert prompts and grants with. Adapter native tests own operation behavior, the eval crate owns cross-phase integration and prompt quality, and composed tests own WASM/WIT conformance.

### 1. Native crate tests — the inner loop

Each adapter crate is `cdylib` + `rlib`, so its wasm-free logic links natively and tests through the standard auto-discovered suite at `{targets,sources}/<name>/tests/<area>.rs`. These tests own operation behavior, including request validation, deterministic projection, and prompt assembly. Judgment legs use `testkit::Harness` (the recording decorator over omnia-testkit's scripted double) to assert "did my prompt edit land in the assembled text"; adapter crates must not duplicate that model/runtime machinery. The wasm32-only guest shims (the inline `mod guest` in each `src/lib.rs`) are single `adapter::source!` / `adapter::target!` export-macro invocations and carry no native tests.

```bash
cargo nextest run -p vectis   # one adapter
cargo make test               # the whole workspace, matching CI
```

`nextest` is mandatory, not a preference: it runs each test in its own process, and that isolation is what lets the CWD/env-mutating suites pass. Never use bare `cargo test`.

### 2. The eval crate — linked-adapter integration and the live rungs

`crates/eval/` is a native-only workspace member declaring only the linked first-party adapters. The engine-owned `specify/crates/harness` supplies the provider, model bridge, telemetry, CLI shim, deterministic grading, and trial/scenario runners; `cargo make eval` passes the trial inputs explicitly. The `eval` binary runs production verb handlers natively without building WebAssembly. Generic catalog, provider, MCP, CLI, and scenario mechanics are tested in `specify/crates/harness/tests`; the eval binding carries no separate deterministic suite. `cargo make ci` builds and lints the crate like any other member; the live rungs below stay operator-invoked.

```bash
cargo make dev -- --project-dir <dir> slice list   # any specify verb, natively
```

The same crate carries the repo's **live trial**: `cargo make eval` (the `eval` binary's `eval` mode) runs the operator rhythm — `init → plan author → transition approved → plan execute → plan archive` — over a persistent gitignored `sandbox/` project, with `documentation` + `intent` bound as sources and `contracts` as the target. It mirrors the engine's `crates/eval` trial: production verbs through the shared typed command router, the live cursor backend at the model seam, and **deterministic grading only** — every plan entry `done`, provenance on evidenced requirements, and the merged `contracts/` baseline non-empty and clean under the adapter's own compiled-in validators. Per-leg completion-request counts are reported (requests beyond one per leg are repairs), never asserted. Every phase pins `SPECIFY_PROJECT_CACHE` inside the sandbox, so runs are hermetic with respect to the operator's normal cache; the operator inputs and seed come from the shared `examples/change/trial.env` and `examples/change/seed/`, the same definition the wasm change example replays. A full trial takes tens of minutes of live model time. A full pass cleans the sandbox; a failing phase retains it for in-place review and per-phase re-runs. Live-only — requires an authenticated [`cursor-agent`](https://cursor.com/docs/cli) on `PATH`; `SPECIFY_EVAL_MODEL=<model-id>` overrides the model.

```bash
cargo make eval           # the full trial
cargo make eval init      # one phase: init | plan | execute | finalize | clean
```

Sibling co-development needs no pin dance: the committed `[patch."https://github.com/augentic/specify.git"]` section in the root `Cargo.toml` resolves every engine crate from the sibling `../specify` checkout, so uncommitted engine changes are picked up directly.

### 3. Single-operation prompt scenarios — fast prompt iteration

`cargo make eval scenario <adapter>/<name>` drives one adapter operation (one `build`, or one `merge` gate) end-to-end against the real cursor backend, natively through the same seam provider the trial uses. This is the fast prompt-iteration rung — one operation, one scratch tree, minutes not a full change; prompt edits under `{targets,sources}/<name>/prose/**` rebuild natively in seconds, so there is no overlay mode. Scenarios prove prompt quality over the linked crates, not WASM/WIT conformance — that stays with rungs 4 and 5. Each scenario is a data directory under [`crates/eval/scenarios/`](crates/eval/scenarios/README.md) — `scenario.toml` routing plus `inputs/*.md` and an optional `seed/**`. For an adapter already linked into the shim that directory is all it takes; a third-party adapter additionally needs a Cargo dependency and an adapter entry in `crates/eval/src/main.rs`, because configuration alone cannot link a Rust crate. Each run retains its isolated scratch tree (with the project cache pinned inside it) and a `report.json` under the gitignored `sandbox/<adapter>/<name>/run-…/`; a failing adapter report or a missing `expect` artifact fails the run and persists `outcome: fail`. Requires [`cursor-agent`](https://cursor.com/docs/cli) on `PATH`; `SPECIFY_EVAL_MODEL=<model-id>` overrides the model.

```bash
cargo make eval scenario                     # list scenarios
cargo make eval scenario contracts/design    # one scenario
```

### 4. Composed-deployment tests — model-free component checks

`composed/` hosts every built adapter component in one Omnia runtime and runs one consolidated component smoke. It owns only WASM/WIT conformance at deployment-specific seams: WIT dispatch, async judgment-leg bridging against a failing stub model (a WIT error, never a trap), per-guest MCP references over `wasi:http`, and guest route isolation. Omnia-testkit supplies the runtime mechanics; adapter behavior and prompt assembly stay in the faster native crate tests. The single test process also avoids repeating the runtime's expensive, process-global telemetry initialization. Guests build from source on first use when artifacts are absent under `target/wasm32-wasip2/debug/`.

```bash
cargo nextest run -p composed
```

### 5. The change example — the wasm end-to-end run

[`examples/change/`](examples/change/README.md) composes the published `specify:core` guest (fetched by `cargo make core-fetch`, pinned by `SPECIFY_CORE_VERSION` in the root `Makefile.toml`) with this repo's built adapter components in one Omnia deployment and drives the full operator rhythm against the live cursor backend, hosted by the `change-example` runtime binary beside the manifest. The task builds only the three components the deployment composes (`documentation`, `intent`, `contracts`), picks a free loopback port per run, and replays the same operator inputs and seed as the native trial (`examples/change/trial.env` + `examples/change/seed/`). It ends with a deterministic completion gate (the `change-support` binary): the captured `plan status --format json` is drained, every `plan.yaml` entry is `done`, and the merged contracts baseline is non-empty and clean under the contracts validator. Still operator-invoked and per-leg ungraded — the graded trial is `cargo make eval` on rung 2. Expect a run to take tens of minutes; `GUEST_TIMEOUT_MS` (default one hour) caps each `wasi:cli/run` invocation's wall clock, and `SPECIFY_EVAL_MODEL=<model-id>` overrides the model. This is the only rung that exercises the real component seam end-to-end: WIT dispatch-by-id, mounts, and the published core, together.

```bash
cargo make core-fetch    # once per pin
cargo make change-run
cargo make change-clean
```

### 6. Consumer project — code changes through the engine

For code (not prose) iteration against a real consumer project, `cargo make adapter [adapter]` builds components with fast profile settings (LTO off, opt-level 1) into `target/wasm32-wasip2/release/<name>.wasm` in seconds instead of a full `cargo make release`. The engine's bare-name resolution is project-contained (no sibling-checkout probe), so supply the built component to the consumer project explicitly — `specify init /path/to/specify-adapters/target/wasm32-wasip2/release/<name>.wasm` mirrors it into that project's component cache. Caveat: switching between the `adapter` and `release` flavors changes the profile fingerprint and forces a rebuild; publishing is rare, so the trade is accepted.

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

`cargo llvm-cov` line/region coverage on still-live code is the safety net. The [`Makefile.toml`](Makefile.toml) has no coverage task, so run it directly, per crate, before and after a reduction:

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
