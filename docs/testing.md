# Testing

Integration-first test posture for `emery-adapters`: integration owns every publicly reachable behavior, and the unit layer is deliberately thin. Design tests against the public surfaces — the WIT contract, each adapter crate's `tests/` suite, and the public-contract live eval — never against private kernels. Read this before adding or deleting a test.

## The rungs

Every rung runs from this repository with its own `make` tasks; the engine repository tests itself independently. There is no cross-repo command surface.

Fastest feedback first. **Every behavior is asserted on exactly one rung** — duplicating an assertion across rungs is a defect, not extra safety.

| #   | Rung                  | Owns                                                     | Entry                                                |
| --- | --------------------- | -------------------------------------------------------- | ---------------------------------------------------- |
| 1   | Native crate tests    | Extract behavior, prompt assembly (scripted model)       | `cargo nextest run -p <adapter>` / `make test` |
| 2   | Component conformance | Each `sources/*` component instantiated under the omnia runtime: instantiation, effect-free `metadata`, the reference-tool round-trip, wire lowering of evidence and the typed `error` | `cargo nextest run -p conformance` / `make test` |
| 3   | Eval runner tests     | Grading kernels, envelope parsing, scorecard green line  | `cargo nextest run -p eval`                          |
| 4   | Graded live eval      | End-to-end spec generation over the adapter contract: prompt quality, the product.md numbers, the scorecard | `make eval [id]` (operator-invoked, never CI) |

Ownership boundaries: the engine's unpublished `emery-testkit` (a dev-only git dependency, like the SDK pin) owns reusable model test mechanics; adapter `tests/` own extract behavior; the conformance crate owns the component boundary and links the omnia host stack (never an engine crate); the eval runner owns grading and the scorecard, and stays a client of the shipped `emery` binary's public contract (architecture-review T6) — it never links engine crates, and CI never runs the live rung. The engine repository proves its own side of the seam with its mock adapter; this repository proves every published component against the same runtime.

Sibling co-development: uncomment the path patches in the root `Cargo.toml` patch blocks to resolve engine crates from `../emery`. The committed tree uses git sources so CI can fetch the engine without a sibling checkout.

Testing a brand-new adapter: [authoring.md](authoring.md).

### 1. Native crate tests — the inner loop

Each adapter crate is `cdylib` + `rlib`, so its wasm-free logic links natively and tests through `sources/<name>/tests/<area>.rs`. Judgment legs use `emery_testkit::Scripted` (a FIFO model script that records every request) to assert "did my prompt edit land in the assembled text" and that the answered Evidence carries the required per-kind extras verbatim; adapter crates must not duplicate that model machinery. The wasm32-only guest shims (inline `mod guest` in each `src/lib.rs`) are single `emery_adapter::source!` invocations and carry no native tests.

```bash
cargo nextest run -p documentation   # one adapter
make test                            # the whole workspace, matching CI
```

`nextest` is mandatory: each test runs in its own process, which is what lets environment-mutating suites pass. Never use bare `cargo test`.

### 2. Component conformance

`examples/conformance` is the omnia-shaped component rung (its `test-programs` pattern: a nested `wasm32-wasip2` build under `OUT_DIR`, generated path constants, a completeness macro). Its build script compiles the caller guest (`examples/caller`) and every `sources/*` adapter to components — incremental after the first build — and generates `foreach_source!`, so a new `sources/<name>` fails to compile `tests/conformance.rs` until it has a same-named test. Each test stages a minimal source tree, deploys the caller as the `wasi:cli/run` guest beside one adapter under the `emery:adapter/source` seam, scripts the host-side model (`conformance::ScriptedModel`, a `WasiModelCtx` double that can drive `read_doc` through the session before answering), and asserts: exit `0` from the caller's own wire-shape checks, exactly one completion (so `metadata` made no model call), the compiled-in `prompts/extract.md` as the system prompt, and the `read_doc` exchange returning that same embedded body. One shared negative proves a fail-closed refusal crosses the seam as the typed WIT `error` variant before any model call.

This rung never asserts prompt text or extraction quality — the native suites own prompt assembly, the live eval owns quality. Put a scenario here only when the component boundary itself is the subject.

```bash
cargo nextest run -p conformance   # the component rung alone
```

### 3. Eval runner tests

The runner's grading kernels (CC-05/CC-06 mechanical checks, envelope parsing, the scorecard green line) are pure and tested in `examples/eval/tests/`. They run in CI; the live model never does.

### 4. Graded live eval

Operator-invoked, never CI: the runner spawns the sibling shipped `emery` binary over the built components, drives one `specify` per case, records typed outcomes, grades the committed spec via `emery show spec`, and writes the dated scorecard. How to run: [`examples/eval/README.md`](../examples/eval/README.md).

```bash
make eval               # every case
make eval orders-docs   # one case
```

## The two layers — minimize the unit layer

Every behavior gets a home in exactly one layer. Decide the layer **before** writing the test. The standing bias is **fewer unit tests**.

| Layer                 | Location                                                       | Required when                                                                                                             | Forbidden when                                                                       |
| --------------------- | -------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| **Kernel unit**       | `#[cfg(test)] mod tests` / sibling `tests.rs` next to the code | The branch is genuinely unreachable through the public API, **or** the behavior is a dense pure matrix whose integration port would inflate the suite | The behavior is reachable through the crate's public surface                          |
| **Crate integration** | `<adapter>/tests/` (one auto-discovered binary per area)       | The behavior is reachable through the library API: extract legs, prompt assembly, fail-closed error paths                 | The same observable behavior is already asserted elsewhere                            |
| **Component conformance** | `examples/conformance/tests/conformance.rs`                 | The component boundary is the subject: instantiation, the seam's wire lowering, the tool streams                          | The behavior is reachable through the library API (it belongs to the crate suite)      |

Current posture: every source adapter, the caller, the conformance harness, and the eval runner carry **zero** `src` unit tests — behavior lives entirely in `tests/` suites.

## Triage rules

Applied to every existing `#[cfg(test)]` / `tests.rs`:

- **Delete** — the observable behavior is already asserted by an integration test, or it is tautological, mock-heavy, or an internal snapshot that gives an agent no boundary signal.
- **Collapse (stay unit)** — a dense pure `(input → output)` matrix becomes one table-driven `#[test]` with a block per case. Coverage-neutral by construction.
- **Re-home** — behavior reachable through the library lands in the crate's `tests/` tree.
- **Keep** — a genuinely unreachable defensive branch, with a one-line comment saying why an agent cannot get the same signal from integration.

## Coverage is the brake on deletion

`cargo llvm-cov` line/region coverage on still-live code is the safety net. Run the `cov-crate` task, per crate, before and after a reduction:

```bash
CRATE=documentation make cov-crate   # cargo llvm-cov nextest -p documentation --summary-only
```

A `TOTAL` line/region drop on still-live code means real coverage was lost: backfill with an integration assertion (preferred) or revert that specific deletion. A pure collapse of redundant cases is coverage-neutral.

## Test naming

Test function names are identifiers, not sentences. The enclosing `tests/<area>.rs` module already names the subject — don't restate it in every `fn`. Push the narrative into the test body or a `//` comment above the `fn`.

## Definition of done for a reduction

- Every surviving unit test has a clear reason an agent cannot get the same signal from integration.
- `cargo llvm-cov nextest --summary-only` `TOTAL` holds on live code for every touched crate.
- `make ci` is green.
- No `pub` / `pub(crate)` widening solely for tests, and no test-only trait pairs.
