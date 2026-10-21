# ui-ir-staging

Drafts of the ui-ir pieces for the behaviour layer. The ui-ir contract
(`augentic/ui-ir-experiment`) has no behaviour schema yet. Each file here
moves into ui-ir as one separate step, then this directory is deleted.

## What is here

| File | What it is |
|---|---|
| `schema/core/behaviour.schema.json` | JSON Schema (2020-12) for `behaviour.json`, `$id urn:ui-ir:core:behaviour:1` |
| `vocab/behaviour-mappings.json` | the hand-written mapping table for behaviour (same shape as ui-ir `vocab/mappings.json`) |
| `docs/node-id.md` | the node id rule, with test vectors |
| `crates/ui-ir-behaviour/` | Rust: `node_id`, the embedded schema and table, `validate`, and a CLI (`check`, `coverage`) |
| `crates/code-to-behaviour/` | native, model-free reader: React + TypeScript source to `behaviour.json`, and `behaviour.json` to Emery `Evidence` claims |
| `tools/capture-to-behaviour/` | Playwright recorder and `apply` / `compare` (Node); `bin/run-samples.mjs` runs every sample end to end |
| `docs/behaviour-layer-proposal.md` | the proposal |

## Move list

1. `schema/core/behaviour.schema.json` to `ui-ir/schema/core/behaviour.schema.json`.
2. `vocab/behaviour-mappings.json` to `ui-ir/vocab/behaviour-mappings.json`
   (or merge its `converters` and `entries` into `vocab/mappings.json`; the
   source keys `jsx:`, `attr:`, `call:`, `zod:` do not collide with the
   existing ones).
3. `docs/node-id.md` to `ui-ir/docs/node-id.md`.
4. Into `ui-ir/crates/ui-ir/src/lib.rs`:
   - `layout::BEHAVIOUR = "behaviour.json"` (inside the IR directory, next to
     `composition.json`) and `layout::BEHAVIOUR_UNMAPPED =
     "behaviour.unmapped.json"` (beside the IR directory, like `UNMAPPED`).
   - `schema::BEHAVIOUR = include_str!("../../../schema/core/behaviour.schema.json")`.
   - `node_id(screen, path)` and its test vectors (from
     `crates/ui-ir-behaviour/src/lib.rs`).
   - the behaviour rows into the mapping loader (`Row`, `Mappings::lookup`
     here are the same first-match / `*`-prefix rules as `mapping.rs`).
5. Into `ui-ir/crates/ui-ir/src/validate.rs`: validate `behaviour.json` when
   present (the `validate` function here, same `jsonschema` 0.33 and
   `validator_for` + `iter_errors` as the existing code).
6. Into the `ui-ir` CLI: `check` covers `behaviour.json`; `coverage` (from
   `src/bin/ui-ir-behaviour.rs`) becomes a subcommand.
7. web-to-ir (ui-ir `sources/web-to-ir`): write `id = node_id(screen, path)` on every
   composition node that comes from a `computed.json` element, and fill each
   `"event": "TODO(event)"` hole from the trigger whose `on.nodes` (or
   `on.submitters[].nodes`) contains that id. Unclaimed holes stay `TODO`.
8. `crates/code-to-behaviour/` and `tools/capture-to-behaviour/` are not
   Emery source adapters (`adapters/<kind>` is reserved for wasm components
   with model turns, AGENTS.md). Change the `ui-ir-behaviour` path
   dependency of `crates/code-to-behaviour/Cargo.toml` (both
   `[dependencies]` and `[dev-dependencies]`) to the `ui-ir` crate, replace
   `ui_ir_behaviour::` with `ui_ir::`, and move both into ui-ir next to
   web-to-ir. Then remove `"ui-ir-staging/crates/*"` from the root
   `Cargo.toml` members, delete `ui-ir-staging/`, and run
   `cargo update -p ui-ir-behaviour` so `Cargo.lock` drops it.
9. `tools/capture-to-behaviour/lib/node-id.mjs` keeps its own copy of the
   rule (JavaScript); its test checks the same vectors.
10. An Emery `behaviour` source adapter, if wanted, is a separate crate under
   `adapters/behaviour`: model turns over the prose, then
   `code_to_behaviour::evidence` claims joined in (as typescript joins
   `type` claims), reading `behaviour.json` that the recorder wrote into the
   workspace (Playwright cannot run in a guest).

vx: the vx behaviour schema requires `intent` (prose). ui-ir behaviour has no
`intent`; an `intent` is an inferred claim of the LLM layer in emery-adapters
(see `docs/behaviour-layer-proposal.md` section 6). On the vx side it counts
as not filled in. vx is not changed.
