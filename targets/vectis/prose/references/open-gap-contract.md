# Open-GAP inventiveness contract

Normative posture for Vectis core write, test, composition, and review when a slice still marks an interaction as unspecified. **Default is stub-faithful.** Concrete navigation or state may ship only when the same core write leg closes build-editable GAP surfaces under the eligibility rules below. Linked from the composition, core-write, test, guidance, and review surfaces; keep phase prompts thin by citing this page.

v1 is **model judgment** over prose markers. There is no deterministic `# GAP` parser gate, and composition schema validation stays name-free.

## Open GAP

An interaction / `Event` is an **open GAP** when **any** of the following holds in the **current slice** artifacts (not archived slices):

1. **Spec scenario marker** — parent scenario THEN (or body) contains phrases in the family: `unspecified`, `operator must supply acceptance criteria`, or equivalent “not evidenced / unanswered” wording that withholds the outcome.
2. **Design TBD marker** — `design.md` marks that Event/handler as `TBD`, `behaviour TBD`, `cannot wire … until`, or lists the navigation target under unanswered risks.
3. **Composition GAP comment** — a `# GAP:` comment on or immediately adjacent to the wired control (FAB, row, etc.) naming that interaction or REQ id.
4. **Missing THEN outcome** — scenario has WHEN but no concrete observable THEN (no page, state, validation, or effect named).

`model.yaml` scenario text is also a detection input: if it still says unspecified for the interaction, treat that as an open-GAP marker for inventory / review unless the core write also closed `spec.md` scenario body in this leg under [Closure eligibility](#closure-eligibility).

**Not** an open GAP (do not conflate with LOG-007):

- Spec is silent on **input validation** / edge cases while the happy-path THEN is concrete → LOG-007 still applies; writers may add common-sense validation.
- Synthesis `[unknown]` in capability matrices — refine-time; out of this build rule except insofar as it landed as unspecified scenario text.

**Closed GAP** — none of (1)–(4) remain for that interaction after the build’s artifact edits.

## Closure eligibility

Engine-correct Option B′: limited same-build closure of **build-editable** surfaces only.

### Artifact write authority

| Surface | Build may edit for GAP closure? | Notes |
| --- | --- | --- |
| `specs/<domain>/spec.md` **scenario body / THEN prose** | **Yes (limited)** | Allowed only under this closure carve-out. Never edit kernel-rendered `ID:` / `Sources:` / `Status:` lines (`slice-spec-provenance-stale`). |
| `design.md` TBD / risk lines for that Event | **Yes** | Already a build-consumed handshake doc; Vectis may clear TBD when wiring. |
| `composition.yaml` `# GAP` comments | **Yes** | Build-owned output. Composition runs first; core writer **patches** matching `# GAP` comments in-place (no composition re-leg required). |
| `model.yaml` | **No** | Synthesis persist tail is the only writer. Scenario text may lag until next refine; `emery slice validate` does **not** gate scenario-body parity (only provenance lines). |
| Evidence / plan docs | **No** | Operator refine / amend. |

### When invent / wire is allowed

Writer **may** implement concrete navigation/state **only if all** hold:

1. **Closes build-editable markers in the core leg** — same core write updates `spec.md` scenario THEN prose to the concrete outcome, removes design TBD/risk language for that Event, and removes or rewrites the matching composition `# GAP` so it no longer claims unspecified.
2. **Destination already grounded** — the target screen / `Route` / `Page` variant **already exists** in current `design.md` (including baseline carried into this slice’s design) **or** is introduced by **other** non-unspecified requirements in this same slice’s spec. Writers must not invent a new destination screen solely to close a GAP.
3. **No contradictory Evidence** — closing must not invent outcomes that contradict remaining Evidence claims under `${SLICE_DIR}/evidence/`; if Evidence still says unanswered, keep stub (operator refine / amend first). Naming pressure alone (`Add list`, `CreateList`, prior-slice hints) is **not** Evidence.
4. **`model.yaml` lag is not a license to skip stub** — if `model.yaml` still says unspecified after a `spec.md`-only close, prefer **stub** unless the writer also closed `spec.md` in this leg **and** eligibility (2)–(3) hold. Document the `model.yaml` lag as acceptable audit debt for v1.

Writer **must refuse** and keep a **stub-faithful** handler when eligibility fails:

- Stub-faithful = emit `render()` (or equivalent no-op side effects already present), **do not** change page/route/tab/domain state that the unspecified scenario left open.
- Composition may still wire `event:` + `# GAP` (surface, don’t invent).

### Sequencing

Build order is unchanged: composition → core → shells → review → final-core-verify → report. Closure patches happen in the **core** leg after composition has already emitted `# GAP` — that is intentional. Closing removes or rewrites the matching `# GAP` in-place; do not re-run the composition leg solely to clear comments.

## Cross-slice / update-mode inventiveness

- Prior-slice **render-only stubs** and archived tasks are **not** a license to invent in a later slice that touches a different screen.
- When update-mode diffs touch handlers for Events that remain open-GAP in **this** slice’s artifacts → keep or restore stub-faithful behaviour.
- Wiring a previously stubbed Event requires **this** slice to satisfy [Closure eligibility](#closure-eligibility) (close markers + grounded destination). Out-of-slice plan docs / later functional docs that are **not** in slice sources do not count as closure.
- Baseline screens from prior merges that this slice does not reference stay carried forward; do not invent new behaviour for them either.

## Test policy

| Artifact state | Allowed asserts | Forbidden |
| --- | --- | --- |
| Open GAP | Effect is render (or documented stub effects); page/route/tab for the unspecified dimension **unchanged**; optional “still on source screen” | Concrete destination (`Page::NewList`, etc.); invented domain mutations; skipping the scenario test entirely |
| Closed in same build | Assert the closed THEN verbatim | Asserting closed behaviour while `# GAP` / unspecified / TBD still present |

- LOG-008 still requires one test per scenario — coverage via stub-faithful asserts, not omission.
- Override `test-runbook.md` “vague THEN → derive from ViewModel” for **open-GAP** scenarios: that heuristic applies only when THEN is vague **but not** an explicit unspecified/GAP withhold.

## See also

- [`../prompts/build/composition.md`](../prompts/build/composition.md) — `# GAP` surface / preserve / close semantics.
- [`../prompts/build/core/write.md`](../prompts/build/core/write.md) — Event inventory and stub vs closure.
- [`../prompts/build/test.md`](../prompts/build/test.md) — stub-faithful asserts.
- [`test-spec-mapping.md`](test-spec-mapping.md) / [`test-runbook.md`](test-runbook.md) — scenario → test mapping carve-outs.
- [`crux/update-change-patterns.md`](crux/update-change-patterns.md) — update-mode: keep stub on open-GAP Events.
- [`review/logic-checks.md`](review/logic-checks.md) — LOG-010 open-GAP inventiveness (when present).
