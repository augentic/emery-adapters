# Vectis build — core review

Inlined by the adapter core into the review leg's system prompt (alongside [../../build.md](../../build.md) and the in-scope shell review prompts) after the mid-build core verify-repair loop succeeds. Scope: the Rust `shared` crate. Drives an agent team — three specialists plus an antagonist — through a bounded review-fix loop (max 3 iterations). Mechanical fixes to `shared/` remain allowed here; a dedicated final-core-verify leg always re-runs Step 6 (and refreshes the digest stamp) before the build report — mid-build verify is not the last clippy pass.

The shared agent-team protocol lives in [`../../../references/agent-teams.md`](../../../references/agent-teams.md); the core-specific team-spawn protocol lives in [`review/team-protocol-core.md`](../../../references/review/team-protocol-core.md).

## Pipeline

1. **Verify prerequisites** — the mid-build core verify-repair loop returned `success`, `${PROJECT_DIR}/shared/` exists, and `cargo check` passes.
2. **Spawn specialists concurrently** with the verbatim prompts in [`review/team-protocol-core.md`](../../../references/review/team-protocol-core.md):
   - **Structural** — CRX-001..011: missing `render()`, serde derives, input validation, `PendingOp` timestamps, ViewModel typing, unused deps. Full library: [`review/crux-checks.md`](../../../references/review/crux-checks.md).
   - **Logic** — LOG-001..009: state-machine completeness, op coalescing, concurrent conflicts, temporal ordering, rapid-action sequences, spec gaps, spec-to-test coverage, stale tests. Full library: [`review/logic-checks.md`](../../../references/review/logic-checks.md).
   - **Quality** — GEN-001..013: no `unwrap` / `expect` outside test setup, no debug output, no hardcoded secrets, error propagation, match exhaustiveness, function length, no inline lint suppressions (**GEN-013**, codex `VECTIS-009`). Full library: [`review/general-checks.md`](../../../references/review/general-checks.md).
3. **Universal checks (lead).** Apply every `UNI-*` rule from the shared universal codex pack ([`../../../rules/universal/`](../../../rules/universal/), embedded in this adapter) with Rust / Crux heuristics. Full library: [`review/universal-checks.md`](../../../references/review/universal-checks.md). Skip universal checks already covered by the specialists per the dedupe table in [`review/team-protocol-core.md`](../../../references/review/team-protocol-core.md).
4. **Adversarial challenge.** Forward all findings to the antagonist. The antagonist confirms, upgrades, downgrades, disputes, and may add `NEW-` findings. Protocol: [`agent-teams.md`](../../../references/agent-teams.md).
5. **Synthesis.** Lead authors the iteration report per [`review/iteration-report.md`](../../../references/review/iteration-report.md).
6. **Mechanical auto-fixes (when safe).** Missing serde derives, `render().and(...)` wraps, `.trim()` / empty input checks, unused deps. Revert the full batch if `cargo check` / `cargo clippy` / `cargo test` regress.
7. **Logic findings stay non-mechanical.** Never auto-fix LOG-001..008 without explicit confirmation; surface them as design-level findings classified `code-fix` or `spec-change`.

## Standalone vs orchestrated

The core reviewer has no orchestrated mode — when design-level findings accumulate it always returns them for consolidation by the parent build prompt / operator. Per-platform shell reviewers ([`../ios/review.md`](../ios/review.md), [`../android/review.md`](../android/review.md)) honour the `orchestrated: true` flag.

## § Consolidate review findings

When all in-scope reviews complete:

1. **Merge findings.** Combine `design_findings` from each reviewer into a single list. Deduplicate universal findings (UNI-prefixed) that both reviewers flagged with identical check IDs and matching evidence — keep the higher-severity instance. Platform-specific findings (CRX-, LOG-, GEN-, IOS-, SWF-, AND-, KTL-, INT-prefixed) are always distinct.
2. **Empty list.** Skip the rest of this section.
3. **Validate classifications.** Each finding already carries `code-fix` or `spec-change`. Treat that as the source of truth. Resolve disagreements between platforms by applying: spec is clear but code is wrong → `code-fix`; spec is silent, ambiguous, or problematic → `spec-change`.
4. **Surface findings.** Findings flow to the operator alongside the build outcome. Cross-platform follow-up work is queued as a new slice via the operator's normal `/emery:plan` flow rather than letting reviewers spawn slices directly.

## § Standards review surface

The per-platform reviewers (this prompt, [`../ios/review.md`](../ios/review.md), [`../android/review.md`](../android/review.md)) carry the model-assisted surface — specialist + antagonist judgment per [`agent-teams.md`](../../../references/agent-teams.md), applying the engineering-standards rules shipped under [`../../../rules/`](../../../rules/) (the Vectis overlay plus the shared `UNI-*` pack at `rules/universal/`).

Vectis render-by-`kind` drift ([`VECTIS-006`](../../../rules/VECTIS-006-asset-render-by-kind.md)) is review-scoped in v1: iOS and Android Integration specialists run **IOS-020** / **AND-028** on the first full-scope iteration (see per-platform review prompts and team protocols).

Framework acceptance fixtures under `quality/fixtures/reference/targets/vectis/` version-control `design-system/assets/exports/` (see [`task-list/design-system/`](https://github.com/augentic/emery/tree/main/quality/fixtures/reference/targets/vectis/task-list/design-system)) so build prompt examples and eval pins demonstrate the materialize-then-copy hand-off without requiring image-processing deps in every CI job.

Per [Standards layer](../../../references/emery-runtime/standards-layer-snippet.md), standards findings may block CI but never transition plan entries, slices, or changes. CI wiring is consumer-project policy, not adapter policy; this prompt acknowledges the surface and links out for the contract.

## Finding-ID conventions

- Report-local occurrence IDs: `CRX-1`, `LOG-1`, `GEN-1`, `UNI-1`, `NEW-1`. These are **report-local** counters — the `id` field on a structured `Diagnostic` (the `Diagnostic` schema uses the equivalent `FIND-0001` shape; this review uses prefixed counters for human triage). They restart in each report and must not be confused with codex `rule-id`s.
- Stable codex citations: `rule_id: VECTIS-001` (for example) appears alongside each mapped finding. Codex ids must match `^VECTIS-[0-9]{3}$`. Markdown `rule_id:` prose maps to the kebab-case `rule-id` field on the `Diagnostic` wire shape (likewise `target-adapter`, `source-adapter`, `related-rule-ids`, `confidence`). Rules: [`adapters/targets/vectis/prose/rules/`](../../../rules/).

Severity values in finding output use the closed `Diagnostic` severity enum: `critical`, `important`, `suggestion`, `optional`.

See [iteration-report.md](../../../references/review/iteration-report.md) § Finding-ID conventions for severity and `file:line` rules.
