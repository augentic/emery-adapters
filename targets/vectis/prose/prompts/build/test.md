# Vectis build — Crux tests

Inlined by the adapter core into the **core** write leg (alongside [../build.md](../build.md) and [core/write.md](core/write.md)). Carries Step 5: authoring the Crux tests. This is a generation step — running the four-command check pass (`cargo fmt` / `check` / `clippy` / `test`), findings-directed repair, and the durable `shared/.vectis/verify.ok` stamp all belong to the engine-dispatched `verify` and `repair` operations, never to this leg.

The spec-to-test mapping rules live in [`test-spec-mapping.md`](../../references/test-spec-mapping.md) and the operational runbook lives in [`test-runbook.md`](../../references/test-runbook.md). Open-GAP stub-faithful asserts: [`open-gap-contract.md`](../../references/open-gap-contract.md).

## Step 5 — Crux tests (test-writer body)

Run after [core/write.md](core/write.md) in the same slice. Detect mode from the existing `#[cfg(test)] mod tests` block in `app.rs`:

- No tests yet → **create mode**.
- Tests exist, spec changed → **update mode** (drift detection: diff spec scenarios against existing tests, add tests for new scenarios, update assertions for modified scenarios, flag stale tests for removed scenarios with `// STALE: scenario removed from spec`).

### Inline writer steps

1. **Read inputs.** `${SPEC_PATH}`, `${DESIGN_PATH}`, `${APP_RS}`, plus current-slice open-GAP markers (see [`open-gap-contract.md`](../../references/open-gap-contract.md)). Use spec-to-test mapping rules: one synchronous `#[test]` per scenario, named after the scenario, with a `/// Spec: <domain> > REQ-XXX > Scenario: <scenario>` traceability comment. Full mapping rules: [`test-spec-mapping.md`](../../references/test-spec-mapping.md).
2. **Map scenarios deterministically.** Each `#### Scenario:` block produces exactly one test function. The `**WHEN**` clause becomes the test setup (model state, dispatched events). The `**THEN**` clause becomes assertions over `Command` effects and `view()` output. Stable `REQ-XXX` ID + scenario title is the drift-detection key. Open-GAP scenarios: stub-faithful asserts only ([`open-gap-contract.md`](../../references/open-gap-contract.md)); never invent destinations; still one test per scenario (LOG-008). Do not apply the runbook’s vague-THEN→ViewModel heuristic to explicit unspecified/GAP withholds.
3. **Write tests inside `#[cfg(test)] mod tests`** in `app.rs` (Crux convention — not a separate `tests/` directory). Preserve existing helpers, factory functions, and test style.
4. **Coverage requirements.** Every scenario; every shell-facing `Event` variant; every page transition (Loading → Main, Error → retry); every validation rule; every adapter's happy and error path; factory helpers for repeated setup.
5. **Crux test API.** Synchronous only — never `#[tokio::test]` or any async runtime. Call `update()` directly; inspect `Command` effects; resolve effects with simulated responses (`expect_one_effect()`, `expect_http()`, `resolve()`); assert on model and view-model state (`expect_one_event()`). Patterns: [`crux/testing-patterns.md`](../../references/crux/testing-patterns.md).
6. **Smoke gate only.** You may run `cargo check` to confirm the tests compile, but do not run the full check pass or fix pre-existing failures — the engine-dispatched `verify` operation runs the four-command pass afterwards and its findings route through `repair`. Preserve test names, `/// Spec:` traceability comments, and assertion intent. Open-GAP destination asserts while markers remain contradict the open-GAP contract — write stub-faithful asserts from the start (prefer revert to stub; B′ close only when eligible).
