# Shared guardrails

Cross-cutting "do not / never / always" rules that apply across many adapter prompts. Prompts should **link** to the relevant section here rather than restating these rules verbatim.

Per-skill guardrails — rules that only make sense for one skill ("never auto-promote a `component:` slug", "never invent cost figures", etc.) — stay in the owning `SKILL.md`. Lift to this file only when 3+ skills repeat the same rule.

## Single-writer for lifecycle state

The CLI is the **only** writer for change and slice lifecycle state. Skills route every write through a CLI verb; they never edit the underlying files by hand.

- **Never hand-edit `plan.yaml`.** Append entries through `emery plan add`; Gate 1 is stamped by the first `emery plan execute` (invoking execute is the approval act); walk entries backwards through `emery plan transition --undo`; close out the plan through `emery plan archive`. The single-writer contract lives in the [`plan` skill](https://emery.augentic.io/reference/change-skills/plan.html).
- **Never hand-edit `.emery/slices/<name>/metadata.yaml`.** Status transitions and timestamp writes are owned by the guest orchestrations (`emery slice refine` / `build` / `merge` / `drop`). The CLI enforces the legal lifecycle edges — skills do not need to track them.
- **Never hand-edit `.emery/archive/`.** Archive moves are atomic operations performed by `emery slice merge`, `emery slice drop`, and `emery plan archive`.
- **Never hand-roll `AGENTS.md` during init.** `emery init` generates it when absent, preserves an existing root `AGENTS.md`, and writes `.emery/context.lock` as the generation fingerprint.

## Baseline immutability for contract authoring

Contract authoring skills (OpenAPI, AsyncAPI, JSON Schema) write only inside the active slice directory. The shared baseline is read-only to authoring; merge into the baseline is a separate, explicit step.

- **Do not modify any file outside `$SLICE_DIR/contracts/`** (or `$SLICE_DIR/contracts/schemas/` for the JSON Schema skill).
- **Never modify baseline files in root `contracts/`.** All authored output lands in the slice-local `contracts/` directory; merging into the baseline is `emery slice merge run`'s job.
- **Never silently delete or narrow a baseline schema's fields.** If the spec requires it, surface the slice as a warning and let a human operator decide whether to bump the schema's `$id`.

## Consumer tooling boundary

During slice **execute / build / merge**, agents are **consumers** of Emery and adapters — not maintainers.

- **Do not** edit `emery`, `emery-adapters`, adapter templates, `guest.wasm`, or `~/.cache/emery/**` to unblock a failing build.
- **Do not** run `emery adapter build`, `sync *-scaffold`, or rebuild/copy WASM to work around verify drift during consumer execute (sync/scaffold remain orchestrator-owned per the Vectis build prompt; this rule blocks *agent-initiated* upstream patching).
- On scaffold, verify, finalize, or toolchain failure: **stop**, print CLI `stop:` / `hint:` / `resume:` output, and exit. Tooling fixes belong in emery / emery-adapters in a separate maintainer session.
- Canonical "stop, don't patch" example: Vectis [Template / version-pin drift handling](https://github.com/augentic/emery-adapters/blob/main/targets/vectis/prose/prompts/build.md#template--version-pin-drift-handling) (`targets/vectis/prose/prompts/build.md` in emery-adapters).
