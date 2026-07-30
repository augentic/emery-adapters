# Omnia build — standards review (code reviewer)

Loaded by [../build.md](../build.md) for the standards-review leg — the build's final judgment leg, after the verify-repair loop (and capture replay, when the build context binds `captures`). Applies **engineering standards** with model-assisted judgment: an agent team of three specialists (Security, Correctness, Quality) plus an antagonist; the lead synthesises findings into `$REVIEW_OUTPUT = $CRATE_PATH/REVIEW.md`. This is build-time standards application, not plan Gate 1. The leg also closes the build (see `## Build close-out`): its answer carries the findings synthesis and output declaration the adapter assembles the build report from in-guest — there is no separate report leg.

## Review pipeline

1. **Verify prerequisites** — `cargo check` passes (the verify-repair loop already guarantees this) and `$CRATE_PATH` exists. Resolve the optional `fix` flag.
2. **Spawn specialists concurrently** using the verbatim prompts in [`team-protocol-crate.md`](../../references/team-protocol-crate.md):
   - Security Reviewer — SEC-prefixed findings.
   - Correctness Reviewer — COR-prefixed findings.
   - Quality Reviewer — QUA-prefixed findings.
   The full check library per specialist (SEC / COR / QUA categories) lives in [`review-categories.md`](../../references/review-categories.md).
3. **Universal checks (lead)** — apply every `UNI-*` rule from the shared universal codex pack ([`../../rules/universal/`](../../rules/universal/), embedded in this adapter and served by the references server) with Omnia / WASM heuristics; prefix `UNI-`. Skip universal checks already covered by SEC / COR / QUA per the table in [`review-categories.md`](../../references/review-categories.md).
4. **Adversarial challenge** — forward all findings to the antagonist. The antagonist confirms, upgrades, downgrades, disputes, and may add `NEW-` findings. Protocol: [`team-protocol-crate.md`](../../references/team-protocol-crate.md).
5. **Synthesis** — author `REVIEW.md` per the template in [`review-output-template.md`](../../references/review-output-template.md). Sections: Summary, Findings (grouped by severity), Adversarial Review (confirmed / downgraded / upgraded / disputed / new tallies), Auto-Fix Summary (when `fix` is set), Quality Metrics.
6. **Auto-fix (only when `fix`)** — apply safe fixes for confirmed / upgraded auto-fixable findings only. Scope, success-rate table, and revert-on-failure recipe: [`review-auto-fix.md`](../../references/review-auto-fix.md). Re-run `cargo check`; revert on failure. Respect antagonist regression flags.

## Finding-ID conventions

- Report-local occurrence IDs: `SEC-1`, `COR-1`, `QUA-1`, `UNI-1`, `NEW-1`. These are the `id` field on a structured `Diagnostic` (the `Diagnostic` schema uses the equivalent `FIND-0001` shape; this report uses prefixed counters for human triage).
- Stable codex citations: `rule_id: OMNIA-002` (for example) appears alongside each mapped finding. The markdown `rule_id:` prose maps to the kebab-case `rule-id` field on the `Diagnostic` wire shape. Omnia-specific rules live under [`adapters/targets/omnia/prose/rules/`](../../rules/): `OMNIA-001` Provider-Only Host Access, `OMNIA-002` WASM Guest Runtime Constraints, `RUST-001` Classified SDK Errors / No Panic Paths, `SEC-001` Host-Managed Secrets and Identity. All codex ids are three digits and match `^(UNI|SRC|FRAME|RUST|IFACE|SEC|OMNIA|VECTIS|ORG)-[0-9]{3}$`.
- Severity uses the closed `Diagnostic` severity enum: `critical`, `important`, `suggestion`, `optional`. Antagonist adjustments rewrite the displayed severity but preserve the original prefix and occurrence ID.
- Every finding carries a `file:line` reference and a verbatim code snippet.

## Auto-fix scope

Auto-fix applies only to findings the antagonist confirmed or upgraded, and only to the auto-fixable categories listed in the per-category success-rate table in [`review-auto-fix.md`](../../references/review-auto-fix.md). Auto-fix runs after synthesis, before the report is finalised. If `cargo check` fails after a fix is applied, the fix reverts and the finding is left for manual handling.

- **`critical` / `important`** findings not auto-fixed are left for the operator.
- **`suggestion`** findings without an auto-fix are documented as accepted technical debt with rationale.
- **`optional`** findings are reported and require no action.

## Remediation cycle

After auto-fix completes:

