# Specify artifact validation checklist

Runtime checklist for source adapters and synthesis self-review. Full artifact reference: [Artifact format](https://specify.augentic.io/reference/artifact-format.html).

## Behavioral specs

- One spec file per domain
- Each spec has Purpose, flat Requirement blocks, stable `ID: REQ-XXX` lines, Scenarios, and Error Conditions
- Specs stay behavioral and avoid platform-binding detail
- Traceability is present for each requirement via stable IDs

## Technical design

- `design.md` captures domain model, APIs, business logic, integrations, and configuration
- Unknowns are marked explicitly with unknown tokens
- Technical decisions live in design, not in specs

## Tasks

- `tasks.md` exists when `/spec:build` depends on it
- Tasks are implementation steps and checkpoints only
- Every task uses numbered checkbox format (`- [ ] X.Y …`) grouped under `## N.` headings

Target-specific artifact checklists live with their owning adapters: the Vectis composition checklist is `targets/vectis/prose/references/composition-checklist.md` and the contracts authoring checklist is part of `targets/contracts/prose/references/artifact-structure.md` (both reachable through each adapter's own `references/`).
