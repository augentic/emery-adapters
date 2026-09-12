# Testing

Integration-first test posture for `emery-adapters`: integration owns every publicly reachable behavior, and the unit layer is deliberately thin. Design tests against the public surfaces — the WIT contract, each adapter crate's `tests/` suite, and the public-contract live eval — never against private kernels. Read this before adding, deleting, or relocating a test.

## The rungs

Every rung runs from this repository with its own `make` tasks; the engine repository tests itself independently. There is no cross-repo command surface.

Fastest feedback first. **Every behavior is asserted on exactly one rung** — duplicating an assertion across rungs is a defect, not extra safety.

| #   | Rung               | Owns                                                                                                                                                                                      | Entry                                                    |
| --- | ------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------- |
| 1   | Native crate tests | An adapter's own extract behavior: the material it puts, its fail-closed refusals, prompt landing (scripted model)                                                                        | `cargo nextest run -p <adapter>` / `make test`           |
| 2   | Seam suites        | Every `sources/*` component under the omnia runtime, driven by a guest program over `emery:adapter/source`: instantiation, effect-free `metadata`, the reference-tool round-trip, WIT bindings lowering of evidence and the typed `error`; plus the embedded corpus of every adapter | `cargo nextest run -p <adapter> --test source`, `cargo nextest run -p emery-adapters` / `make test` |
| 3   | Graded live eval   | End-to-end spec generation over the adapter contract: prompt quality, the scorecard                                                                                                       | operator-invoked, never CI (to be recreated under `examples/`) |

Ownership boundaries: omnia's `omnia-test` crate owns the reusable doubles and the fixture pipeline — `omnia_test::guest::Scripted` (the FIFO model script over the `omnia_guest::model::Model` trait, a native-only dev-dependency of every adapter crate), the host-side `omnia_test::host::{ScriptedModel, Backends, Deployment, scratch}` the seam suites drive, and `omnia_test::build::Components`, the nested `wasm32-wasip2` build behind `crates/test-programs`; adapter `tests/extract.rs` own extract behavior; the seam suites own the component boundary and link the omnia host stack through omnia-test (never an engine crate); the live eval owns grading and the scorecard, and stays a client of the shipped `emery` binary's public contract (architecture-review T6) — it never links engine crates, and CI never runs the live rung. The engine repository proves its own side of the seam with its mock adapter; this repository proves every published component against the same runtime.

Sibling co-development: uncomment the path patches in the root `Cargo.toml` patch blocks to resolve engine crates from `../emery`. The committed tree uses git sources so CI can fetch the engine without a sibling checkout.

Testing a brand-new adapter: [authoring.md](authoring.md).

### 1. Native crate tests — the inner loop

Each adapter crate is `cdylib` + `rlib`, so its wasm-free logic links natively and tests through `sources/<name>/tests/extract.rs`. Judgment legs use `omnia_test::guest::Scripted` (a FIFO model script that records every request; `seen()` is the record to assert on — system prompt and message bodies) to assert "did my prompt edit land in the assembled text", that the material the adapter put reads as intended for a tree and for an inline value, that a malformed input is a typed refusal before any model call, and that the answered Evidence carries the required per-kind extras verbatim. What every adapter shares is asserted once elsewhere, never per adapter: the request shape — the declared tools, the schema format, the `check` flag, the workspace lend — and the claim gate's in-place repair and spent-rounds refusal are the SDK's own suite's (`emery-sdk`'s `evidence` tests); what crosses the component boundary is the seam suite's. The script is strict: a run past its end panics and a dropped script with unconsumed turns fails the test, so a scenario scripts exactly what its run consumes — a refusal that precedes the model uses `Scripted::default()`. The wasm32-only guest shim (the one `emery_sdk::source!` invocation at the root of each `src/lib.rs`, which declares the guest module itself) carries no native tests.

```bash
cargo nextest run -p documentation   # one adapter
make test                            # the whole workspace, matching CI
```

`nextest` is mandatory: each test runs in its own process, which is what lets environment-mutating suites pass. Never use bare `cargo test`.

### 2. Seam suites — `crates/test-programs`

The component rung follows omnia's `test-programs` pattern file for file. `crates/test-programs` is the fixture pipeline: its `build.rs` runs `omnia_test::build::Components` twice — `scan_packages("sources")` compiles every `sources/*` adapter, the shipped `cdylib` components themselves rather than example stand-ins, as `ADAPTER_<NAME>` with a `foreach_adapter!` arm each; `scan("crates/test-programs/programs")` compiles every `programs/<group>/<scenario>.rs` as an `[[example]]` cdylib (the stanzas below the `# Generated by build.rs` marker in its `Cargo.toml` are regenerated by the build) into a `<GROUP>_<SCENARIO>` path constant and a `foreach_<group>!` completeness macro — a nested `wasm32-wasip2` build under `OUT_DIR`, incremental after the first build. Natively the crate is those generated tables and nothing else; on `wasm32` it is the programs' shared helpers (`src/helpers.rs`: the `Caller` side of the seam and the checks a driver runs over what crosses it). The crate has no tests of its own.

