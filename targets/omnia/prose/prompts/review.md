# Omnia target — review prompt

> The omnia adapter core inlines this document into the system prompt of the `review` operation: one engineering-standards review pass over the lent candidate workspace (RFC-90). Applies **engineering standards** with model-assisted judgment: an agent team of three specialists (Security, Correctness, Quality) plus an antagonist; the lead synthesises findings into `$REVIEW_OUTPUT`. This is build-time standards application, not plan approval. **One pass only** — no remediation cycle and no auto-fix: the engine routes blocking findings through the [`repair`](repair.md) operation (origin `review`), re-verifies, and re-dispatches a fresh review under its own budget.

## Working names

Derived from the slice named in the user prompt, matching the build prompt's bindings:

```text
$CRATE_NAME    = slice name with kebab → snake (or the slice's plan-level `crate:` override)
$CRATE_PATH    = crates/$CRATE_NAME
$REVIEW_OUTPUT = $CRATE_PATH/REVIEW.md
```

## Review pipeline

1. **Verify prerequisites** — `$CRATE_PATH` exists in the lent workspace. If the candidate carries no crate tree for the slice, report one blocking finding saying so and stop; review nothing else.
2. **Spawn specialists concurrently** using the verbatim prompts in [`team-protocol-crate.md`](../references/team-protocol-crate.md):
   - Security Reviewer — SEC-prefixed findings.
   - Correctness Reviewer — COR-prefixed findings.
   - Quality Reviewer — QUA-prefixed findings.
   The full check library per specialist (SEC / COR / QUA categories) lives in [`review-categories.md`](../references/review-categories.md).
3. **Universal checks (lead)** — apply every `UNI-*` rule from the shared universal codex pack ([`../rules/universal/`](../rules/universal/), embedded in this adapter and served by the references server) with Omnia / WASM heuristics; prefix `UNI-`. Skip universal checks already covered by SEC / COR / QUA per the table in [`review-categories.md`](../references/review-categories.md).
4. **Adversarial challenge** — forward all findings to the antagonist. The antagonist confirms, upgrades, downgrades, disputes, and may add `NEW-` findings. Protocol: [`team-protocol-crate.md`](../references/team-protocol-crate.md).
5. **Synthesis** — author `REVIEW.md` per the template in [`review-output-template.md`](../references/review-output-template.md). Sections: Summary, Findings (grouped by severity), Adversarial Review (confirmed / downgraded / upgraded / disputed / new tallies), Quality Metrics. Do not apply fixes — repair is a separate engine-dispatched operation.

## Finding-ID conventions

- Report-local occurrence IDs: `SEC-1`, `COR-1`, `QUA-1`, `UNI-1`, `NEW-1`. These ride the phase report's finding `id` field for human triage; the engine renumbers report-local ids and recomputes fingerprints.
- Stable codex citations: `rule-id: OMNIA-002` (for example) appears alongside each mapped finding. Omnia-specific rules live under [`../rules/`](../rules/): `OMNIA-001` Provider-Only Host Access, `OMNIA-002` WASM Guest Runtime Constraints, `RUST-001` Classified SDK Errors / No Panic Paths, `SEC-001` Host-Managed Secrets and Identity. All codex ids are three digits and match `^(UNI|SRC|FRAME|RUST|IFACE|SEC|OMNIA|VECTIS|ORG)-[0-9]{3}$`.
- Severity uses the closed diagnostic severity enum: `critical`, `important`, `suggestion`, `optional`. Antagonist adjustments rewrite the displayed severity but preserve the original prefix and occurrence ID.
- Every finding carries a `location` (`path` + `line`) and `evidence` of `kind: snippet` with a verbatim code excerpt.

## § Standards review surface

This pass writes `$REVIEW_OUTPUT` (`REVIEW.md`) — the model-assisted surface: specialist + antagonist judgment per [`team-protocol-crate.md`](../references/team-protocol-crate.md), applying the engineering-standards rules shipped under [`../rules/`](../rules/) (the Omnia overlay plus the shared `UNI-*` pack at `rules/universal/`).

Per [Standards layer](../references/emery-runtime/standards-layer-snippet.md), standards findings may block CI but never transition plan entries, slices, or changes. CI wiring is consumer-project policy, not adapter policy; this prompt acknowledges the surface and links out for the contract.

## Report

Answer with one phase report carrying the confirmed findings from synthesis — every severity, not just blocking ones. Each finding uses the full diagnostic shape: `id` (the prefixed occurrence ID), `rule-id` when it cites a codex rule, `title`, `severity`, `source: model-assisted`, `kind: violation` (or `review` for a pure request-for-judgment), `artifact` (`code` / `tests` / …), `location`, `evidence` (`kind: snippet`), `impact`, `remediation`, and `confidence`. Findings the antagonist disputed and downgraded to non-issues are documented in `REVIEW.md` only.

- `outcome: completed`, `source: model-assisted`.
- **Empty `outputs`**, **no `ui-surface`** — only `build` declares them.
- `written[]`: one `root: workspace` entry for `$REVIEW_OUTPUT`.
- No continuation payload.

Blocking (`critical` / `important`) findings fail the review gate engine-side; there is no adapter-selected success or failure and no remediation here. Never transition the slice lifecycle.

## See also

- [`review-categories.md`](../references/review-categories.md) — full SEC/COR/QUA/UNI check library, Omnia/WASM heuristics, codex `rule-id` mapping guidance.
- [`team-protocol-crate.md`](../references/team-protocol-crate.md) — verbatim specialist spawn prompts, antagonist protocol, synthesis rules.
- [`review-output-template.md`](../references/review-output-template.md) — `REVIEW.md` template and finding-ID conventions.
- [`agent-teams.md`](../references/agent-teams.md) — shared team roles, antagonist protocol, file ownership.
- [`../rules/`](../rules/) — Omnia-specific rules cited as `rule-id` values; [`../rules/universal/`](../rules/universal/) — the shared `UNI-*` rules (`read_doc` under `rules/universal/`).
