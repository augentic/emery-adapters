# Prompt evaluation

Live-model harness for first-party adapters (`examples/eval/`). The composition binary links every source and target into the native catalog and drives production verbs through the shared cursor backend. Outputs are graded by **deterministic** checks — not a model.

**Day-to-day loop** (run → debug → edit prose → re-run) lives in the [repo README](../../README.md). This directory owns the deeper references:

| Doc | Contents |
| --- | -------- |
| [scenarios.md](scenarios.md) | Single-operation scenarios: anatomy, index, how to add one |
| [trial.md](trial.md) | Full trial rhythm, grading, custom trials, Omnia migration example |

## Related

- [docs/testing.md](../../docs/testing.md) — where this example sits among the five rungs
- [examples/wasm/](../wasm/README.md) — same operator rhythm over the real WASM component seam (not the native catalog)
