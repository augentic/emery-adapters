# Emery Adapters - Agent Instructions

This repository owns Emery's first-party **source adapters**. Each adapter is an independently versioned WebAssembly component consumed by the `emery` runtime. The contract and canonical vocabulary live in [`augentic/emery`](https://github.com/augentic/emery/blob/main/AGENTS.md); this repository owns adapter-specific extraction behavior, prose, and the graded live eval. The v1 tree (survey + extract sources, target adapters, the composition eval, the wasm examples) is archived at git tag `v1` — retrieve with `git worktree add ../emery-adapters-v1 v1`; deletion means deletion on the live branch.

## Vocabulary and boundaries

Use these roles verbatim:

- **source adapter** — input role exporting the WIT `source-adapter` world: `extract` + `metadata`. `extract` takes a typed `SourceInput` (`key`, workspace-or-value) and returns one Evidence document of typed claims — the spec IR. Required per-kind extras (`requirement`→`statement`, `criterion`→`criterion`, `example`→`replay-digest`) are one closed table, `emery_adapter::source::ClaimKind::required_extras` (A8, ADR-0009 §3): the SDK's provided `SourceAdapter::evidence` enforces it as the `check` on every candidate the backend proposes, so a miss is corrected in place, and the engine re-runs the same gate fail-closed over the WIT bindings — a claim that still lacks its extra fails the whole run typed (`bad_request`). Adapter operations fail with `emery_sdk::Error` — omnia's `omnia_guest::Error`, built with the re-exported `bad_request!` / `server_error!` / `bad_gateway!` macros; there is no adapter error type, and the WIT `error` variant is lowered and lifted inside the contract crate alone. Survey, leads, and the target axis are deleted from the WIT contract (ADR-0008; archived at `v1`).
- **Cursor plugin** — operator-facing skills distributed by the Emery repository. Adapters are not Cursor plugins.

Emery's **engine** owns lifecycle, artifact schemas, reconciliation, and synthesis. Adapters contribute extraction behavior through their WIT operations and embedded prose; they never acquire lifecycle authority or read the revision store. Preserve missing information as `[unknown]` rather than guessing; keep extracted claims platform-neutral.

## Component contract

- Each adapter ships as one component exporting the `source-adapter` world from the `emery:adapter` WIT package (owned and published by `augentic/emery`; the `emery-adapter` contract crate — named for the package — embeds it as its `source` axis module and the `emery-sdk` SDK re-exports it).
- Identity comes from the guest crate's version and published `emery:<name>@<semver>` package. Resolve-time metadata comes from the component's `metadata` operation; there is no adapter manifest.
- Keep reusable adapter logic wasm-free in library modules. Each adapter implements `emery_sdk::SourceAdapter` on a unit type; one `emery_sdk::source!` export-macro invocation at the crate root over that implementor declares the `wasm32` guest module (the adapter carries no `cfg` of its own).
- Do not commit built `.wasm` artifacts.

