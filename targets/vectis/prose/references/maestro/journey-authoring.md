# Maestro journey authoring (local-dev, post-drain)

Maestro provides **③b shell interaction** feedback — journey taps and navigation — after compile verify (③a) passes. It is **not** part of the build verify gate.

## When to run

Run only after **`emery plan execute` projects `drained`** (every slice `done`). Per-slice Maestro is usually meaningless until the full app exists.

```bash
emery plan status    # must show drained
cargo make maestro-android   # or maestro-ios / maestro-web — booted device required (native)
```

Operators wire this into desk-check runners between **execute** and **finalize**.

## Authoring workflow

```text
requirements + design + composition test_id + contract/ui-strings.yaml
  → add composition test_id + contract string keys + cargo make generate-bindings
  → wire MaestroTestIds / UiStrings into shell UI
  → author .maestro/journeys/ and wire runFlow steps in platform entries
  → (optional) MCP inspect / inline run while drafting
  → commit YAML
  → cargo make maestro-<platform>   # deterministic gate
  → fail → repair shell UI or YAML → re-run
```

### Outcome quality (fact-based)

| | App OK | App broken |
|--|--------|------------|
| **Pass** | Good — journey met | Bad — false negative |
| **Fail** | Bad — wrong assert/selector | Good — bug surfaced |

Do **not** drop failing steps to chase green runs. Use `# GAP: …` only when Maestro cannot express a step (no stable selector) — never because the app is broken.

## File layout

| Path | Role |
|------|------|
| `composition.yaml` inline `test_id` | SSOT for Maestro / accessibility ids |
| `contract/ui-strings.yaml`, `contract/ui-errors.yaml` | SSOT for display copy and error messages |
| `contract/test-ids.yaml` | Exemplar demo seed; product apps receive composition projection at `emery build` |
| `.maestro/config.yaml` | Project config (`entries/**`, tag `ci`) |
| `.maestro/entries/maestro.mobile.yaml` | iOS + Android entry (`appId` + `launchApp` + `runFlow` journeys) |
| `.maestro/entries/maestro.web.yaml` | Web entry (`url` + `openLink` + `runFlow` journeys) |
| `.maestro/journeys/**/*.yaml` | Shared steps referenced by entries |
| `.maestro/scripts/load-*.sh` | Export contract vars for `maestro test -e` |
| `.maestro/scripts/run-maestro.sh` | Single runner invoked by `cargo make maestro-*` |

**One entry per platform.** Do not add feature-named entry files — add journeys under `.maestro/journeys/` and wire them from the platform entry via `runFlow`.

Mobile entry must start with:

```yaml
appId: ${APP_ID}
---
- launchApp:
    clearState: true
```

Web entry uses `url: ${APP_URL}` and `openLink` instead.

Use **`${MAESTRO_…}`** for test ids and **`"${SPLASH_TITLE}"`** etc. for display strings — never hardcode values that exist in `contract/`.

On Android, Maestro `id:` selectors require `testTagsAsResourceId = true` on the root `Surface` (exemplar `ContentView` ships this). Prefer `id:` for test tags; use visible text asserts only for display strings from `contract/ui-strings.yaml`.

## MCP vs CLI

| Phase | Tool |
|-------|------|
| Explore / first draft | Maestro MCP (`inspect_screen`, inline `run`) |
| Gate / dev loop / CI | `maestro test` / `cargo make maestro-*` (committed YAML) |

See Emery insight: CLI = deterministic gate; MCP = authoring aid.

## Platform runners

| Task | Script |
|------|--------|
| `cargo make maestro-android` | `bash .maestro/scripts/run-maestro.sh android` |
| `cargo make maestro-ios` | `bash .maestro/scripts/run-maestro.sh ios` |
| `cargo make maestro-web` | `bash .maestro/scripts/run-maestro.sh web` |

Prerequisites: Maestro CLI on `PATH`, simulator/emulator booted (native), app builds via `make -C <shell> build`.

## Build agents

During slice **build**, agents MAY:

- Edit `contract/*.yaml`
- Run `cargo make generate-bindings`
- Add `.maestro/journeys/` and update `runFlow` steps in `maestro.mobile.yaml` / `maestro.web.yaml`

Agents MUST NOT run `maestro test` inside the Android/iOS verify loop — host device state is not guaranteed mid-build.
