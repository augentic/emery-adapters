# Emery Adapters - Agent Instructions

This repository owns Emery's first-party **source adapters**. Each adapter is an independently versioned WebAssembly component consumed by the `emery` runtime. The contract and canonical vocabulary live in [`augentic/emery`](https://github.com/augentic/emery/blob/main/AGENTS.md); this repository owns adapter-specific extraction behavior, prose, and the graded live eval. The v1 tree (survey + extract sources, target adapters, the composition eval, the wasm examples) is archived at git tag `v1` — retrieve with `git worktree add ../emery-adapters-v1 v1`; deletion means deletion on the live branch.

## Vocabulary and boundaries

Use these roles verbatim:

- **source adapter** — input role exporting the WIT `source-adapter` world: `extract` + `metadata`. `extract` takes a typed `SourceInput` (`key`, workspace-or-value) and returns one Evidence document of typed claims — the spec IR. Required per-kind extras (`requirement`→`statement`, `criterion`→`criterion`, `example`→`replay-digest`) are enforced fail-closed engine-side (A8, ADR-0009 §3); a claim missing its extra fails the whole run typed. Survey, leads, and the target axis are deleted from the WIT contract (ADR-0008; archived at `v1`).
- **Cursor plugin** — operator-facing skills distributed by the Emery repository. Adapters are not Cursor plugins.

Emery's **engine** owns lifecycle, artifact schemas, reconciliation, and synthesis. Adapters contribute extraction behavior through their WIT operations and embedded prose; they never acquire lifecycle authority or read the output home. Preserve missing information as `[unknown]` rather than guessing; keep extracted claims platform-neutral.

## Component contract

- Each adapter ships as one component exporting the `source-adapter` world from the `emery:adapter` WIT package (owned and published by `augentic/emery`; the `emery-source` contract crate embeds it and the `emery-adapter` SDK re-exports it).
- Identity comes from the guest crate's version and published `emery:<name>@<semver>` package. Resolve-time metadata comes from the component's `metadata` operation; there is no adapter manifest.
- Keep reusable adapter logic wasm-free in library modules. Each adapter implements `emery_adapter::SourceAdapter` on a unit type; the `wasm32` guest module is a single `emery_adapter::source!` export-macro invocation over that implementor.
- Do not commit built `.wasm` artifacts.

The root is a virtual workspace of `sources/*` (documentation, intent, typescript) plus `examples/eval` (the graded live-eval runner), `examples/caller` (the guest-only conformance caller), and `examples/conformance` (the native component-conformance harness and suite). Scripted doubles come from omnia's `omnia-test` crate — `guest::Scripted` for the adapter suites, `host::{ScriptedModel, Backends, Deployment}` for the conformance harness — a native-only dev-dependency, never an engine dependency. The adapter SDK (`emery-adapter`), the prose registry + walker (`emery-prose`; the `emit` feature is a build-dependency concern), and the source contract (`emery-source`, re-exported by the SDK; only `examples/caller` names it directly) are dependencies on `augentic/emery` — published under `emery-*` names, so Rust paths are `emery_adapter::` / `emery_prose::` / `emery_source::` — pinned by engine git (until a release tag, RFC-77 D13) and the committed `Cargo.lock`; for sibling co-development, uncomment the path patches in the root `Cargo.toml` `[patch.crates-io]` block. The eval runner is a **public-contract client** (architecture-review T6): it spawns the sibling shipped `emery` binary over built components and never links engine crates.

## Prose and rules

Adapter `prose/` trees are compiled into their components:

- `prose/prompts/extract.md` carries the one extraction pass; keep it below the 800-line hard cap and move depth into references.
- References are linked, not inlined. The embed-time walker (engine `prose` crate) includes Markdown documents, follows symlinks, and fails the build on a dangling relative link.
- Shared runtime references live under `codex/references/runtime/` and reach adapters through their `prose/references/emery-runtime` symlinks; adapter-local rules live under `prose/rules/`.
- Contributor guidance belongs in `AGENTS.md`, never in the embedded corpus. Survey prompts are deleted, not ported (ADR-0008).

## Rust and testing

The external Rust baseline is the [Pragmatic Rust Guidelines](https://microsoft.github.io/rust-guidelines/guidelines/index.html), layered under the engine repo's [docs/standards/](https://github.com/augentic/emery/tree/main/docs/standards) house deltas (deltas win). Follow the workspace lint configuration in `Cargo.toml`. Identifier and comment density caps live in the engine [coding-standards.md](https://github.com/augentic/emery/blob/main/docs/standards/coding-standards.md) and are review-only. `make lint` runs clippy (`clippy.toml` carries the guest deny-list).

Testing is integration-first:

- Publicly reachable adapter behavior belongs in each adapter crate's `tests/` suite (scripted models; no credentials). Do not widen public APIs solely for tests.
- The component rung (`examples/conformance`, inside `make test`) instantiates every `sources/*` component under the omnia runtime with a scripted host-side model, driven by the `examples/caller` guest over the `emery:adapter/source` seam; its build script is one `omnia_test::build::Components` call (`scan_packages("sources")` + `extra_package("caller")`) that nested-builds the components to `wasm32-wasip2` and generates `foreach_source!`, so a new `sources/<name>` fails to compile until it has a conformance test. It owns the boundary only — instantiation, effect-free `metadata`, the reference-tool round-trip, wire lowering — never prompt text or quality.
- Use `cargo nextest`, not bare `cargo test`; process isolation is required by environment-mutating suites.
- The live rung is operator-invoked, never CI: `make eval [id]` spawns the shipped `emery` binary over the built components, wall-clocks one `specify` → committed generation, records typed outcomes, grades the committed spec via `emery show spec`, and writes the dated scorecard (`sandbox/scorecard.md`). The `omnia-r9k` case shallow-clones its `UNLICENSED` upstream into the gitignored `cases/omnia-r9k/fixture/` cache on first run. See [`examples/eval/README.md`](examples/eval/README.md).

Read [`docs/testing.md`](docs/testing.md) before adding, deleting, or relocating tests.

## Commands

Run from the repository root, driven by `make` ([`Makefile`](./Makefile) → mise):

```bash
make check                # fmt, lint (clippy), nextest, doctests, docs
make ci                   # full gate, including vet and deny
cargo nextest run -p NAME # focused adapter tests
cargo nextest run -p conformance # the component rung alone
make adapter NAME         # fast development component build
make release              # release-build every adapter component (excludes eval, caller, conformance)
make publish NAME         # push one built component to its exact GHCR tag (Publish Release / local breakout)
make eval [id]            # graded live eval over the public contract (operator-invoked, never CI)
```

Run `make ci` before committing. If it cannot run, report exactly which narrower checks ran and why the full gate was unavailable.

## Area-specific guidance

- Human contributor setup (toolchain, layout, pin, publishing): [`CONTRIBUTING.md`](CONTRIBUTING.md)
- Creating a source adapter (anatomy, walkthrough): [`docs/authoring.md`](docs/authoring.md)
- Test ownership: [`docs/testing.md`](docs/testing.md); live eval how-to: [`examples/eval/README.md`](examples/eval/README.md)
