# Vectis review — core

Inlined by the adapter core into the engine-dispatched `review` operation's system prompt (alongside [../../review.md](../../review.md) and the in-scope shell review prompts). Scope: the Rust `shared` crate. Drives an agent team — three specialists plus an antagonist — through **one review pass**. Review reports; it never remediates or loops — blocking findings route through the engine's bounded `repair` dispatch, and the engine re-verifies afterwards.

The shared agent-team protocol lives in [`../../../references/agent-teams.md`](../../../references/agent-teams.md); the core-specific team-spawn protocol lives in [`review/team-protocol-core.md`](../../../references/review/team-protocol-core.md).

## Pipeline

1. **Verify prerequisites** — `${PROJECT_DIR}/shared/` exists (skip the core team when it does not; the shell reviews may still run).
2. **Spawn specialists concurrently** with the verbatim prompts in [`review/team-protocol-core.md`](../../../references/review/team-protocol-core.md):
   - **Structural** — CRX-001..011: missing `render()`, serde derives, input validation, `PendingOp` timestamps, ViewModel typing, unused deps. Full library: [`review/crux-checks.md`](../../../references/review/crux-checks.md).
   - **Logic** — LOG-001..010: state-machine completeness, op coalescing, concurrent conflicts, temporal ordering, rapid-action sequences, spec gaps (validation only — LOG-007), spec-to-test coverage, stale tests, open-GAP inventiveness (LOG-010). Full library: [`review/logic-checks.md`](../../../references/review/logic-checks.md). Open-GAP contract: [`open-gap-contract.md`](../../../references/open-gap-contract.md).
   - **Quality** — GEN-001..013: no `unwrap` / `expect` outside test setup, no debug output, no hardcoded secrets, error propagation, match exhaustiveness, function length, no inline lint suppressions (**GEN-013**, codex `VECTIS-009`). Full library: [`review/general-checks.md`](../../../references/review/general-checks.md).
3. **Universal checks (lead).** Apply every `UNI-*` rule from the shared universal codex pack ([`../../../rules/universal/`](../../../rules/universal/), embedded in this adapter) with Rust / Crux heuristics. Full library: [`review/universal-checks.md`](../../../references/review/universal-checks.md). Skip universal checks already covered by the specialists per the dedupe table in [`review/team-protocol-core.md`](../../../references/review/team-protocol-core.md).
4. **Adversarial challenge.** Forward all findings to the antagonist. The antagonist confirms, upgrades, downgrades, disputes, and may add `NEW-` findings. Protocol: [`agent-teams.md`](../../../references/agent-teams.md).
5. **Synthesis.** Lead authors the review report per [`review/review-report.md`](../../../references/review/review-report.md).
6. **No fixes of any kind.** Mechanical and design-level findings alike are reported, never applied here — even a missing serde derive is a finding, not an edit. Classify each finding `code-fix` or `spec-change`; the engine routes blocking findings through its `repair` dispatch.

## Standalone vs orchestrated

The core reviewer has no orchestrated mode — when design-level findings accumulate it always returns them for consolidation by the parent review prompt. Per-platform shell reviewers ([`../ios/review.md`](../ios/review.md), [`../android/review.md`](../android/review.md)) honour the `orchestrated: true` flag.

## § Consolidate review findings

When all in-scope reviews complete:

1. **Merge findings.** Combine `design_findings` from each reviewer into a single list. Deduplicate universal findings (UNI-prefixed) that both reviewers flagged with identical check IDs and matching evidence — keep the higher-severity instance. Platform-specific findings (CRX-, LOG-, GEN-, IOS-, SWF-, AND-, KTL-, INT-prefixed) are always distinct.
2. **Empty list.** Skip the rest of this section.
3. **Validate classifications.** Each finding already carries `code-fix` or `spec-change`. Treat that as the source of truth. Resolve disagreements between platforms by applying: spec is clear but code is wrong → `code-fix`; spec is silent, ambiguous, or problematic → `spec-change`. For **LOG-010** (open-GAP inventiveness): default `code-fix` (revert to stub or perform B′ closure); use `spec-change` only when Evidence blocks honest closure — see [`open-gap-contract.md`](../../../references/open-gap-contract.md). Intentional open GAP + stub-faithful code/tests is not a finding.
4. **Surface findings.** The consolidated list becomes the phase report's `findings[]` (the mapping lives in the parent review prompt). Cross-platform follow-up work is queued as a new slice via the operator's normal `/emery:plan` flow rather than letting reviewers spawn slices directly.

## § Standards review surface

The per-platform reviewers (this prompt, [`../ios/review.md`](../ios/review.md), [`../android/review.md`](../android/review.md)) carry the model-assisted surface — specialist + antagonist judgment per [`agent-teams.md`](../../../references/agent-teams.md), applying the engineering-standards rules shipped under [`../../../rules/`](../../../rules/) (the Vectis overlay plus the shared `UNI-*` pack at `rules/universal/`).

Vectis render-by-`kind` drift ([`VECTIS-006`](../../../rules/VECTIS-006-asset-render-by-kind.md)) is review-scoped in v1: iOS and Android Integration specialists run **IOS-020** / **AND-028** in every review pass (see per-platform review prompts and team protocols).

Framework acceptance fixtures under `quality/fixtures/reference/targets/vectis/` version-control `design-system/assets/exports/` (see [`task-list/design-system/`](https://github.com/augentic/emery/tree/main/quality/fixtures/reference/targets/vectis/task-list/design-system)) so build prompt examples and eval pins demonstrate the materialize-then-copy hand-off without requiring image-processing deps in every CI job.

Per [Standards layer](../../../references/emery-runtime/standards-layer-snippet.md), standards findings may block CI but never transition plan entries, slices, or changes. CI wiring is consumer-project policy, not adapter policy; this prompt acknowledges the surface and links out for the contract.

## Finding-ID conventions

- Report-local occurrence IDs: `CRX-1`, `LOG-1`, `GEN-1`, `UNI-1`, `NEW-1`. These are **report-local** counters — the `id` field on a structured `Diagnostic` (the `Diagnostic` schema uses the equivalent `FIND-0001` shape; this review uses prefixed counters for human triage). They restart in each report and must not be confused with codex `rule-id`s.
- Stable codex citations: `rule_id: VECTIS-001` (for example) appears alongside each mapped finding. Codex ids must match `^VECTIS-[0-9]{3}$`. Markdown `rule_id:` prose maps to the kebab-case `rule-id` field on the `Diagnostic` wire shape (likewise `target-adapter`, `source-adapter`, `related-rule-ids`, `confidence`). Rules: [`adapters/targets/vectis/prose/rules/`](../../../rules/).

Severity values in finding output use the closed `Diagnostic` severity enum: `critical`, `important`, `suggestion`, `optional`.

See [review-report.md](../../../references/review/review-report.md) § Finding-ID conventions for severity and `file:line` rules.
