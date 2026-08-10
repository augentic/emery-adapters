# Omnia target — repair prompt

> The omnia adapter core inlines this document into the system prompt of the `repair` operation: one findings-directed writer pass (RFC-90). The engine supplies `repair-origin` — which gate produced the findings — and the deterministic bounded repair brief (the typed findings in the user prompt). **One pass only**: apply the fixes and answer; the engine always dispatches a fresh verification afterwards, so never re-run the full check suite in a loop, re-review, or select a next operation. Findings beyond the brief remain gate-visible engine-side — fix what the brief names, nothing speculative.

## Repair discipline (both origins)

Minimum change only — fix the reported finding and nothing else. Scope the diff to the files and functions the finding names. Group same-class findings and fix each class once. Full recipes: [`repair-patterns.md`](../references/repair-patterns.md). Idempotency holds: if a finding is already fixed in the tree, touch nothing and say so in the answer.

You may run a single targeted command (e.g. `cargo check`) to understand a failure or confirm one fix compiles, but do not iterate — the engine's next verification is the authority on whether the repair succeeded.

## `repair-origin: verification`

Mechanical check failures from the [`verify`](verify.md) pass:

- **`cargo fmt --check`** — run `cargo fmt` once; formatting is mechanical.
- **`cargo check` / `cargo clippy` errors** — minimum-change code repair per [`repair-patterns.md`](../references/repair-patterns.md), honouring the crate-writer authority hierarchy ([`build/crate.md`](build/crate.md)): artifacts stay ground truth, no `unwrap()` / `expect()` in production code, WASM guardrails absolute.
- **`cargo test` failures** — classify per the table in [`repair-patterns.md`](../references/repair-patterns.md): errors in `tests/` paths or `MockProvider` are test-side fixes; errors in `src/` paths are code-side fixes; manifest / workspace errors are fixed in `Cargo.toml` directly. In update mode, a previously-passing test that now fails is a true regression unless the spec explicitly changed the asserted behaviour — fix the code, not the test, when the spec did not change.

## `repair-origin: review`

Engineering-standards findings from the [`review`](review.md) pass's `REVIEW.md` synthesis:

- Apply each finding's `remediation` directly, respecting its `rule-id` codex citation (the rule bodies are fetchable under `rules/` via the MCP references).
- Route the fix by `artifact`: `code` findings land in `src/`, `tests` findings in the test suite; never weaken a test to silence a standards finding.
- Safe-fix recipes and the regression guard live in [`review-auto-fix.md`](../references/review-auto-fix.md); do not re-run the review team — the engine re-reviews after re-verification.

## Report

Answer with one phase report: `outcome: completed`, `source: model-assisted`, **empty `outputs`**, **no `ui-surface`**, `written[]` naming the workspace-relative files this pass edited (`root: workspace`), no continuation. Report a finding only for something you could not repair (same diagnostic shape as the brief); it is evidence, not routing — the next verification remains the gate.

The engine owns budgets and routing: never claim the loop is finished, never transition the slice.
