# Omnia target — verify prompt

> The omnia adapter core inlines this document into the system prompt of the `verify` operation: one model-assisted check pass over the lent candidate workspace (RFC-90). The operation receives no slice identity — the same pass runs against whatever candidate the engine lends. **One pass only**: run the checks, report the findings, fix nothing. Repair routing, retry rounds, and budgets are engine policy; after this pass the engine decides whether to dispatch [`repair.md`](repair.md) and re-verify.

## § Check pass

Run from the workspace root, over the whole cargo workspace the candidate carries:

```bash
cargo fmt --check
cargo check
cargo clippy --all-targets -- -D warnings
cargo test
```

Run all four even when an earlier one fails — the engine's repair brief is most useful when one pass reports every failure class. Do not run `cargo fmt`, edit any file, or re-run a failed command after changing anything: this operation observes, it never repairs.

## Findings

Report each distinct failure as one finding:

- `title` — the failing check plus the error headline (e.g. `cargo clippy: needless clone in payments handler`).
- `severity` — `critical` when the tree fails to compile (`cargo check` errors); `important` for formatting, clippy `-D warnings`, and test failures; `suggestion` for advisory output that blocks nothing.
- `source: model-assisted`, `kind: violation`, `artifact` — `code` for `src/` paths and manifests, `tests` for `tests/` paths.
- `location` — the workspace-relative `path` (plus `line` / `column`) whenever the cargo output names one.
- `evidence` — `kind: snippet` with a bounded verbatim excerpt of the error output.
- `impact` and `remediation` — what breaks and the concrete fix action; for a `cargo test` failure, name the failing test and classify it per the table in [`repair-patterns.md`](../references/repair-patterns.md) (test issue vs code issue vs manifest issue) so the repair pass can route it without re-deriving.

Do not fold multiple errors into one finding and do not suppress findings you believe are cascades unless the cargo output itself declares the dependency.

## Report

Answer with one phase report: `outcome: completed`, `source: model-assisted` (you ran and interpreted native commands), `findings[]` as above, **empty `outputs`**, **no `ui-surface`**, empty `written` (this pass writes nothing), no continuation. A clean pass is the same report with no findings — it is self-consistency evidence (the candidate passes its own checks, including tests this build may have authored), not an independent oracle; never overstate it.

The engine owns the loop: never retry, never repair, never select a next operation, never transition the slice.
