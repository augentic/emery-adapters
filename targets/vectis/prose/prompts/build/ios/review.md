# Vectis review — iOS

Inlined by the adapter core into the engine-dispatched `review` operation's system prompt (alongside [../../review.md](../../review.md) and [../core/review.md](../core/review.md)) when `ios` is in scope. One pass, report only — no fixes. Scope: every Swift file under `${IOS_SHELL_DIR}` plus read-only access to `${PROJECT_DIR}/shared/src/app.rs` and the wired UI input set (`composition.yaml`, `tokens.yaml`, `assets.yaml`).

The iOS-specific team-spawn protocol lives in [`review/team-protocol-ios.md`](../../../references/review/team-protocol-ios.md).

## Pipeline

1. **Verify prerequisites** — `${IOS_SHELL_DIR}` exists with Swift files (skip the iOS team when it does not).
2. **Spawn specialists concurrently** with the verbatim prompts in [`review/team-protocol-ios.md`](../../../references/review/team-protocol-ios.md):
   - **Structural** — IOS-001..019: ViewModel / screen correspondence, effect handlers, token usage, ScrollView hazards, recurring-group component candidates. Full library: [`review/ios-checks.md`](../../../references/review/ios-checks.md).
   - **Quality** — SWF-001..010: concurrency, force unwraps, a11y labels, state management, previews, swiftformat, no inline lint suppressions (**IOS-022**, codex `VECTIS-009`). Full library: [`review/swift-quality-checks.md`](../../../references/review/swift-quality-checks.md).
   - **Integration** — always in scope. Token / asset / composition cross-artifact checks per [`review/team-protocol-ios.md`](../../../references/review/team-protocol-ios.md) § Integration, including **IOS-020** (render-by-`kind`, codex `VECTIS-006`) in [`review/ios-checks.md`](../../../references/review/ios-checks.md).
3. **Universal checks (lead).** Apply every `UNI-*` rule from the shared universal codex pack ([`../../../rules/universal/`](../../../rules/universal/), embedded in this adapter) with Swift heuristics. Full library: [`review/universal-checks.md`](../../../references/review/universal-checks.md).
4. **Adversarial challenge.** Forward all findings to the antagonist per [`agent-teams.md`](../../../references/agent-teams.md).
5. **Synthesis.** Lead authors the review report per [`review/review-report.md`](../../../references/review/review-report.md).
   - Return classified `design_findings` per [../core/review.md](../core/review.md) § Consolidate review findings.
6. **No fixes of any kind.** Accessibility labels, token swaps, and missing `#Preview` blocks are findings, never edits — the engine routes blocking findings through its `repair` dispatch.

## Finding-ID conventions

- Report-local occurrence IDs: `IOS-1`, `SWF-1`, `INT-1`, `UNI-1`, `NEW-1`. These are **report-local** counters — the `id` field on a structured `Diagnostic` (the `Diagnostic` schema uses the equivalent `FIND-0001` shape; this review uses prefixed counters for human triage). They restart in each report and must not be confused with codex `rule-id`s.
- Stable codex citations: `rule_id: VECTIS-101` (for example) appears alongside each mapped finding. Codex ids must match `^VECTIS-[0-9]{3}$`. Markdown `rule_id:` prose maps to the kebab-case `rule-id` field on the `Diagnostic` wire shape (likewise `target-adapter`, `source-adapter`, `related-rule-ids`, `confidence`). Rules: [`adapters/targets/vectis/prose/rules/`](../../../rules/).

Severity values in finding output use the closed `Diagnostic` severity enum: `critical`, `important`, `suggestion`, `optional`.

See [review-report.md](../../../references/review/review-report.md) § Finding-ID conventions for severity and `file:line` rules.
