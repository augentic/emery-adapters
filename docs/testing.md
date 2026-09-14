# Testing

Integration-first test posture for `emery-adapters`: integration owns every publicly reachable behavior, and the unit layer is deliberately thin. Design tests against the public surfaces — the WIT contract, each adapter crate's `tests/` suite, and the public-contract live eval — never against private kernels. Read this before adding, deleting, or relocating a test.

## The rungs

Every rung runs from this repository with its own `make` tasks; the engine repository tests itself independently. There is no cross-repo command surface.

Fastest feedback first. **Every behavior is asserted on exactly one rung** — duplicating an assertion across rungs is a defect, not extra safety.

| #   | Rung               | Owns                                                                                                                                                                                      | Entry                                                    |
| --- | ------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------- |
| 1   | Native crate tests | An adapter's own extract behavior: the material it puts, its fail-closed refusals (scripted model)                                                                                        | `cargo nextest run -p <adapter>` / `make test`           |
| 2   | Seam suites        | Every `sources/*` component under the omnia runtime, driven by a guest program over `emery:adapter/source`: instantiation, effect-free `metadata`, the compiled-in prompt; over fixture adapters, the SDK's side of the seam, the WIT `error` arms, and the bindings' lowering of every record field; plus the embedded corpus of every adapter | `cargo nextest run -p emery-adapters` / `make test` |
| 3   | Graded live eval   | End-to-end spec generation over the adapter contract: prompt quality, the scorecard                                                                                                       | operator-invoked, never CI (to be recreated as a root example) |

The live examples (`cargo build --example <name> --target wasm32-wasip2 --release`, then `emery specify --config examples/<name>/emery.toml`) are not a rung: each walks one adapter through the shipped `emery` binary and the Cursor backend — extract, synthesis, the committed revision — asserting nothing. They are for watching a prompt change land in a specification, not for proving it — see [examples/README.md](../examples/README.md).

Ownership boundaries: omnia's `omnia-test` crate owns the reusable doubles and the fixture pipeline — `omnia_test::guest::Scripted` (the FIFO model script over the `omnia_guest::model::Model` trait, a native-only dev-dependency of every adapter crate), the host-side `omnia_test::host::{ScriptedModel, Backends, Deployment, scratch}` the seam suites drive, and `omnia_test::build::Components`, the nested `wasm32-wasip2` build behind `crates/test-programs`; adapter `tests/extract.rs` own extract behavior; the seam suites own the component boundary and link the omnia host stack through omnia-test (never an engine crate); the live eval owns grading and the scorecard, and stays a client of the shipped `emery` binary's public contract (architecture-review T6) — it never links engine crates, and CI never runs the live rung. The engine repository proves its own side of the seam with its mock adapter; this repository proves every published component against the same runtime.

Sibling co-development: uncomment the path patches in the root `Cargo.toml` patch blocks to resolve engine crates from `../emery`. The committed tree uses git sources so CI can fetch the engine without a sibling checkout.

Testing a brand-new adapter: [authoring.md](authoring.md).

### 1. Native crate tests — the inner loop

Each adapter crate is `cdylib` + `rlib`, so its wasm-free logic links natively and tests through `sources/<name>/tests/extract.rs`. Judgment legs use `omnia_test::guest::Scripted` (a FIFO model script that records every request; `seen()` is the record to assert on — system prompt and message bodies) to assert what the adapter itself decides: that its `SOURCE` noun names the bound tree in the turn, that a `Material::Prepared` note reads as intended (intent puts the brief it read from a one-file tree), and that a malformed input is a typed refusal before any model call. An adapter with no logic of its own (documentation, typescript) carries one test — the noun landing. Nothing here pins prompt phrases or the SDK's brief text: a wording edit is not a behavior change, and the SDK's turn is asserted byte-exact in its own suite. What every adapter shares is asserted once elsewhere, never per adapter: the request shape — the declared tools, the schema format, the `check` flag, the workspace lend — and the claim gate's in-place repair and spent-rounds refusal are the SDK's own suite's (`emery-sdk`'s `evidence` tests), and once more under the runtime over the `gated` probe (rung 2); what crosses the component boundary is the seam suite's. The script is strict: a run past its end panics and a dropped script with unconsumed turns fails the test, so a scenario scripts exactly what its run consumes — a refusal that precedes the model uses `Scripted::default()`. The wasm32-only guest shim (the one `emery_sdk::source!` invocation at the root of each `src/lib.rs`, which declares the guest module itself) carries no native tests.

