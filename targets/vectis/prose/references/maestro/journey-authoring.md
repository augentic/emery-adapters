# Maestro journey authoring

Operator and build-agent reference for authoring Maestro journey YAML after the plan is **drained**. Maestro exercises the built iOS / Android / web shell with real taps and navigation; it is **post-drain desk feedback**, not part of the slice **build verify gate**.

Canonical UI bindings (`test_id`, `ui-contract`, codegen, in-guest verify findings) live in [`canonical-ui-bindings.md`](../canonical-ui-bindings.md). This document covers **when** to run Maestro and **how** to author journeys. Execution commands and template file layout are in `$TEMPLATE_DIR/.maestro/README.md`.

## Verification layers

Vectis separates **build-time** verification from **runtime shell interaction**:

| Layer | Authority | When |
|-------|-----------|------|
| **Build verify gate** | In-guest `verify::run` (`canonical-*` findings), compile assurance (`make build`, verify stamps) | `emery build`, per slice — see [`build/report.md`](../../prompts/build/report.md) |
| **Shell interaction** | Maestro journeys (taps, navigation, visible-text asserts on the running app) | Post-drain only — operator desk-check between **execute** and **finalize** |

Maestro is the second layer. It runs only after the first layer passes for the slices that built the app; it does **not** block `Refined → Built` and agents MUST NOT invoke `maestro test` inside the Android/iOS verify loop (host device state is not guaranteed mid-build).

## When to run

Run only after **`emery plan status` projects `drained`** (every plan entry `done`). Per-slice Maestro is usually meaningless until the full app exists.

```bash
emery plan status    # must show drained
cargo make maestro-android   # or maestro-ios / maestro-web — booted device required (native)
```

Operators wire this into desk-check runners between **execute** and **finalize**.

## Authoring workflow

```text
requirements + design + composition test_id + ui-contract/ui-strings.yaml
  → add composition test_id + contract string keys + cargo make generate-bindings
  → wire MaestroTestIds / UiStrings into shell UI
  → author .maestro/journeys/ and wire runFlow steps in platform entries
  → commit YAML
  → cargo make maestro-<platform>   # deterministic CLI gate
  → fail → repair shell UI or YAML → re-run
```

Committed journey YAML plus `cargo make maestro-*` is the authority; author journeys from the composition `test_id` and `ui-contract` keys, not from screen scraping.

### Outcome quality (fact-based)

| | App OK | App broken |
|--|--------|------------|
| **Pass** | Good — journey met | Bad — false negative |
| **Fail** | Bad — wrong assert/selector | Good — bug surfaced |

Do **not** drop failing steps to chase green runs. Use `# GAP: …` only when Maestro cannot express a step (no stable selector) — never because the app is broken.

## Authoring conventions

**One entry per platform.** Do not add feature-named entry files — add journeys under `.maestro/journeys/` and wire them from the platform entry via `runFlow`.

Mobile entry must start with:

```yaml
appId: ${APP_ID}
---
- launchApp:
    clearState: true
```

Web entry uses `url: ${APP_URL}` and `openLink` instead.

Use **`${MAESTRO_…}`** for test ids and **`"${SPLASH_TITLE}"`** etc. for display strings — never hardcode values that exist in `ui-contract/`.

On Android, Maestro `id:` selectors require `testTagsAsResourceId = true` on the root `Surface` (exemplar `ContentView` ships this). Prefer `id:` for test tags; use visible text asserts only for display strings from `ui-contract/ui-strings.yaml`.

## Build agents

During slice **build**, agents MAY:

- Edit `ui-contract/*.yaml` (strings/errors; product apps must not hand-edit projected `test-ids.yaml` — see [`canonical-ui-bindings.md`](../canonical-ui-bindings.md))
- Run `cargo make generate-bindings`

Journey authoring is **required, not optional, whenever the regenerated `composition.yaml` declares ≥1 `test_id`** (the slice has an interactive UI surface): author one `.maestro/journeys/` flow per `spec.md` scenario and wire each into `maestro.mobile.yaml` / `maestro.web.yaml` via `runFlow`. The trigger is the composition's `test_id` / `event` / screen map — **not** a prose call-out in `design.md` / `tasks.md`. Mobile journeys are shared by iOS + Android; extend the shared entry idempotently rather than authoring twice. Core-only slices (no composition, no `test_id`) have no journeys.

Agents MUST NOT run `maestro test` inside the Android/iOS verify loop — host device state is not guaranteed mid-build.

## See also

- [`canonical-ui-bindings.md`](../canonical-ui-bindings.md) — `test_id` projection, codegen, in-guest verify findings
- [`build/report.md`](../../prompts/build/report.md) — shell verify gate at the report leg
- `$TEMPLATE_DIR/.maestro/README.md` — operator run commands and template strip/keep rules
