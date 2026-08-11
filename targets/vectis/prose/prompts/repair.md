# Vectis target — repair prompt

The adapter core inlines this document into the system prompt of the engine-dispatched `repair` operation. One findings-directed pass: the user prompt carries the typed repair brief (the engine's deterministic projection of the blocking findings from its verification or review gate) plus the origin. Apply the minimum change that clears each finding, then answer with the phase report. The engine re-verifies after this pass; there is no loop here and no stamp writing.

**One pass only.** Never run verification or standards review yourself, never retry beyond confirming a targeted fix compiles (`cargo check` as a smoke gate is fine), never select or anticipate the next operation. Work only from the typed brief — do not reconstruct failures from transcripts or decide which findings "really" count; a brief entry you cannot repair stays reported in your answer.

## Origin

- `origin: verification` — mechanical check failures: `cargo fmt` / `check` / `clippy` / `test`, a shell's `make build`, or deterministic in-guest gate findings (composition validation, catalog cross-reference, canonical-UI drift, stamps, suppression scan).
- `origin: review` — engineering-standards findings from the reviewer teams, usually carrying a codex rule id (`UNI-*`, `CRX-*`, `LOG-*`, `GEN-*`, `IOS-*`, `SWF-*`, `AND-*`, `KTL-*`, `VECTIS-*`).

## Classify before editing

Route each finding to the right fix surface first (full protocol: [`test-spec-mapping.md`](../references/test-spec-mapping.md)):

| Signal | Fix surface |
| --- | --- |
| Error inside `#[cfg(test)] mod tests`, test helpers, or factories | Test code — the expectation or Crux `expect_*` chain is wrong. |
| Error in production code (`app.rs` outside tests), missing types / methods | Core code under `shared/src/`. |
| Assertion mismatch where the *actual* value matches the spec | Test issue — fix the expected value. |
| Assertion mismatch where the *expected* value matches the spec | Code issue — the handler returns the wrong result. |
| Destination-shaped assert while the flow still carries open `# GAP` markers | Revert to stub-faithful asserts per [`open-gap-contract.md`](../references/open-gap-contract.md). |
| Swift / Xcode errors | Shell Swift under `iOS/<APP_NAME>/` — never `iOS/Makefile` or `iOS/project.yml`. |
| Kotlin / Gradle compile errors | Shell Kotlin under `Android/app/src/` — never the Makefile or Gradle DX files. |
| Composition validator / catalog cross-reference / canonical-UI findings | The candidate `composition.yaml` (and `build/component-bindings.yaml`) in the writable artifact stage the user prompt names. |
| DX or version-pin drift (Makefiles, `project.yml`, Gradle files, BoltFFI pack recipes) | Re-copy the drifted paths from `$TEMPLATE_DIR` with identity substitution per [`template-materialize.md`](../references/template-materialize.md) — never hand-invent pins or DX content. |
| Unresolved import / missing crate in `Cargo.toml` | Edit `Cargo.toml`; dependency versions still come from `$TEMPLATE_DIR`. |

## Discipline

- **Minimum change.** Fix what the finding names and nothing else; keep the diff scoped to the files and functions it cites.
- **Structural fixes for lint findings.** Refactor — extract helpers, split match arms, narrow types, underscore unused parameters. Never add or preserve `#[allow]` / `#[expect]`, `swiftlint:disable`, `swift-format-ignore`, `@Suppress`, or any suppression comment (the deterministic suppression scan blocks them).
- **Preserve intent.** Keep test names, `/// Spec:` traceability comments, and assertion intent; adjust only the syntax expressing them. Hard rules stay in force: [`hard-rules-core.md`](../references/hard-rules-core.md), [`hard-rules-ios.md`](../references/hard-rules-ios.md), [`hard-rules-android.md`](../references/hard-rules-android.md).
- **Writes are split by root.** Product-code fixes go under the lent workspace root; candidate slice-artifact fixes go only to the writable artifact stage. The authoritative slice tree is read-only.

## Answer

Answer with the phase report: `outcome: completed`, `written` entries for every file you touched (root `workspace` or `slice`), empty `outputs`, no `ui-surface`. Carry a structured finding only for brief entries this target could not repair, and answer `outcome: not-applicable` only when none of the brief's findings are repairable by this target.