A program is one guest scenario: `#![cfg(target_arch = "wasm32")]`, `omnia_guest::command!(scenario)`, an `async fn scenario()` that asserts what it observes and traps on failure. The group directory names the capability under test:

- `programs/source/` — drivers over the `emery:adapter/source` seam, universal across adapters: `roundtrip` (`metadata` checked, `extract` over the lent workspace checked, `extract` over an inline value checked) and `refused` (`extract` must fail with the Omnia error code the host passes as the program's one argument).
- `programs/probe/` — fixture adapters (`emery_sdk::source!` over a hand-written implementor) that stand in for the adapter under test, one per WIT `error` arm: `refusing` fails with `bad_request!`, `upstream` with `bad_gateway!`.

Host suites live in the crate under test, one flat file per seam, with a local `run_guest` wrapper and no shared support module:

- `sources/<name>/tests/source.rs` — `test_programs::foreach_source!()`, so every `programs/source/*` driver must have a same-named test (`source_roundtrip`, `source_refused`) in every adapter. Each test stages the adapter's minimal tree in an `omnia_test::host::scratch()` directory, describes the deployment through `omnia_test::host::Deployment` — the driver as the `wasi:cli/run` guest beside the built component registered as `test_programs::ADAPTER` under the `emery:adapter/source` link — scripts the host-side model (`omnia_test::host::ScriptedModel`, a `WasiModelCtx` double that can drive `read_doc` through the session before answering, swapped into `Backends::defaults()`), and asserts wire fidelity: `ExitStatus::SUCCESS` from the driver's own checks, the script exactly consumed, one completion per `extract` (so `metadata` made none), the compiled-in `prompts/extract.md` as the system prompt, the `read_doc` exchange returning that same embedded body, the `check` exchanges, and the lend present for the workspace leg and absent for the inline one.
- Root `tests/probe.rs` — `test_programs::foreach_probe!()`; runs the `source_refused` driver against each probe, proving each WIT `error` arm lifts back to its Omnia class with no model turn consumed.
- Root `tests/prose.rs` — `test_programs::foreach_adapter!()`, the orphan guard for the adapter set: a new `sources/<name>` fails to compile here until a test of that name exists. Each asserts the adapter's embedded corpus: the extraction prompt present and headed, survey prose absent, the 800 non-blank-line cap, the `references/emery-runtime` symlink resolved, plus the adapter's own registry facts.

This rung never asserts prompt text or extraction quality — the native suites own prompt assembly, the live eval owns quality. Put a scenario here only when the component boundary itself is the subject.

```bash
cargo nextest run -p intent --test source   # one adapter's seam suite
cargo nextest run -p emery-adapters         # the root suites: probes and corpora
```

### 3. Graded live eval

Operator-invoked, never CI: the runner spawns the sibling shipped `emery` binary over the built components, drives one `specify` per case, records typed outcomes, grades the committed spec via `emery show spec`, and writes the dated scorecard. The runner is being recreated as a root example under `examples/` now that the fixture pipeline is in place; until then the live rung has no entry.

## The two layers — minimize the unit layer

Every behavior gets a home in exactly one layer. Decide the layer **before** writing the test. The standing bias is **fewer unit tests**.

| Layer                 | Location                                                                                   | Required when                                                                                                                                         | Forbidden when                                                                   |
| --------------------- | ------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| **Kernel unit**       | `#[cfg(test)] mod tests` / sibling `tests.rs` next to the code                             | The branch is genuinely unreachable through the public API, **or** the behavior is a dense pure matrix whose integration port would inflate the suite | The behavior is reachable through the crate's public surface                     |
| **Crate integration** | `sources/<name>/tests/extract.rs`                                                          | The behavior is reachable through the library API: extract legs, material wording, fail-closed error paths                                            | The same observable behavior is already asserted elsewhere                       |
| **Seam**              | `sources/<name>/tests/source.rs`, root `tests/probe.rs` and `tests/prose.rs`, over `crates/test-programs` | The component boundary is the subject: instantiation, the seam's WIT bindings lowering, the tool streams, the compiled-in corpus                       | The behavior is reachable through the library API (it belongs to the crate suite) |

Current posture: every source adapter and `crates/test-programs` carry **zero** `src` unit tests — behavior lives entirely in `tests/` suites.

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

Test function names are identifiers, not sentences — name the *scenario* (`well_formed`, `empty_workspace`), never the outcome (`well_formed_spec_passes`, `empty_workspace_rejected`). The enclosing `tests/<area>.rs` module already names the subject — don't restate it in every `fn`. A seam test is the exception by construction: `foreach_<group>!` requires it to carry its program's full `<group>_<scenario>` name. Push the narrative into the `//` comment above the `fn`. The identifier cap is ≤ 25 characters (review-only; same rule as the engine).

## Definition of done for a reduction

- Every surviving unit test has a clear reason an agent cannot get the same signal from integration.
- `cargo llvm-cov nextest --summary-only` `TOTAL` holds on live code for every touched crate.
- `make ci` is green.
- No `pub` / `pub(crate)` widening solely for tests, and no test-only trait pairs.
