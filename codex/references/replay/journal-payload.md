# Replay payload shapes

Closed shapes for the build-time `replay` hook. Recording rules and merge posture live in [`hook-contract.md`](hook-contract.md).

## No journal event

Emery core does **not** define a `slice.replay.completed` journal event. Replay results are advisory in the build transcript only — classify `passed` / `failed` / `skipped` there and stop. Do not write journal events for replay (there is no emit verb; journal writes are engine-orchestration side effects).

## Aspirational `metadata.yaml` block (future CLI)

The capture-backed replay workflow defines an additive block targets MAY write to `$SLICE_DIR/metadata.yaml` once a CLI-owned surface lands. **Agents must not hand-edit this block** — there is no core recorder until then.

```yaml
replay:
  passed: <int>
  failed: <int>
  skipped: <int>
  ran-at: <ISO-8601 UTC>
  runner: <e.g. "omnia-target@1 (cargo nextest)">
```

Worked examples:

- With block: [`quality/fixtures/reference/targets/omnia/with-replay/`](https://github.com/augentic/emery/tree/main/quality/fixtures/reference/targets/omnia/with-replay)
- Without block (omission-is-not-an-error): [`quality/fixtures/reference/targets/omnia/without-replay/`](https://github.com/augentic/emery/tree/main/quality/fixtures/reference/targets/omnia/without-replay)

The block is additive; it must not reshape other `metadata.yaml` fields. The merge phase reads it when present for the one-line closing summary described in [`hook-contract.md`](hook-contract.md).

## See also

- [`hook-contract.md`](hook-contract.md) — when to run, advisory posture, merge rules
- [`../../../targets/omnia/prose/prompts/build/replay.md`](../../../targets/omnia/prose/prompts/build/replay.md) — Omnia runner