```bash
cargo nextest run -p documentation   # one adapter
make test                            # the whole workspace, matching CI
```

`nextest` is mandatory: each test runs in its own process, which is what lets environment-mutating suites pass. Never use bare `cargo test`.

The root is a package, so a `cargo nextest run` from the root without `--workspace` selects the root alone and never executes an adapter's suite; `make test` and the shared CI workflow pass `--workspace`, and so must any invocation meant to match them.

The guest side — every program under `crates/test-programs/programs/` and the adapters' export shims — is `cfg(target_arch = "wasm32")`, so `make lint` compiles it to nothing. Lint it for `wasm32-wasip2`, where `clippy.toml`'s guest deny-list applies, with `cargo clippy --workspace --exclude emery-adapters --lib --examples --target wasm32-wasip2 -- -D warnings` (the root package, native tests alone, is excluded); no gate runs it today.

### 2. Seam suites — `crates/test-programs`

The component rung follows omnia's `test-programs` pattern file for file. `crates/test-programs` is the fixture pipeline: its `build.rs` runs one `omnia_test::build::Components` build drawing from two sources — `scan_packages("sources").group("adapter")` compiles every `sources/*` adapter, the shipped `cdylib` components themselves rather than example stand-ins, as `ADAPTER_<NAME>` with a `foreach_adapter!` arm each; `scan("crates/test-programs/programs")` compiles every `programs/<group>/<scenario>.rs` as an `[[example]]` cdylib (the stanzas below the `# Generated by build.rs` marker in its `Cargo.toml` are regenerated by the build) into a `<GROUP>_<SCENARIO>` path constant and a `foreach_<group>!` completeness macro — a nested `wasm32-wasip2` build under `OUT_DIR`, incremental after the first build, written as one `gen.rs`. Natively the crate is that generated table and nothing else; on `wasm32` it is the programs' shared helpers (`src/helpers.rs`: the `Caller` side of the seam, the checks a driver runs over what crosses it, and `maximal()`, the evidence with every record field populated). The crate has no tests of its own.

A program is one guest scenario: `#![cfg(target_arch = "wasm32")]`, `omnia_guest::command!(scenario)`, an `async fn scenario()` that asserts what it observes and traps on failure. The group directory names the capability under test:

- `programs/source/` — the one driver over the `emery:adapter/source` seam, `extract`: `metadata` checked, then `extract` in the mode the host names as its arguments. None: over the lent workspace and over an inline value, each answer re-checked against the contract's claim gate on the caller's side. `refused <code>`: over the lent workspace, expected to fail and lift to that Omnia error class. `echoed`: over an inline value, expected to answer `maximal()` field for field.
- `programs/probe/` — fixture adapters (`emery_sdk::source!` over a hand-written implementor) that stand in for the adapter under test: `refusing` fails with `bad_request!` and `upstream` with `bad_gateway!`, one per WIT `error` arm; `echo` answers `maximal()` without a model call; `gated` is an inline two-document corpus over `Material::Bound` — nothing of its own, so what it shows is the SDK's.

Host suites are the root package's, one flat file per subject over one shared runner. `tests/support/mod.rs` is the scenario support both seam suites declare as `mod support;`: its `run(adapter, project, args, model)` describes the deployment through `omnia_test::host::Deployment` — the driver as the `wasi:cli/run` guest beside the component under test registered as `test_programs::ADAPTER` under the `emery:adapter/source` link, the project mounted read-only as `.`, the driver's mode as its arguments, the scripted host-side model (`omnia_test::host::ScriptedModel`, a `WasiModelCtx` double swapped into `Backends::defaults()`) — and requires `ExitStatus::SUCCESS` from the driver's own checks and the script exactly consumed, returning the model's record for the suite to assert on:

