# Specify Adapters - Agent Instructions

This repository owns Specify's first-party source and target adapters. Each adapter is an independently versioned WebAssembly component consumed by the `specify` runtime. The workflow contract and canonical vocabulary live in [`augentic/specify`](https://github.com/augentic/specify/blob/main/AGENTS.md); this repository owns adapter-specific analysis, generation, enrichment, review behavior, and engineering standards.

## Vocabulary and boundaries

Use these roles verbatim:

- **source adapter** — input role exporting `survey` and `extract`; emits leads and Evidence.
- **target adapter** — output role exporting `guidance`, `build`, and `merge`; consumes Specify artifacts and writes target outputs.
- **Cursor plugin** — operator-facing skills and rules distributed by the Specify repository. Adapters are not Cursor plugins.

Specify owns workflow lifecycle, artifact schemas, synthesis, and deterministic state transitions. Adapters contribute specialist behavior through their WIT operations and embedded prose. Do not move workflow orchestration into adapter prompts or infer new lifecycle states here.

Artifacts outrank source behavior. Preserve missing information as `[unknown]` rather than guessing. Keep behavioral requirements platform-neutral in `spec.md`; target-specific implementation choices belong in target guidance, build prompts, references, and generated code.

## Component contract

- Each adapter ships as one component exporting exactly one axis world from `wit/specify.wit`.
- Identity comes from the guest crate's version and published `specify:<name>@<semver>` package. Resolve-time metadata comes from the component's WIT operation; there is no adapter manifest.
- Keep reusable adapter logic wasm-free in library modules. Each adapter implements its axis operations trait (`adapter::Source` / `adapter::Target`) on a unit type; the `wasm32` guest module is a single `adapter::source!` / `adapter::target!` export-macro invocation over that implementor.
- Do not commit built `.wasm` artifacts.
- Adapter names must remain unique across the source and target axes.

The root workspace includes `crates/*`, `sources/*`, `targets/*`, and `examples/change` (the wasm change example's host). The adapter SDK (`adapter`), the linked host (`linked`), and the lab-only evaluation library (`eval`) are git dependencies on `augentic/specify`, not local crates — resolved from the sibling `../specify` checkout by the committed path patch until the exposing engine revision is published and pinned by `rev`. `crates/lab/` is the native-only, unpublished composition binary: it owns the first-party `catalog()` declaration, the Tokio runtime, the Cursor backend construction, and the prompt-scenario root, dispatching between linked command mode and `eval::run`; the engine-owned `linked` supplies the catalog machinery, provider, reference hosting, and command execution, `eval` supplies the trial/scenario runners, telemetry, and deterministic grading, and the invoking task passes trial inputs explicitly. For sibling co-development the committed path patch in the root `Cargo.toml` resolves the engine crates from the `../specify` checkout.

## Prose and rules

Adapter `prose/` trees are compiled into their components:

- Parent prompts orchestrate an operation and load phase prompts by relative link. Keep them below roughly 150 non-blank lines.
- Phase prompts carry one operational phase. Prefer splitting or moving depth into references before approaching the 800-line hard cap.
- References are linked, not inlined. The embed-time walker includes Markdown documents and follows symlinks; keep relative links resolvable.
- Engineering standards are Markdown rules under `codex/rules/` and adapter-local `prose/rules/` overlays. Preserve stable rule IDs and namespace ownership.
- Contributor guidance belongs in `AGENTS.md`, not in the embedded engineering-rule corpus.

Generation behavior belongs in adapter prompts and wasm-free adapter logic, not in consumer-facing skills. Shared runtime references live under `codex/references/runtime/` and are exposed to adapters through their `prose/references/spec-runtime` symlinks.

## Rust and testing

Follow the workspace lint configuration in `Cargo.toml`. Prefer strong domain types, explicit errors, small functions, and comments that explain current invariants rather than history.

Testing is integration-first:

- Publicly reachable adapter behavior belongs in each adapter crate's `tests/` suite.
- Keep `src` unit tests only for genuinely unreachable defensive branches or dense pure matrices that are materially cheaper in-process.
- Do not widen public APIs solely for tests.
- Use `cargo nextest`, not bare `cargo test`, for native workspace tests; process isolation is required by CWD- and environment-mutating suites.
- Adapter native tests own operation behavior; the eval crate owns cross-phase integration and prompt quality; the operator-invoked change example owns WASM/WIT conformance. Do not duplicate the same assertion across rungs.
- The live rungs are operator-invoked, never CI: `cargo make eval` (the native live-model trial over `sandbox/`, deterministic grading only), `cargo make eval scenario <adapter>/<name>` (one adapter operation over a fixture scratch tree — the fast prompt-iteration loop), and `cargo make change-run` (the wasm change example composing this package's specify guest with the built adapter components).

Read [`TESTING.md`](TESTING.md) before adding, deleting, or relocating tests.

## Commands

Run from the repository root:

```bash
cargo make check          # fmt, clippy, nextest, doctests, docs
cargo make ci             # full gate, including vet and deny
cargo nextest run -p NAME # focused adapter tests
cargo make adapter NAME   # fast development component build
cargo make release        # release-build every component
cargo make specify -- ARGS # any specify verb through the native lab shim
cargo make eval [phase]   # live-model trial over sandbox/ (operator-invoked)
cargo make eval scenario [id]  # one live prompt scenario; bare lists them (operator-invoked)
cargo make change-run     # the wasm change example (operator-invoked)
```

Run `cargo make ci` before committing. If it cannot run, report exactly which narrower checks ran and why the full gate was unavailable.

## Area-specific guidance

- Vectis: [`targets/vectis/AGENTS.md`](targets/vectis/AGENTS.md)
- Rule catalog and namespace model: [`codex/rules/README.md`](codex/rules/README.md)
- Test ownership and live harnesses: [`TESTING.md`](TESTING.md)
