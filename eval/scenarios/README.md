# Prompt scenarios

Single-operation prompt scenarios for the live eval rung: each drives one
adapter seam operation (a `build`, or one `merge` gate) end-to-end against the
real cursor backend, natively through the same seam provider the `specify-dev`
CLI and the trial use. This is the fast prompt-iteration loop — edit an
adapter's `prose/`, rebuild natively in seconds, re-run the scenario.

## Anatomy

A scenario is a data directory, not Rust — third-party adapters get the same
rung by dropping one in:

```text
scenarios/<adapter>/<name>/
  scenario.toml    routing: axis-qualified adapter id, operation, slice name,
                   and the `expect` artifact-exists gate (scratch-relative
                   paths a passing run must produce)
  inputs/*.md      typed slice inputs by file stem (proposal / design / tasks / spec*)
  seed/**          files copied into the scratch project root (optional)
```

The runner ([`../src/scenario.rs`](../src/scenario.rs)) seeds a fresh scratch
tree under the gitignored `sandbox/scenarios/<adapter>/<name>/run-<stamp>/`,
dispatches the operation over the linked adapter, writes `report.json` beside
the scratch delta, and fails on a failing report or a missing `expect`
artifact (a success report that produced nothing is a silent no-op, not a
pass). The scratch tree is retained for review.

## Running

Requires [`cursor-agent`](https://cursor.com/docs/cli) on `PATH`, authenticated
via `CURSOR_API_KEY` or a prior `cursor-agent login`. Set
`SPECIFY_EVAL_MODEL=<model-id>` to override the model driver-side for a run.

```bash
cargo make eval scenario                     # list scenarios
cargo make eval scenario contracts/design    # run one
```

The model-free smoke — every `scenario.toml` parses, routes to a linked
adapter, and carries inputs — runs with the eval workspace tests
(`cargo make eval-test`); no scenario executes a model there.

## Index

| Scenario                | Slice                        | Shape                                                                       |
| ----------------------- | ---------------------------- | --------------------------------------------------------------------------- |
| `contracts/describe`    | `user-adapter-api`           | Generate schema + HTTP contracts from prose inputs                          |
| `contracts/design`      | `returns-api`                | Generate contracts from a design document                                   |
| `contracts/import`      | `import-ticket-api-contract` | Import a vendored OpenAPI document into the contract tree                   |
| `contracts/source`      | `orders-api-contract`        | Extract contracts from a vendored TypeScript service                        |
| `contracts/update`      | `loyalty-api-contract`       | Update an existing contract baseline                                        |
| `vectis/single-screen`  | `daily-quote`                | A tiny single-screen feature on `core + ios` — composition, core, shell     |

The contracts scenarios mirror the operator-driven scenario packs beside them
(see [`contracts/README.md`](contracts/README.md)), reduced to the build leg
this rung exercises.
