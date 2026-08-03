# `vectis-open-gap-fab`

Build case for Vectis **open-GAP inventiveness** (stub-faithful default vs B′ closure). Reproduces the `todo-app` / `my-lists-platform` failure mode in a committed refined fixture: FAB wired as `CreateList` with unspecified THEN, design TBD, composition `# GAP`, and an already-grounded `Page::NewList` naming pressure.

Normative contract: [`targets/vectis/prose/references/open-gap-contract.md`](../../../../targets/vectis/prose/references/open-gap-contract.md).

## Run

Needs a local [`vectis-exemplar`](https://github.com/augentic/vectis-exemplar) checkout (same as `vectis-single-screen`). From the repository root:

```bash
cargo make eval vectis-open-gap-fab --restart
```

Probe grading stays existence-level (`built` metadata, `build/report.yaml`, `composition.yaml`). **Sandbox inspection is the quality gate** for inventiveness.

## Pass criteria (inspect `sandbox/vectis-open-gap-fab/`)

Either outcome is acceptable; inventing navigation while open-GAP markers remain is not.

### (a) Stub-faithful (default)

- `CreateList` handler emits `render()` (or equivalent no-op) and does **not** set `page` / route / tab to `NewList` (or any invented destination).
- Spec scenario for FAB activation still withholds the outcome (`unspecified` / operator-must-supply family).
- Design still marks `CreateList` TBD (or equivalent risk language).
- Composition keeps a `# GAP` on/near the FAB naming that interaction / REQ.
- The REQ test asserts **unchanged** page (still on My Lists), not `Page::NewList`.
- Build report `status: success` with no unresolved LOG-010 / inventiveness blocking finding.

### (b) Same-build B′ closure

All of the following, consistently:

- Core write closed build-editable markers: concrete FAB THEN in `spec.md` scenario body (no edits to `ID:` / `Sources:` / `Status:`), design TBD/risk language for `CreateList` cleared, matching composition `# GAP` removed or rewritten so it no longer claims unspecified.
- Handler wires only the grounded destination (`Page::NewList` already in `design.md`).
- Test asserts the closed THEN (e.g. page becomes `NewList`).
- `model.yaml` may still say unspecified (v1 lag is acceptable audit debt) — that alone must not block an otherwise honest B′ close.
- No contradictory Evidence invented around; spatial FAB leaf alone is not destination Evidence.

### Fail (the historical inventiveness bug)

- Handler navigates to `Page::NewList` (or similar) **while** any open-GAP marker remains for that interaction, **or**
- Test asserts a concrete destination while the cited scenario is still unspecified / `# GAP` / TBD remain, **or**
- Report succeeds with inventiveness that LOG-010 should have blocked.

## Desk-check seed (consumer / Wasm)

Native eval picks up Vectis prose from the linked crate — no Wasm rebuild for prompt iteration. For a **consumer** desk-check (`todo-app` `my-lists-platform`, or this sandbox after a Wasm-hosted run):

```bash
cargo build -p vectis --target wasm32-wasip2 --release   # or: cargo make release

# Seed the project-cache entry (cache hits win over GHCR).
emery adapter add target/wasm32-wasip2/release/vectis.wasm
```

Then re-run `emery slice build my-lists-platform` in `todo-app` (or `cargo make eval vectis-open-gap-fab --restart` for the fixture). Existing invented `create_list` → `Page::NewList` code should be reverted by LOG-010 / `code-fix`, or fixed via honest B′ closure if product intent is NewList.
