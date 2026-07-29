# Emery Adapters - Agent Instructions

This repository owns Emery's first-party source and target adapters. Each adapter is an independently versioned WebAssembly component consumed by the `emery` runtime. The workflow contract and canonical vocabulary live in [`augentic/emery`](https://github.com/augentic/emery/blob/main/AGENTS.md); this repository owns adapter-specific analysis, generation, enrichment, review behavior, and engineering standards.

## Vocabulary and boundaries

Use these roles verbatim:

- **source adapter** — input role exporting `survey` and `extract`; emits leads and Evidence.
- **target adapter** — output role exporting `guidance`, `build`, and `merge`; consumes Emery artifacts and writes target outputs.
- **Cursor plugin** — operator-facing skills and rules distributed by the Emery repository. Adapters are not Cursor plugins.

Emery's **engine** owns lifecycle, artifact schemas, synthesis, and deterministic state transitions. Adapters contribute specialist behavior through their WIT operations and embedded prose. Do not move plan/slice orchestration into adapter prompts or infer new lifecycle states here. Reserve **workflow** for the operator loop (`plan → execute → finalize`).

Artifacts outrank source behavior. Preserve missing information as `[unknown]` rather than guessing. Keep behavioral requirements platform-neutral in `spec.md`; target-specific implementation choices belong in target guidance, build prompts, references, and generated code.

## Component contract

- Each adapter ships as one component exporting exactly one axis world from the `emery:adapter` WIT package (owned and published by `augentic/emery`; the `adapter` SDK embeds it).
- Identity comes from the guest crate's version and published `emery:<name>@<semver>` package. Resolve-time metadata comes from the component's WIT operation; there is no adapter manifest.
- Keep reusable adapter logic wasm-free in library modules. Each adapter implements its axis operations trait (`adapter::Source` / `adapter::Target`) on a unit type; the `wasm32` guest module is a single `adapter::source!` / `adapter::target!` export-macro invocation over that implementor.
- Do not commit built `.wasm` artifacts.
- Adapter names must remain unique across the source and target axes.

The root is a virtual workspace of `sources/*`, `targets/*`, and `examples/eval` (the live composition package). The wasm examples under `examples/wasm/` are fixture + operator scripts only — they drive the sibling `augentic/emery` shipped binary against built adapter components (no in-tree Omnia host). The adapter SDK (`emery-adapter`), the native host (`emery-native`), and the lab-only probe library (`emery-probe`, `client` feature — no mock, no binary) are git dependencies on `augentic/emery`, not local crates — the packages publish under `emery-*` names but keep their short lib names, so Rust paths stay `adapter::` / `native::` / `probe::` — pinned by engine release tag (`tag = "vX.Y.Z"`, RFC-77 D13) and the committed `Cargo.lock`; for sibling co-development, uncomment the `[patch."https://github.com/augentic/emery.git"]` block in the root `Cargo.toml` to resolve them from the `../emery` checkout. The `eval` package at `examples/eval/` owns the first-party catalog declaration (in `src/main.rs`), its `cases/` root, and the composition binary, delegating dispatch and the Cursor backend to the shared `probe::client` (the engine `probe` crate's `client` feature); engine `native` supplies catalog machinery and command execution, engine `probe` supplies the typed case runner, telemetry, and grading. Neither can merge into engine `probe`: the first-party catalog and `cases/` live here. For sibling co-development the committed path patch in the root `Cargo.toml` resolves the engine crates from the `../emery` checkout.

## Prose and rules

Adapter `prose/` trees are compiled into their components:

- Parent prompts orchestrate an operation and load phase prompts by relative link. Keep them below roughly 150 non-blank lines.
- Phase prompts carry one operational phase. Prefer splitting or moving depth into references before approaching the 800-line hard cap.
- References are linked, not inlined. The embed-time walker (engine `prose` crate) includes Markdown documents, follows symlinks, and fails the build on a dangling relative link; keep links resolvable.
- Engineering standards are Markdown rules under `codex/rules/` and adapter-local `prose/rules/` overlays. Preserve stable rule IDs and namespace ownership.
- Contributor guidance belongs in `AGENTS.md`, not in the embedded engineering-rule corpus.

Generation behavior belongs in adapter prompts and wasm-free adapter logic, not in consumer-facing skills. Shared runtime references live under `codex/references/runtime/` and are exposed to adapters through their `prose/references/emery-runtime` symlinks.

## Rust and testing

Follow the workspace lint configuration in `Cargo.toml`. Prefer strong domain types, explicit errors, small functions, and comments that explain current invariants rather than history.

Testing is integration-first:

- Publicly reachable adapter behavior belongs in each adapter crate's `tests/` suite.
- Keep `src` unit tests only for genuinely unreachable defensive branches or dense pure matrices that are materially cheaper in-process.
- Do not widen public APIs solely for tests.
- Use `cargo nextest`, not bare `cargo test`, for native workspace tests; process isolation is required by CWD- and environment-mutating suites.
- Adapter native tests own operation behavior; the eval composition example (over engine `probe`) owns cross-phase integration and prompt quality; the operator-invoked wasm examples own WASM/WIT conformance. Do not duplicate the same assertion across rungs.
- The live rungs are operator-invoked, never CI: `cargo make eval <id> --restart` (one eval case over its retained `sandbox/<id>/` tree — build cases like `contracts-design` for fast prompt iteration, workflow cases like `orders-contracts` and `omnia-r9k` for the full source-to-target rhythm; bare `cargo make eval` lists them) and `cargo make wasm-contracts` / `cargo make wasm-omnia-r9k` (sibling `emery` binary plus built adapter components over the real component seam). The `omnia-r9k` case (and `wasm-omnia-r9k`) shallow-clones its `UNLICENSED` upstream into the case's gitignored `fixture/` cache on first run. Day-to-day eval loop: [`README.md`](README.md); eval case catalog: [`examples/eval/`](examples/eval/).

Read [`docs/testing.md`](docs/testing.md) before adding, deleting, or relocating tests.

## Commands

Run from the repository root:

```bash
cargo make check          # fmt, clippy, nextest, doctests, docs
cargo make ci             # full gate, including vet and deny
cargo nextest run -p NAME # focused adapter tests
cargo make adapter NAME   # fast development component build
cargo make release        # release-build every component
cargo make publish NAME   # push one built component to its exact GHCR tag (Publish Release / local breakout)
cargo make lab -- ARGS # any emery verb through the native lab shim
cargo make eval [id] [--restart] [--until RUNG]  # one live eval case; bare lists them (operator-invoked)
cargo make wasm-contracts     # contracts wasm example (operator-invoked)
cargo make wasm-omnia-r9k     # omnia-r9k wasm example (operator-invoked)
```

Run `cargo make ci` before committing. If it cannot run, report exactly which narrower checks ran and why the full gate was unavailable.

## Area-specific guidance

- Human contributor setup (toolchain, layout, prompts, pin, publishing): [`CONTRIBUTING.md`](CONTRIBUTING.md)
- Creating an adapter (anatomy, walkthrough, harness wiring): [`docs/authoring.md`](docs/authoring.md)
- Vectis: [`targets/vectis/AGENTS.md`](targets/vectis/AGENTS.md)
- Rule catalog and namespace model: [`codex/rules/README.md`](codex/rules/README.md)
- Test ownership and five-rung map: [`docs/testing.md`](docs/testing.md); live eval how-to: [`README.md`](README.md)