1. Parse `$REVIEW_OUTPUT`. Process by severity.
2. **`critical` / `important`** — auto-fixable + not disputed: apply the fix directly. Non-auto-fixable: classify as test issue vs code issue and re-enter the matching writer prompt (back to [crate writer](crate.md) or [test writer](test.md)). After all `critical` / `important` fixes, return to the build prompt's verify-repair loop with max 2 iterations (tighter than the standard 3, since these are targeted repairs).
3. **`suggestion`** — auto-fix when available; otherwise document as accepted technical debt in `REVIEW.md`.
4. **`optional`** — document only.
5. Re-run this review (without `fix`) to verify fix quality. If new `critical` / `important` findings appear, repeat the remediation cycle once.

## Build close-out (absorbed report residue)

After the remediation cycle resolves, close out the build in this same leg — the adapter assembles the build report from this answer in-guest, so no report leg follows:

1. **Mark `tasks.md` checkboxes.** Check off every completed task in the slice directory's `tasks.md`; leave genuinely unfinished tasks unchecked.
2. **Declare outputs.** List the build outputs in the answer's `outputs[]`: the slice's crate tree (`$CRATE_PATH`) and, when this build wrote the guest scaffolding (create mode), the workspace-root guest files — each as `platform: core` with a path relative to the project root. Declare only paths the working tree actually contains; the deterministic report gate fails the build on a declared-but-missing path.
3. **Synthesise findings.** Fold the findings left unresolved after remediation — from `REVIEW.md`, the verify-repair output, and capture replay's classification (its outcome rides the user prompt's `Phase outcomes` block) — into the answer's `findings[]` (`title` / `severity` / `impact` / `remediation`, plus `rule-id` for codex citations). Status is derived, never judged: any `critical` / `important` finding makes the assembled report `status: failure`, so a build that cannot succeed (an exhausted verify-repair budget, unclearable blocking findings, confirmed replay failures) must carry at least one blocking finding. A clean build answers with non-blocking findings only (or none).

## § Standards review surface

This leg writes `$REVIEW_OUTPUT` (`REVIEW.md`) — the model-assisted surface: specialist + antagonist judgment per [`team-protocol-crate.md`](../../references/team-protocol-crate.md), applying the engineering-standards rules shipped under [`../../rules/`](../../rules/) (the Omnia overlay plus the shared `UNI-*` pack at `rules/universal/`).

Per [Standards layer](../../references/emery-runtime/standards-layer-snippet.md), standards findings may block CI but never transition plan entries, slices, or changes. CI wiring is consumer-project policy, not adapter policy; this prompt acknowledges the surface and links out for the contract.

## Build report

The build report is assembled **in-guest** from this leg's schema-gated answer — no report leg is spawned and no report file is written. The answer's `## Build close-out` above carries the report's judgmental residue: the findings left unresolved after the remediation cycle and the declared build outputs. Never transition the slice lifecycle — the deterministic in-guest report gate checks the assembled report's coherence against the working tree and the engine guest owns the `Refined → Built` transition.

**Status is derived, never judged.** The assembled report is `status: success` iff the answer carries no blocking (`critical` / `important`) finding and every declared output exists in the working tree; the deterministic gate adds a blocking finding for any declared-but-missing output. A build that cannot succeed — an exhausted verify-repair budget, unresolved blocking review findings, replay failures the review confirms — must carry at least one blocking finding in the answer.

- **Clean build** — the verify-repair loop passes (`cargo fmt --check`, `cargo check`, `cargo clippy -- -D warnings`, `cargo test`), the remediation cycle leaves no unresolved `critical` / `important` findings in `REVIEW.md`, and replay passes when the build context binds `captures` → an answer with no blocking findings assembles as `status: success`.
- **Unresolved build** — the verify-repair budget is exhausted (3 iterations) or the remediation cycle cannot clear its blocking findings → blocking findings in the answer assemble as `status: failure`.

Each answer finding carries `title`, `severity`, `impact`, and `remediation` (plus `rule-id` when it cites a codex rule); the adapter folds them into the engine's report findings. Map omnia's verify-repair, `REVIEW.md`, and replay findings into that shape.

## See also

- [`review-categories.md`](../../references/review-categories.md) — full SEC/COR/QUA/UNI check library, Omnia/WASM heuristics, codex `rule_id` mapping guidance.
- [`team-protocol-crate.md`](../../references/team-protocol-crate.md) — verbatim specialist spawn prompts, antagonist protocol, synthesis rules.
- [`review-auto-fix.md`](../../references/review-auto-fix.md) — `fix` scope, success-rate table, regression guard, recovery process.
- [`review-output-template.md`](../../references/review-output-template.md) — `REVIEW.md` template and finding-ID conventions.
- [`agent-teams.md`](../../references/agent-teams.md) — shared team roles, antagonist protocol, file ownership.
- [`codex/`](../../rules/) — Omnia-specific rules cited as `rule_id` values.
- [`../../rules/universal/`](../../rules/universal/) — the shared `UNI-*` rules, embedded in this adapter (`read_doc` under `rules/universal/`).
