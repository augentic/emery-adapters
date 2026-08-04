# Sub-agent delegation contract

Each writer / reviewer phase prompt runs in its **own sub-agent** with a clean context window. Core test verify-repair still runs inside its phase sub-agent. **iOS and Android shell verify are exceptions:** after each shell writer sub-agent returns, the orchestrator (not a verify sub-agent) runs that platform's verify shell commands; failures spawn platform repair sub-agents that may edit generated UI sources only. The leg's orchestrating agent coordinates the sequence and executes iOS and Android verify shell commands inline.

**Inputs (orchestrator → sub-agent):** `task` (one of `core-writer`, `test-writer`, `ios-writer`, `ios-verify-repair`, `android-writer`, `android-verify-repair`, `core-reviewer`, `ios-reviewer`, `android-reviewer`), `arguments` (the build prompt's standard arguments), `mode` (`create`, `update`, or `repair` — decided by the orchestrator from on-disk inspection), `skip_verification` (true for shell writers; iOS and Android verify commands run from the orchestrator, not verify sub-agents), `artifact_paths` (paths to `spec.md`, `design.md`, `proposal.md`, regenerated `composition.yaml`, sibling `tokens.yaml` / `assets.yaml` when present, and `components.yaml` when `CATALOG_PATH` exists), `forbidden_paths` (`ios-verify-repair` only — `iOS/Makefile`, `iOS/project.yml`), `allowed_paths` (`ios-verify-repair` only — Swift under `iOS/<APP_NAME>/`, plus `Theme/`, `Components/`, `Assets.xcassets/`; `android-verify-repair` only — Kotlin under `Android/app/src/main/java/`), `orchestrated` (reviewer sub-agents only; signals that the reviewer is running inside a build phase so its `design_findings` should flow into the core review prompt's `## § Consolidate review findings` — reviewers always return `design_findings` for the parent to consolidate, never auto-spawn follow-up slices), `extra_context` (phase-specific: `error_output` for repair sub-agents, baseline test log for regression checks, prior phase warnings).

**Outputs (sub-agent → orchestrator):** `status` (`success` / `failure` / `pending`), `files_modified`, `verification` (inline result when the sub-agent ran one), `errors`, `warnings`, `design_findings` (reviewers only; empty list when nothing surfaced).

## Open-GAP closure write carve-out (`core-writer`)

By default, build legs treat `spec.md` / `design.md` as already-synthesised inputs and edit implementation trees only. Under the [open-GAP inventiveness contract](open-gap-contract.md), the `core-writer` sub-agent may additionally edit, in the same core leg and only when closing an eligible open GAP:

- `specs/<domain>/spec.md` **scenario body / THEN prose** (never kernel-rendered `ID:` / `Sources:` / `Status:` lines)
- `design.md` TBD / risk lines for that Event
- matching `composition.yaml` `# GAP` comments (in-place patch after the composition leg)

`model.yaml`, Evidence, and plan docs remain out of bounds. Test / shell / repair sub-agents do not inherit this carve-out.

## Why verify is serial; review is parallel

The iOS verify pipeline (`make build` → typegen + `boltffi pack apple` + xcodegen + simulator build) and the Android verify pipeline (`make build` → typegen + `boltffi pack android` + `:app:assembleDebug`) both invoke `cargo` against the same shared Rust workspace. Cargo uses a workspace-level lock file, so concurrent invocations serialise on the lock rather than running in parallel. The reviewers are pure code-analysis agent teams; they use different formatters (`swiftformat` vs Kotlin) and never invoke `cargo`, Gradle, or Xcode. With no shared mutable state and no build-tool contention, they are safe to run concurrently.
