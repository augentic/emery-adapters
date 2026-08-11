# Maestro MCP inspection mode

Optional post-drain UI probing with **Maestro MCP**. Not the regression path ([`../journey-authoring.md`](../journey-authoring.md)).

**This file** is the mode contract (when, MCP boundary, knowledge rules, capture, forbidden).  
**How to run / spawn:** [`team.md`](team.md). Shared team mechanics: [`../../agent-teams.md`](../../agent-teams.md). Operator index: `$TEMPLATE_DIR/.maestro/README.md` (do not copy this contract there).

## When

Only after **`emery plan status` projects `drained`**, between **execute** and **finalize**. Opt-in. Never inside slice build / verify / repair.

## MCP boundary

| Context | Maestro MCP |
|---------|-------------|
| regression (`cargo make maestro-*`), build / verify / repair / finalize | **off** |
| explicit post-drain inspection run | **on** for probe roles only; Lead + Planner stay off-device |
| closing CLI replay (`maestro test`) | **off** |

## Knowledge

- **Addressing (probe roles):** `${MAESTRO_*}` / `"${STRING_KEY}"` from `ui-contract/`.
- **Product intent:** Planner only — to derive **attack hypotheses**, not a pass/fail script. Do **not** pass expected outcomes to Hammer / Visual as “what must happen.”
- **Judgment core (universal):** crash / ANR / process death; stuck / non-navigable screen; tap/gesture with no observable state change; state corruption from rapid re-tap / rotation / invalid input.
- **App-specific attacks** (BACK exits, navigation contracts, empty-submit rules, …): from the Planner list only — not hardcoded as universal law.
- **Visual:** clipped text, overlap, empty/broken chrome, unusable hit targets → primarily `# GAP: visual` / REVIEW; YAML only when a gesture reproduces it. `assertScreenshot` suites are out of this mode.

## Capture

Durable flows live under `.maestro/journeys/mcp-inspect/` (`run-maestro.sh` picks them up; do **not** also wire into `entries/`).

Selector priority: `${MAESTRO_*}` / `"${STRING_KEY}"` → percentage `point` → absolute coordinate (last resort: inspection-only + `# GAP: no stable selector`). Missing `test_id` on an interactive element is a finding. Keep failing steps — do not chase green.

Lead (MCP off): edit only `mcp-inspect/`; restore dropped fails; fix bad selectors; fold visual `# GAP`s into `REVIEW.md`; append Adversarial Review + confidence + `## Capture log` ([`../../review/review-report.md`](../../review/review-report.md)).

Budget is a time/step box. Session ends with durable YAML + `REVIEW.md` + `cargo make maestro-<platform>`. Promote lasting defects as **new** regression journeys — do not copy inspect YAML into `entries/`.

## Forbidden

- Maestro MCP outside an explicit post-drain inspection run
- Passing expected product outcomes to Hammer / Visual as the pass oracle
- Requiring MCP inspection in **`/emery:finalize`** or hosting it in **merge** postflight
- Wiring `mcp-inspect/*.yaml` into `entries/` (double-run) or absolute-coordinate `# GAP` flows into regression without a stable selector
