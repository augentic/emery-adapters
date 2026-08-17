# Prompt evaluation

Live-model eval cases for first-party adapters, over real `emery` verbs and deterministic grading (not a model). Day-to-day repair loop: [repo README](../../README.md).


| Reach for a build case when…                           | Prefer a workflow case when…                          |
| ------------------------------------------------------ | ----------------------------------------------------- |
| You changed adapter prose                              | You need survey → extract → synthesis → build → merge |
| You have a committed refined slice fixture             | Sources are real trees (docs, TypeScript, …)          |
| You care about one build's report + `expect` artifacts | You care about plan progress and the merged baseline  |




## Quick start



### Prerequisites

- [cursor-agent](https://cursor.com/docs/cli) installed 
- `cursor-agent login` or `CURSOR_API_KEY` in root `/.env` file

See [Makefile.toml](Makefile.toml) for environment variable overrides for model, logging, and timeout.

From the repository root:

```bash
cargo make eval omnia-r9k --restart
cargo make eval contracts-design --restart
```



### Eval sandbox

Each case owns its own sandbox at `sandbox/<case>/` which allows for review and continued runs without having to re-run the entire workflow.

 Build reports (`.emery/change/slices/<slice>/build/report.yaml`) are available for review on build success. To correct input or output, edit the adapter prose and re-run the case using `--restart`.

Inspect or debug a retained run with a bound native verb:

```bash
cargo make eval orders-contracts plan status
```



## Catalog


| Id                     | Kind     | Shape                                                                                                                            |
| ---------------------- | -------- | -------------------------------------------------------------------------------------------------------------------------------- |
| `contracts-design`     | build    | Contracts from a design document                                                                                                 |
| `contracts-import`     | build    | Import vendored OpenAPI                                                                                                          |
| `omnia-health`         | build    | Tiny create-mode crate (`GET /health`)                                                                                           |
| `vectis-single-screen` | build    | Single-screen feature on `core + ios` (needs `$TEMPLATE_DIR`)                                                                    |
| `vectis-open-gap-fab`  | build    | Open-GAP FAB inventiveness desk-check (`core + ios`; needs `$TEMPLATE_DIR`) — [case README](cases/vectis-open-gap-fab/README.md) |
| `orders-contracts`     | workflow | docs → contracts over a reviewed definition home (`[examples/wasm/fixture](../wasm/fixture/)`)                                   |
| `omnia-r9k`            | workflow | `at_r9k_position_adapter` → omnia (cloned on first run)                                                                          |
| `orders-omnia`         | workflow | two-target docs → contracts, then intent → omnia                                                                                 |
| `orders-cap-one`       | workflow | the cap-comparison pair's serial half (`cap = 1`) over the orders-omnia shape, with a shared blind acceptance set (RFC-96 D11)   |
| `orders-cap-four`      | workflow | the cap-comparison pair's concurrent half (`cap = 4`), same definition, fixture, and blind set as `orders-cap-one`               |




## Continuing a run

Each case owns one stable retained sandbox at the repository-root `sandbox/<id>/` (composition-owned; beside the wasm examples' `sandbox/wasm-*/` trees), kept on success and failure alike. A failed or stopped **workflow** run is continued — graded — by re-running the same command: a sandbox holding an authored plan resumes at `plan refine` (the engine's own re-run contract) and still reaches the graded tail; a bound-not-authored sandbox re-runs `plan author`, which resumes its open and parked domains. `--restart` is the only runner-owned reset; build sandboxes and unbound workflow sandboxes refuse without it (a single build phase has no resume semantics — build-case repair is `--restart`). Inspect a retained sandbox with a bound native verb — a leading case id binds that case's sandbox-local stores:

```bash
cargo make eval orders-contracts           # resume the parked run, graded
cargo make eval orders-contracts plan status   # inspect it
```



## Manual native verbs

For maximum control (skip the case runner, keep a project across sessions), drive the catalog yourself. cargo-make needs `--` only when the first token is a cargo-make flag:

```bash
cargo make eval -- --project-dir /path/to/product init omnia --name <name>
cargo make eval plan author <change> \
  --from /path/to/definition --wave deliver --change-dir /path/to/change
cargo make eval plan refine --change-dir /path/to/change
cargo make eval plan execute --change-dir /path/to/change
```

This is the same native seam as the case runner; you own lifecycle and grading.

## Grading

Hard assertions only (the shared `probe` case runner):


| Case kind          | Check      | Pass condition                                                                                                                                                            |
| ------------------ | ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| workflow (plan)    | Entries    | `plan author` produces at least one entry, every entry `pending`                                                                                                          |
| workflow (execute) | Lifecycle  | Every plan entry is `done`                                                                                                                                                |
| workflow (execute) | Provenance | Every evidenced requirement on each materialized accepted CID carries sources; ids are present                                                                            |
| workflow (execute) | Blind set  | When the case declares `blind`, every `accept` needle appears in the accepted baseline (the set is never copied into the sandbox, so workflow model calls cannot read it) |
| build              | Lifecycle  | Slice metadata is `built`                                                                                                                                                 |
| build              | Report     | `.emery/change/slices/<slice>/build/report.yaml` exists                                                                                                                   |
| build              | Artifacts  | Every `expect` path holds a file inside the sandbox                                                                                                                       |


Per-leg request / repair counts are **reported, not asserted**. A leg drifting from zero repairs toward the budget is the early signal that a prompt or answer-schema change degraded the model's first answer.

After a completed workflow execute the runner also renders the **coordination-cost report** (RFC-96 D11) — accepted requirements/CIDs and code growth per target, time to first accepted result, build starts and rebuilds, waves per target, touched-path heat, per-leg request counts, and reported cost. Everything projects from journal facts, build records, and request telemetry — nothing is written into workflow artifacts, and cost stays `unknown` until RFC-92 provider usage facts land. Run a cap-comparison pair (`orders-cap-one` vs `orders-cap-four`) and compare the two reports.

Grading does **not** assert target-specific quality (contract YAML shape, generated Rust compiling outside the adapter's own verify-repair, etc.). Inspect the retained sandbox for that.

## Case shapes

A case is a data directory under `[cases/](cases/)`, flat kebab-case ids:

```text
cases/<id>/
  case.toml      kind = "workflow" | "build" plus the shape's fields
  fixture/       optional; copied into the fresh sandbox (case.toml may
                 instead point `fixture` at another tree, e.g. the wasm
                 example fixture)
```

A workflow case may instead declare `clone = { url = "…", dest = "…" }`
(mutually exclusive with `fixture`): on first run the runner
shallow-clones the upstream tree into the case's own `fixture/<dest>`
(stripping `.git`) and reuses that cache on every later run — for
source trees that cannot ship as committed fixtures, e.g. the
`UNLICENSED` omnia-r9k upstream, kept out of the repository by a
`.gitignore` in the case directory. Refresh the snapshot by deleting
the cached tree.

- `build` — `slice` + `expect`: the fixture carries the exact refined state the build phase consumes (`.emery/project.yaml`, the slice's `metadata.yaml` at `status: refined`, proposal / design / tasks / specs, and any source material such as `vendor/`). The runner drives the build orchestration for that slice once and gates on `built` metadata, the authoritative `build/report.yaml`, and every confined `expect` path.
- `workflow` — `change` + `wave` + a definition home (`definition = "…"` or sibling `definition/`): the runner copies the home, inits each path-locator target tree, then drives `plan author --from --wave` → `plan refine` → `plan execute`. `--until plan` / `refine` stop early; `--until finalize` adds `plan archive`. In-place mint from `intent` / `[sources]` remains for engine probe tests. Gates: a non-empty authored plan with every entry pending, every entry `done` after execute, then provenance grading over materialized accepted CIDs. Two optional RFC-96 D11 keys: `cap = <1..=8>` injects `EMERY_POOL` for the run (`1` is the serial reference of a cap-comparison pair), and `blind = "<path>"` names a TOML `accept = […]` blind acceptance set graded only against the accepted baseline — never copied into the sandbox.

Linked adapters need only the directory. A third-party adapter also needs a Cargo dep on `eval` and a catalog line in `[src/main.rs](src/main.rs)`.

## Vectis greenfield prerequisite

`vectis-single-screen`, `vectis-open-gap-fab`, and any live Vectis build that materializes shells need a local `[vectis-exemplar](https://github.com/augentic/vectis-exemplar)` checkout.

`cargo make eval` auto-sets `VECTIS_EXEMPLAR_DIR` to the workspace-sibling `../vectis-exemplar` when that checkout exists and the env var is unset. That matters because the eval sandbox is `sandbox/<case>/`: without the export, the build prelude's default `../vectis-exemplar` would resolve to `sandbox/vectis-exemplar`. For non-sibling layouts, export `VECTIS_EXEMPLAR_DIR` yourself to an absolute path before invoking make.

After materialize the agent strips `VECTIS-OPTIONAL` / `cap=demo` per `$TEMPLATE_DIR/AGENTS.md`, regenerates the iOS Xcode project (`make -C iOS generate-project`), runs `make build`, and stamps `iOS/.vectis/verify.ok` / `Android/.vectis/verify.ok` (adapter-owned; not in the template).

### Open-GAP inventiveness (`vectis-open-gap-fab`)

Build fixture for stub-faithful vs B′ closure when a FAB is unspecified but `Page::NewList` already exists. Probe grading is existence-level; inventiveness quality is sandbox inspection. Pass criteria and consumer Wasm desk-check seed: [cases/vectis-open-gap-fab/README.md](cases/vectis-open-gap-fab/README.md).

## Omnia legacy migration (r9k)

The `omnia-r9k` workflow case migrates Propellerhead's `[at_r9k_position_adapter](https://bitbucket.org/Propellerhead/at_r9k_position_adapter)` TypeScript service into an Omnia WASM crate (`typescript` source → `omnia` target).

The upstream tree is `UNLICENSED`, so the case's `clone` shallow-clones it into the case's gitignored `fixture/` cache on first run — it never enters the repository (the case directory carries the `.gitignore`). The first run needs network access to Bitbucket; later restarts reuse the cache offline. Refresh the snapshot with `rm -rf examples/eval/cases/omnia-r9k/fixture`:

```bash
cargo make eval omnia-r9k --restart          # tens of minutes of live model time
cargo make wasm-omnia-r9k                    # same rhythm over the real WASM seam
```

The component-seam twin shares the case's gitignored fixture cache. Pass/fail from grading is lifecycle + provenance; for migration quality, treat the generated crate, guest, tests, and `REVIEW.md` in the retained sandbox as the real signal. If you are editing omnia `prose/` and only need to know whether **build** still produces a crate, use `omnia-health` instead — do not burn a full r9k run for prompt typos.

## Related

- [docs/testing.md](../../docs/testing.md) — five-rung map
- [examples/wasm/](../wasm/README.md) — same rhythms (`wasm-contracts`, `wasm-omnia-r9k`) over the real WASM component seam
- Engine case-runner mechanics: `[crates/probe/README.md](https://github.com/augentic/emery/blob/main/crates/probe/README.md)`

