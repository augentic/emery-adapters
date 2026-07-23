# Prompt evaluation

Live-model harness for first-party adapters. The composition binary (`examples/eval/`) links every source and target into the native catalog and drives production verbs through the shared cursor backend. Outputs are graded by **deterministic** checks — not a model.

## Prerequisites

From the `specify-adapters` repo root:

1. Authenticated [`cursor-agent`](https://cursor.com/docs/cli) on `PATH`:
  - `cursor-agent login`, or
  - `CURSOR_API_KEY` in a repo-root `.env` (the `eval` task loads it).
2. Optional: `EVAL_MODEL=<model-id>`, `EVAL_TIMEOUT_SECS=<secs>` (the `eval` task defaults timeout to `300`).

Any specify verb through the native catalog[^1]:

```bash
make specify -- --project-dir <dir> slice list
```



## Choose a loop


| Loop           | Use when                                                                                                | Command                                               | Doc                          |
| -------------- | ------------------------------------------------------------------------------------------------------- | ----------------------------------------------------- | ---------------------------- |
| **Scenario**   | Fast prompt iteration on one adapter operation (`build` / merge gate). Synthetic slice inputs; minutes. | `make eval scenario …`                                | [scenarios.md](scenarios.md) |
| **Full trial** | End-to-end operator rhythm with real sources → real working-tree outputs. Tens of minutes.              | `make eval` or a custom `cargo run -p eval -- eval …` | [trial.md](trial.md)         |


```text
scenario   one seam op over inputs/*.md (+ optional fixture)
               → sandbox/<adapter>/<name>/run-…/ + report.json

trial      init → plan author → approved → plan execute → archive
               → sandbox/ project (cleaned on full pass)
```

### Start here by target

Stock `make eval` is the **contracts** trial only. Other targets use a scenario and/or a [custom trial](trial.md#custom-trials).

| Target | Scenario smoke | Full trial |
| ------ | -------------- | ---------- |
| **contracts** | `make eval scenario contracts/design` ([index](scenarios.md#index)) | `make eval` ([trial.md](trial.md)) |
| **omnia** | `make eval scenario omnia/health` | Custom trial — e.g. TypeScript → Omnia migration ([r9k / test-spec](trial.md#example-omnia-legacy-migration-test-spec--r9k-shape)) |
| **vectis** | `make eval scenario vectis/single-screen` | Custom trial (`--target vectis`, fixture + platforms as needed) |

## Related

- [TESTING.md](../../TESTING.md) — where this example sits among the five rungs
- [examples/wasm/](../wasm/README.md) — same operator rhythm over the real WASM component seam (not the native catalog)

[^1]: `--project-dir` is a lab convenience on this binary (before the subcommand). It is not a global flag on the shipped `specify` CLI.