The root is the `emery-adapters` package — a tests-and-examples, `publish = false` package with no `src/`, whose `tests/` are the root seam suites and whose `examples/` are the live walks — over a workspace of `crates/*` and `sources/*` (documentation, intent, typescript). `crates/test-programs` is omnia's `test-programs` pattern, file for file: the guest scenario programs under `programs/<group>/<scenario>.rs` and, natively, the generated tables of every compiled component (`ADAPTER_<NAME>` for each `sources/*` adapter, `<GROUP>_<SCENARIO>` for each program) with a `foreach_<group>!` completeness macro each. Scripted doubles and the fixture pipeline come from omnia's `omnia-test` crate — `guest::Scripted` for the adapter suites, `host::{ScriptedModel, Backends, Deployment, scratch}` for the seam suites, `build::Components` for `crates/test-programs/build.rs` — a native-only dev-dependency (build-dependency for the pipeline), never an engine dependency. The adapter SDK (`emery-sdk`), the prose registry + walker (`emery-prose`; the `emit` feature is a build-dependency concern), and the `emery:adapter` contract (`emery-adapter`, re-exported by the SDK; only `crates/test-programs`' wasm32 helpers name it directly, as `emery_adapter::source::{Source, …}`) are dependencies on `augentic/emery` — published under `emery-*` names, so Rust paths are `emery_sdk::` / `emery_prose::` / `emery_adapter::` — pinned by engine git (until a release tag, RFC-77 D13) and the committed `Cargo.lock`; for sibling co-development, uncomment the path patches in the root `Cargo.toml` `[patch.crates-io]` block. `examples/` walks each adapter live over the seam, in the engine repository's example shape: one shared `examples/runtime.rs` (a command-mode `omnia::runtime!` over the Cursor model, declared as the root `runtime` example, compiling nothing in) and one `examples/<name>/` per adapter — a wasm32 driver `guest.rs` (the root `[[example]]` of the adapter's name, a `cdylib`; `#![cfg(target_arch = "wasm32")]` like the test programs), the `omnia.toml` deployment binding the built driver and component by path with the `emery:adapter/source` link and the tree it lends, and the fixture it lends. The runtime takes the deployment through omnia's `run --config` grammar, so the root's `omnia` dev-dependency enables `cli` and `link`. The examples score nothing and never spawn `emery`; the graded live-eval runner, when recreated, sits beside them as a root example and stays a **public-contract client** (architecture-review T6): it spawns the sibling shipped `emery` binary over built components and never links engine crates.

## Prose and rules

Adapter `prose/` trees are compiled into their components:

- `prose/prompts/extract.md` carries the one extraction pass; keep it below the 800 non-blank-line hard cap (the root `tests/prose.rs` enforces it on the prompt alone) and move depth into references. Its `## Worked example` JSON fence must be an Evidence document the claim gate accepts under the adapter's authority — the same suite parses it.
- References are linked, not inlined. The embed-time walker (engine `prose` crate) includes Markdown documents, follows symlinks, and fails the build on a dangling relative link.
- Shared runtime references live under `codex/references/runtime/` and reach adapters through their `prose/references/emery-runtime` symlinks; adapter-local rules live under `prose/rules/`.
- Contributor guidance belongs in `AGENTS.md`, never in the embedded corpus. Survey prompts are deleted, not ported (ADR-0008).

## Rust and testing

The external Rust baseline is the [Pragmatic Rust Guidelines](https://microsoft.github.io/rust-guidelines/guidelines/index.html), layered under the engine repo's [docs/standards/](https://github.com/augentic/emery/tree/main/docs/standards) house deltas (deltas win). Follow the workspace lint configuration in `Cargo.toml`. Identifier and comment density caps live in the engine [coding-standards.md](https://github.com/augentic/emery/blob/main/docs/standards/coding-standards.md) and are review-only. A test `fn` names the scenario (`well_formed`), never the outcome (`well_formed_spec_passes`). `make lint` runs clippy (`clippy.toml` carries the guest deny-list).

Testing is integration-first:

- An adapter's own extract behavior — the material it puts (its `SOURCE` noun, a `Prepared` note), its fail-closed refusals — belongs in its `tests/extract.rs` (scripted `omnia_test::guest::Scripted`; no credentials). What every adapter shares (the request shape, the claim gate's repair and spent-rounds refusal) is the SDK's suite's, asserted once natively, and once under the runtime over the `gated` probe. No test pins prompt phrases: the prompt is proved present and gate-consistent by the root `tests/prose.rs`, and its quality by the live eval. Do not widen public APIs solely for tests.
- The component rung (inside `make test`) is the root seam suites over `crates/test-programs`, whose `build.rs` runs one `omnia_test::build::Components` build into one `gen.rs` — `scan_packages("sources").group("adapter")` nested-builds every `sources/*` component to `wasm32-wasip2` as `ADAPTER_<NAME>` + `foreach_adapter!`; `scan("crates/test-programs/programs")` builds each `programs/<group>/<scenario>.rs` program as `<GROUP>_<SCENARIO>` + `foreach_<group>!`, syncing the `[[example]]` stanzas below the marker in its `Cargo.toml`. Programs are `#![cfg(target_arch = "wasm32")]` guests (`omnia_guest::command!(scenario)`) that assert and trap: `programs/source/extract.rs` is the one driver over the `emery:adapter/source` seam — `metadata`, then `extract` in the mode its arguments name: none (both `SourceInput` arms, each answer re-checked against the claim gate on the caller's side), `refused <code>` (`extract` must fail and lift to that Omnia class), or `echoed` (`extract` must answer the helpers' `maximal()` evidence field for field); `programs/probe/` are fixture adapters standing in for the one under test: `refusing` and `upstream` (one per WIT `error` arm), `echo` (answers `maximal()` without a model call — every claim kind, both `backing` arms, every anchor form, non-string extras), and `gated` (an inline two-document corpus over `Material::Bound` — the SDK's side of the seam with no shipped prompt underneath). Host suites are the root package's, one flat file per subject, each with its own runner and no shared support module: `tests/source.rs` invokes `foreach_adapter!()` and runs the driver's answered legs against every shipped component (registered as `test_programs::ADAPTER`), asserting only what the host sees of that component — `metadata` opened no completion, each `extract`'s system prompt is the `prompts/extract.md` this build embedded, and for intent the brief read through the mount is the turn's material; `tests/probe.rs` invokes `foreach_probe!()` and proves the error arms, the lowering of every record field, and — once, over `gated` — the SDK's request shape, reference tools, lend, and budget-spent refusal under the runtime; `tests/prose.rs` invokes `foreach_adapter!()` over every adapter's embedded corpus. So a new probe fails to compile until the root tests it, and a new `sources/<name>` fails to compile until both root suites name it; `foreach_source!` goes uninvoked, the one driver being named directly. The rung owns the boundary only — instantiation, effect-free `metadata`, both `SourceInput` arms and the workspace lend, the reference-tool round-trip, WIT bindings lowering, the compiled-in prompt — never adapter behaviour (that is `tests/extract.rs`, natively), prompt text, or quality.
- The root is a package, so `[workspace] default-members` names every member: the shared CI workflow runs `cargo nextest run` without `--workspace`, which would otherwise select the root alone and skip every adapter's suites.
- Use `cargo nextest`, not bare `cargo test`; process isolation is required by environment-mutating suites.
- The live rung is operator-invoked, never CI: a runner spawning the shipped `emery` binary over the built components, grading the committed spec via `emery show spec` into a dated scorecard. It is being recreated as a root example beside `examples/<name>/`; until then there is no `make eval`. The examples themselves are not a rung: `make example <name>` walks one adapter's `extract` live through the Cursor backend (`cursor-sdk-bridge` on `PATH`, `CURSOR_API_KEY`) and prints the claims, asserting nothing — see [`examples/README.md`](examples/README.md).

Read [`docs/testing.md`](docs/testing.md) before adding, deleting, or relocating tests.

## Commands

Run from the repository root, driven by `make` ([`Makefile`](./Makefile) → mise):

```bash
make check                # fmt, lint (clippy), nextest, doctests, docs
make ci                   # full gate, including vet and deny
cargo nextest run -p NAME # one adapter's native extract suite
cargo nextest run -p emery-adapters # the root seam suites: every component, the probes, the corpora
make adapter NAME         # fast development component build
make example NAME         # live walk of one adapter: build its component and driver, run the example runtime (needs the Cursor bridge)
make release              # release-build every adapter component (excludes emery-adapters, test-programs)
make publish NAME         # push one built component to its exact GHCR tag (Publish Release / local breakout)
make sweep                # drop target/ artifacts untouched for a week (cargo-sweep); cargo never collects them itself
```

Run `make ci` before committing. If it cannot run, report exactly which narrower checks ran and why the full gate was unavailable.

## Area-specific guidance

- Human contributor setup (toolchain, layout, pin, publishing): [`CONTRIBUTING.md`](CONTRIBUTING.md)
- Creating a source adapter (anatomy, walkthrough): [`docs/authoring.md`](docs/authoring.md)
- Test ownership: [`docs/testing.md`](docs/testing.md)
