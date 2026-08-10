# Vectis target — review prompt

The adapter core inlines this document into the system prompt of the engine-dispatched `review` operation, ahead of the core review prompt and each in-scope shell review prompt. One engineering-standards review pass over the lent workspace: spawn the reviewer teams, consolidate their findings, and answer with the phase report.

**One pass, report only.** This operation never remediates, auto-fixes, or loops: blocking findings return to the engine, which routes them through its bounded `repair` dispatch and re-verifies. Do not edit the workspace, mark tasks, write stamps, or run the mechanical verify commands here.

## Standard arguments

Resolve these before any fan-out; every assembled review prompt below reads them:

| Symbol | Meaning |
| --- | --- |
| `SLICE_ID` | The active slice name (supplied by the user prompt). |
| `PROJECT_DIR` | The lent workspace root — the product code under review. |
| `STAGE_DIR` | The writable artifact stage root named in the user prompt — the candidate slice tree (`composition.yaml`, `tasks.md`, bookkeeping). Read-only in this operation. |
| `IOS_SHELL_DIR` | `${PROJECT_DIR}/iOS` (only when an iOS review prompt is assembled). |
| `ANDROID_SHELL_DIR` | `${PROJECT_DIR}/Android` (only when an Android review prompt is assembled). |
| `APP_NAME` | The Xcode target / Swift source folder name, read from the tree. |

Slice artifacts (`spec.md`, `design.md`, `tasks.md`) read from the stage when present, else from the read-only slice tree the user prompt names.

## Pipeline

1. Run the core review prompt's reviewer team (structural / logic / quality specialists plus the antagonist pass) over `shared/`.
2. For each in-scope shell whose review prompt is assembled below, run its platform reviewer team; teams may run in parallel per [`agent-teams.md`](../references/agent-teams.md).
3. Consolidate everything per the core review prompt's `## § Consolidate review findings` — deduplicate, resolve severity conflicts upward, drop nothing silently.
4. Map every consolidated finding onto the phase-report finding shape: `title`, `severity` (`critical` / `important` / `suggestion` / `optional`), `source: model-assisted`, `kind: violation` for rule breaches (`review` for judgment-grade concerns), `artifact`, `location` (`file:line` when known), snippet `evidence`, `impact`, `remediation`, and the codex `rule-id` when one applies.

## Answer

Answer with the phase report: `outcome: completed` and the consolidated findings (empty when the review is clean), empty `outputs`, no `ui-surface`, nothing under `written`. When there is no core or shell tree in the workspace to review, answer `outcome: not-applicable`.
