# Vectis review — Android

Inlined by the adapter core into the engine-dispatched `review` operation's system prompt (alongside [../../review.md](../../review.md) and [../core/review.md](../core/review.md)) when `android` is in scope. One pass, report only — no fixes. Scope: every Kotlin file under `${ANDROID_SHELL_DIR}` plus read-only access to `${PROJECT_DIR}/shared/src/app.rs` and the wired UI input set (`composition.yaml`, `tokens.yaml`, `assets.yaml`).

The Android-specific team-spawn protocol lives in [`review/team-protocol-android.md`](../../../references/review/team-protocol-android.md).

## Pipeline

1. **Verify prerequisites** — `${ANDROID_SHELL_DIR}` exists with Kotlin files (skip the Android team when it does not).
2. **Spawn specialists concurrently** with the verbatim prompts in [`review/team-protocol-android.md`](../../../references/review/team-protocol-android.md):
   - **Structural** — AND-001..027: screen / ViewModel correspondence, effect handlers, token usage, BoltFFI `CoreFfi` bridge, generated-type imports, coroutine safety, recurring-group component candidates. Full library: [`review/android-checks.md`](../../../references/review/android-checks.md).
   - **Quality** — KTL-001..010: force-unwraps, debug output, coroutine cancellation, Compose state, previews, a11y `contentDescription`, no inline lint suppressions (**AND-029**, codex `VECTIS-009`). Full library: [`review/kotlin-quality-checks.md`](../../../references/review/kotlin-quality-checks.md).
   - **Integration** — always in scope. Token / asset / composition cross-artifact checks per [`review/team-protocol-android.md`](../../../references/review/team-protocol-android.md) § Integration, including **AND-028** (render-by-`kind`, codex `VECTIS-006`) in [`review/android-checks.md`](../../../references/review/android-checks.md).
3. **Universal checks (lead).** Apply every `UNI-*` rule from the shared universal codex pack ([`../../../rules/universal/`](../../../rules/universal/), embedded in this adapter) with Kotlin / Android heuristics. Full library: [`review/universal-checks.md`](../../../references/review/universal-checks.md).
4. **Adversarial challenge.** Forward all findings to the antagonist per [`agent-teams.md`](../../../references/agent-teams.md).
5. **Synthesis.** Lead authors the review report per [`review/review-report.md`](../../../references/review/review-report.md).
   - Return classified `design_findings` per [../core/review.md](../core/review.md) § Consolidate review findings.
6. **No fixes of any kind.** `contentDescription`, token swaps, missing `@Preview` blocks, generated-FFI-type imports, and `CancellationException` rethrows are findings, never edits — the engine routes blocking findings through its `repair` dispatch. When flagging a package-path finding, cite the resolved `ANDROID_PACKAGE`, never a hardcoded `com.vectis.*` fallback.

## Finding-ID conventions

- Report-local occurrence IDs: `AND-1`, `KTL-1`, `INT-1`, `UNI-1`, `NEW-1`. These are **report-local** counters — the `id` field on a structured `Diagnostic` (the `Diagnostic` schema uses the equivalent `FIND-0001` shape; this review uses prefixed counters for human triage). They restart in each report and must not be confused with codex `rule-id`s.
- Stable codex citations: `rule_id: VECTIS-201` (for example) appears alongside each mapped finding. Codex ids must match `^VECTIS-[0-9]{3}$`. Markdown `rule_id:` prose maps to the kebab-case `rule-id` field on the `Diagnostic` wire shape (likewise `target-adapter`, `source-adapter`, `related-rule-ids`, `confidence`). Rules: [`adapters/targets/vectis/prose/rules/`](../../../rules/).

Severity values in finding output use the closed `Diagnostic` severity enum: `critical`, `important`, `suggestion`, `optional`.

See [review-report.md](../../../references/review/review-report.md) § Finding-ID conventions for severity and `file:line` rules.
