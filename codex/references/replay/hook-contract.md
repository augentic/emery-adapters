# Replay hook contract

Target-agnostic rules for the optional build-time `replay` hook (the capture-backed replay workflow). Each implementing target adds a runner prompt that links here and supplies target-specific paths and commands.

## When to run

The hook is **OPTIONAL**. Run it only when the slice's `plan.yaml.sources[]` list carries a `captures` binding. Targets that skip the step produce no `replay` surface and emit no replay journal event; **omission is not an error**.

## Preconditions

Before invoking the target-specific runner:

1. **Prior build phases complete** — code generation, tests, and any target-local verify/review steps that must precede replay have finished.
2. **Evidence or captures available** — the slice's Evidence includes `kind: example` claims from the `captures` extract pass, or the bound capture tree remains readable at the plan-level source path.
3. **Replay tests exist** — the target's test-generation phase has produced tests that exercise the captured scenarios (each implementing target documents where those tests live).

Capture wire format: [`captures/references/capture-format.md`](../../../sources/captures/prose/references/capture-format.md). Claim shape and 64 KiB inline cap: [`captures/prompts/extract.md`](../../../sources/captures/prose/prompts/extract.md).

## Advisory posture

Replay failures are **advisory**:

- A non-zero `failed` count does **not** park the build.
- The slice still transitions to `built`.
- The operator inspects replay results at merge time via the journal event.

This matches the current synthesis posture on `[conflict]` and `[divergence]` tags — review signals, not automatic gates. Stricter posture belongs in a custom target adapter fork, CI policy on journal events, or a future core contract.

## Recording results

### Recorder: journal event

Emit `slice.replay.completed` (`project::journal::EventKind::SliceReplayCompleted` in `augentic/specify`) via `specify journal emit slice.replay.completed --payload <json>`. Payload shape: [`journal-payload.md`](journal-payload.md).

The implementing target's runner prompt supplies the `runner` string (e.g. `omnia-target@1 (cargo nextest)`).

### Do not hand-edit `metadata.yaml`

Agents must not write slice metadata by hand. The phase contract has no `slice outcome set` CLI surface — see [`phase-outcome-contract.md`](../runtime/phase-outcome-contract.md).

A future CLI surface may persist a `replay:` block to `$SLICE_DIR/metadata.yaml`. Until that lands, the journal event is the sole supported recorder. The aspirational block shape lives in [`journal-payload.md`](journal-payload.md).

## Merge posture

When a `replay:` block is present on `metadata.yaml` (operator tooling or a future CLI surface), `/spec:merge` surfaces a one-line summary in its closing message:

```text
replay: <passed> passed, <failed> failed, <skipped> skipped
```

Rules:

- **Missing block** → omit the line; absence is not an error.
- **`failed > 0`** → `merge` does **not** auto-refuse; the operator decides whether to land.

Capture the block before archival if present — `specify slice merge` moves the slice directory.

## See also

- [`README.md`](README.md) — target adoption table
- [`journal-payload.md`](journal-payload.md) — closed payload shapes
- [`../../../targets/omnia/prose/prompts/build/replay.md`](../../../targets/omnia/prose/prompts/build/replay.md) — Omnia runner (reference implementation)