- Root `tests/source.rs` — `test_programs::foreach_adapter!()`, so every shipped component has a same-named test. Each stages the adapter's minimal tree in an `omnia_test::host::scratch()` directory, scripts one gate-valid claims-only answer per `extract` (the SDK stamps the adapter's constant), and asserts only what the host sees of *this* component: one completion per `extract` (so `metadata` made none), and the compiled-in `prompts/extract.md` as each system prompt. An adapter adds only what its component alone shows the host — intent asserts the brief it read through the mount is the first turn's material, the one adapter-specific behavior that runs inside the guest.
- Root `tests/probe.rs` — `test_programs::foreach_probe!()`. `probe_refusing` and `probe_upstream` run the driver's `refused` mode, proving each WIT `error` arm lifts back to its Omnia class with no model turn consumed. `probe_echo` runs `echoed`, proving every field of the contract's records — each claim kind, both `backing` arms, every anchor form, extras that are objects, numbers, lists, and null — crosses the bindings as it left the adapter (extras ride as canonical JSON text and are parsed back). `probe_gated` runs the answered legs with `list_docs` and `read_doc` driven through the session, and asserts the SDK's side of the seam once under the runtime: the embedded prompt as the system, the reference tools declared, `check` set, the source noun and key in the turn, the lend present for the workspace leg and absent for the inline one, the corpus answering both tools, and each candidate accepted by the guest's `check`; `gated_spent` runs `refused bad_request` over a candidate the gate rejects, proving the backend's spent budget crosses as `bad_request` with the correction naming the finding.
- Root `tests/prose.rs` — `test_programs::foreach_adapter!()`, so every shipped adapter's corpus is checked (a new `sources/<name>` fails to compile here until a test of that name exists). Each asserts the two corpus rules nothing else enforces: `prompts/extract.md` stays under the 800 non-blank-line cap, and its `## Worked example` JSON fence parses as the SDK's `Answer` and passes the claim gate under the adapter's constant — the one machine-checkable part of a prompt, so a prompt edit cannot teach the model a shape the adapter would refuse, and an engine pin that moves the contract (the required extras, the id grammar) fails here rather than in the live eval's repair rounds. Nothing else about the corpus is asserted: reference presence is the embed-time walker's (a dangling link or symlink fails the build), and the prompt each component embeds is proved under the runtime by `tests/source.rs`.

`foreach_source!` is generated but goes uninvoked: the one `source` program is the driver both seam suites name directly, so there is no per-adapter test for a completeness macro to guard.

This rung never asserts prompt text, adapter behaviour, or extraction quality — the native suites own the adapter's material and refusals, the SDK's suite owns what every adapter shares, the live eval owns quality. Put a scenario here only when the component boundary itself is the subject.

```bash
cargo nextest run -p emery-adapters                 # the root suites: every component, the probes, the corpora
cargo nextest run -p emery-adapters --test source   # the shipped components alone
```

### 3. Graded live eval

Operator-invoked, never CI: the runner spawns the sibling shipped `emery` binary over the built components, drives one `specify` per case, records typed outcomes, grades the committed spec via `emery show spec`, and writes the dated scorecard. The runner is being recreated as a root example beside the live examples now that the fixture pipeline is in place; until then the live rung has no entry.

## Adding a test

Every behavior gets a home in exactly one place. Decide the home **before** writing the test:

| Home                  | Location                                                                        | When                                                                                                                           |
| --------------------- | ------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| **Crate integration** | `sources/<name>/tests/extract.rs`                                               | The adapter itself decides it: material wording, a `Prepared` note, a fail-closed refusal                                       |
| **Seam**              | root `tests/source.rs`, `tests/probe.rs`, `tests/prose.rs`, over `crates/test-programs` | The component boundary or the corpus is the subject: instantiation, the bindings' lowering, the tool streams, the embedded prompt |
| **Kernel unit**       | `#[cfg(test)] mod tests` next to the code                                       | The branch is genuinely unreachable through the public API — with a one-line comment saying why; there are none today          |

Never add a test for what the SDK or the contract already proves (the request shape, the gate's repair loop, `Evidence` deserialization), and never pin a prompt phrase — prompt quality is the live eval's. Do not widen `pub` surface solely for a test.

Coverage is advisory here, not a gate: adapter code is a few lines per crate, and `make cov` (`cargo llvm-cov nextest --workspace`) instruments the native side alone — the components the seam rung runs inside wasmtime go uncounted.

## Test naming

Test function names are identifiers, not sentences — name the *scenario* (`bound_tree`, `not_one_file`), never the outcome (`bound_tree_named`, `not_one_file_rejected`). The enclosing `tests/<area>.rs` module already names the subject — don't restate it in every `fn`. A test a `foreach_<group>!` macro guards is the exception by construction: it carries its program's full `<group>_<scenario>` name (`probe_echo`) or its adapter's (`intent`). Push the narrative into the `//` comment above the `fn`.
