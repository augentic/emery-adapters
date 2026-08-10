# Sub-agent delegation contract

Each writer / reviewer phase prompt runs in its **own sub-agent** with a clean context window. The operations own their orchestrators: the `build` operation's legs fan out writer sub-agents; the `verify` operation's orchestrating agent runs the check commands itself (no verify sub-agents); the `repair` operation's orchestrating agent applies findings-directed fixes (spawning a scoped repair sub-agent per failure class when useful); the `review` operation fans out reviewer sub-agent teams.

**Inputs (orchestrator → sub-agent):** `task` (one of `core-writer`, `test-writer`, `ios-writer`, `ios-repair`, `android-repair`, `android-writer`, `core-reviewer`, `ios-reviewer`, `android-reviewer`), `arguments` (the parent prompt's standard arguments), `mode` (`create`, `update`, or `repair` — decided by the orchestrator from on-disk inspection), `skip_verification` (true for shell writers; check commands belong to the `verify` operation), `artifact_paths` (paths to `spec.md`, `design.md`, `proposal.md`, regenerated `composition.yaml`, sibling `tokens.yaml` / `assets.yaml` when present, and `components.yaml` when `CATALOG_PATH` exists), `forbidden_paths` (`ios-repair` only — `iOS/Makefile`, `iOS/project.yml`), `allowed_paths` (`ios-repair` only — Swift under `iOS/<APP_NAME>/`, plus `Theme/`, `Components/`, `Assets.xcassets/`; `android-repair` only — Kotlin under `Android/app/src/main/java/`), `orchestrated` (reviewer sub-agents only; signals that the reviewer is running inside the review operation so its `design_findings` should flow into the core review prompt's `## § Consolidate review findings` — reviewers always return `design_findings` for the parent to consolidate, never auto-spawn follow-up slices), `extra_context` (phase-specific: the typed findings brief for repair sub-agents, prior phase warnings).

**Outputs (sub-agent → orchestrator):** `status` (`success` / `failure` / `pending`), `files_modified`, `errors`, `warnings`, `design_findings` (reviewers only; empty list when nothing surfaced).

## Open-GAP closure write carve-out (`core-writer`)

By default, build legs treat `spec.md` / `design.md` as already-synthesised inputs and edit implementation trees only. Under the [open-GAP inventiveness contract](open-gap-contract.md), the `core-writer` sub-agent may additionally edit, in the same core leg and only when closing an eligible open GAP:

- `specs/<domain>/spec.md` **scenario body / THEN prose** (never kernel-rendered `ID:` / `Sources:` / `Status:` lines)
- `design.md` TBD / risk lines for that Event
- matching `composition.yaml` `# GAP` comments (in-place patch after the composition leg)

`model.yaml`, Evidence, and plan docs remain out of bounds. Test / shell / repair sub-agents do not inherit this carve-out.

## Why verify checks are serial; review is parallel

Within the `verify` operation, the iOS check pipeline (`make build` → typegen + `boltffi pack apple` + xcodegen + simulator build) and the Android check pipeline (`make build` → typegen + `boltffi pack android` + `:app:assembleDebug`) both invoke `cargo` against the same shared Rust workspace. Cargo uses a workspace-level lock file, so concurrent invocations serialise on the lock rather than running in parallel — run the per-platform checks one after another. The reviewers are pure code-analysis agent teams; they use different formatters (`swiftformat` vs Kotlin) and never invoke `cargo`, Gradle, or Xcode. With no shared mutable state and no build-tool contention, they are safe to run concurrently.
