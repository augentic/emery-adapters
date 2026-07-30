# Omnia build — capture replay

Dispatched by the adapter core only when the build context's bound source names carry `captures` — the engine forwards the slice's bindings on the build call, so when `captures` is unbound the adapter skips this leg in-guest and no agent is spawned. The leg runs after generation and before standards review, whose findings synthesis folds unresolved replay failures.

## Shared contract

Read [`../../../../codex/references/replay/hook-contract.md`](../../../../../codex/references/replay/hook-contract.md) first — skip rules, generic preconditions, advisory posture, no core recorder, merge summary, and the ban on hand-editing `metadata.yaml`. Aspirational payload shapes: [`../../../../codex/references/replay/journal-payload.md`](../../../../../codex/references/replay/journal-payload.md).

## Omnia preconditions

In addition to the shared contract:

- Generation complete: crate, tests, guest (create mode), and the verify-repair loop have run.
- Replay data is present under `$CRATE_PATH/tests/data/replays/` — copied or symlinked during [test writer](test.md) when a `captures` binding exists.

Capture wire format: [`captures/references/capture-format.md`](../../../../../sources/captures/prose/references/capture-format.md). Claim shape and 64 KiB inline cap: [`captures/prompts/extract.md`](../../../../../sources/captures/prose/prompts/extract.md).

## Omnia execution

1. **Confirm replay tree.** List `$CRATE_PATH/tests/data/replays/<handler>/*.json`. Every scenario file the `captures` adapter extracted should have a corresponding integration test from phase 3; if gaps exist, re-enter [test.md](test.md) before replay.

2. **Run the replay suite.**

   ```bash
   cd $CRATE_PATH && cargo nextest run --tests
   ```

   Fall back to `cargo test` when nextest is unavailable. The operator's `captures` binding may point at a different root than the crate copy — replay always runs against `$CRATE_PATH/tests/data/replays/`.

3. **Classify results** in the build transcript and your answer's summary (passed / failed / skipped) per the shared contract (advisory in v1) — the standards-review leg folds unresolved failures into the build report's findings. Do **not** emit a journal event and do **not** hand-edit `metadata.yaml`.

## References

- [`../../../../codex/references/replay/README.md`](../../../../../codex/references/replay/README.md) — shared hook index and target adoption table
- [`../../references/replay-crate-layout.md`](../../references/replay-crate-layout.md) — crate paths and fixture loading
- [`../../references/replay-fixtures.md`](../../references/replay-fixtures.md) — `setup` block and MockProvider mapping
- [`../../references/examples/replay/`](../../references/examples/replay/) — worked migration examples
- [`test.md`](test.md) — generates replay integration tests in phase 3
