# Prompt evaluation

Live-model eval cases for first-party adapters, over real `specify` verbs and deterministic grading (not a model). Day-to-day repair loop: [repo README](../../README.md).

## Quick start

Needs [`cursor-agent`](https://cursor.com/docs/cli) on `PATH` (`cursor-agent login` or `CURSOR_API_KEY` in a repo-root `.env`). Optional: `EVAL_MODEL=<model-id>`. Tracing: `RUST_LOG` (default `info,opentelemetry_sdk=off` via the make task) and optional `OTEL_GRPC_URL` for OTLP — `probe::client` initializes `omnia::Telemetry` and calls `omnia::telemetry::flush` before exit on every native run.

From the repository root:

```bash
make eval                                # list the cases
make eval contracts-design --restart     # one build case (~2–5 minutes)
make eval omnia-health --restart         # omnia smoke
make eval orders-contracts --restart     # full workflow (tens of minutes)
```

A build case prints its retained sandbox and the authoritative report path (`.specify/slices/<slice>/build/report.yaml`) on success. Edit `{targets,sources}/<name>/prose/`, re-run the same command with `--restart`; native runs pick up prose changes with no Wasm rebuild.

## Catalog

| Id | Kind | Shape |
| --- | --- | --- |
| `contracts-describe` | build | Schema + HTTP contracts from prose |
| `contracts-design` | build | Contracts from a design document |
| `contracts-import` | build | Import vendored OpenAPI |
| `contracts-source` | build | Extract from vendored TypeScript |
| `omnia-health` | build | Tiny create-mode crate (`GET /health`) |
| `vectis-single-screen` | build | Single-screen feature on `core + ios` |
| `orders-contracts` | workflow | docs → contracts ([`examples/wasm/fixture`](../wasm/fixture/)) |
| `omnia-r9k` | workflow | `at_r9k_position_adapter` → omnia (cloned on first run) |

| Reach for a build case when… | Prefer a workflow case when… |
| --- | --- |
| You changed adapter prose and want a minutes-scale signal | You need survey → extract → synthesis → build → merge |
| You have a committed refined slice fixture | Sources are real trees (docs, TypeScript, …) |
| You care about one build's report + `expect` artifacts | You care about plan lifecycle and the merged baseline |

## Case shapes

A case is a data directory under [`cases/`](cases/), flat kebab-case ids:

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

- **`build`** — `slice` + `expect`: the fixture carries the exact refined state `specify slice build` consumes (`.specify/project.yaml`, the slice's `metadata.yaml` at `status: refined`, proposal / design / tasks / specs, and any source material such as `vendor/`). The runner invokes `slice build <slice>` once and gates on `built` metadata, the authoritative `build/report.yaml`, and every confined `expect` path.
- **`workflow`** — `target` + `change` + `intent` / `[sources]`: init, `plan author`, then (past `--until plan`) `plan approve` and the genuine drained `plan execute`; `--until finalize` adds `plan archive`. Gates: a non-empty pending plan at Gate 1, every entry `done` after execute, then provenance grading.

Linked adapters need only the directory. A third-party adapter also needs a Cargo dep on `eval` and a catalog line in [`src/main.rs`](src/main.rs).

## Sandboxes and continuation

Each case owns one stable retained sandbox at `sandbox/<id>/` (a sibling of `cases/`), kept on success and failure alike. `--restart` is the only runner-owned reset; an existing sandbox without it refuses before mutation. The runner never infers workflow progress — continue or debug a retained sandbox explicitly through the native verbs:

```bash
cargo make specify -- --project-dir examples/eval/sandbox/orders-contracts plan approve
cargo make specify -- --project-dir examples/eval/sandbox/orders-contracts plan execute
```

## Grading

Hard assertions only (the shared `probe` case runner):

| Case kind | Check | Pass condition |
| --- | --- | --- |
| workflow (plan) | Entries | `plan author` produces at least one entry, lifecycle `pending` |
| workflow (execute) | Lifecycle | Every plan entry is `done` |
| workflow (execute) | Provenance | Every evidenced requirement carries sources; ids are present |
| build | Lifecycle | Slice metadata is `built` |
| build | Report | `.specify/slices/<slice>/build/report.yaml` exists |
| build | Artifacts | Every `expect` path holds a file inside the sandbox |

Per-leg request / repair counts are **reported, not asserted**. A leg drifting from zero repairs toward the budget is the early signal that a prompt or answer-schema change degraded the model's first answer.

Grading does **not** assert target-specific quality (contract YAML shape, generated Rust compiling outside the adapter's own verify-repair, etc.). Inspect the retained sandbox for that.

## Omnia legacy migration (r9k)

The `omnia-r9k` workflow case migrates Propellerhead's [`at_r9k_position_adapter`](https://bitbucket.org/Propellerhead/at_r9k_position_adapter) TypeScript service into an Omnia WASM crate (`typescript` source → `omnia` target).

The upstream tree is `UNLICENSED`, so the case's `clone` shallow-clones it into the case's gitignored `fixture/` cache on first run — it never enters the repository (the case directory carries the `.gitignore`). The first run needs network access to Bitbucket; later restarts reuse the cache offline. Refresh the snapshot with `rm -rf examples/eval/cases/omnia-r9k/fixture`:

```bash
make eval omnia-r9k --restart          # tens of minutes of live model time
```

Pass/fail from grading is lifecycle + provenance; for migration quality, treat the generated crate, guest, tests, and `REVIEW.md` in the retained sandbox as the real signal. If you are editing omnia `prose/` and only need to know whether **build** still produces a crate, use `omnia-health` instead — do not burn a full r9k run for prompt typos.

## Manual native verbs

For maximum control (skip the case runner, keep a project across sessions), drive the catalog yourself:

```bash
cargo make specify -- --project-dir /path/to/project init omnia --name <name>
cargo make specify -- --project-dir /path/to/project plan author <change> \
  --intent "…" \
  --source "legacy=typescript:legacy/at_r9k_position_adapter"
cargo make specify -- --project-dir /path/to/project plan approve
cargo make specify -- --project-dir /path/to/project plan execute
```

This is the same native seam as the case runner; you own lifecycle and grading.

## Related

- [docs/testing.md](../../docs/testing.md) — five-rung map
- [examples/wasm/](../wasm/README.md) — same rhythm over the real WASM component seam
- Engine case-runner mechanics: [`crates/probe/README.md`](https://github.com/augentic/specify/blob/main/crates/probe/README.md)
