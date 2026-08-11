# contracts.build

> The contracts adapter core inlines this document into the system prompt of every generation leg — the three format sub-flows and the close-out leg — alongside the leg's own format sub-prompt under [`build/`](build/). Leg sequencing lives in the adapter core (`src/operations.rs`), not here. Build is **generation only** (RFC-90): checking is the engine-dispatched verify phase ([`verify.md`](verify.md)) and findings-directed fixing is the engine-dispatched repair phase ([`repair.md`](repair.md)) — never verify, repair, retry, or loop inside a build leg.

Build authors and imports machine-readable contract artifacts under the slice's staged contract delta across three per-format sub-prompts (`build/json-schema.md`, `build/openapi.md`, `build/asyncapi.md`); each carries an internal author / import intent table that fans out to references under [`../references/<format>/`](../references/).

## Scope

Build writes only the slice's staged contract delta: the `contracts/` directory of the writable artifact stage the engine lends (`$ARTIFACT_STAGE/contracts/` — the user prompt names the concrete path). The stage mirrors the slice tree at `.emery/slices/<slice>/`; the engine promotes staged writes on terminal success. The authoritative slice tree is read-only — never write it directly.

- `contracts/schemas/*.yaml` — reusable JSON Schema payload vocabulary (one named type per file).
- `contracts/http/*.yaml` — OpenAPI 3.1 HTTP / resource-style documents.
- `contracts/messages/*.yaml` — AsyncAPI 3.0 evented / pub-sub / streaming / WebSocket documents.

Progress ticks in the slice's `tasks.md` also belong on the stage (`$ARTIFACT_STAGE/tasks.md`). Build MUST NOT edit the root `contracts/` baseline directly. Baseline updates happen only during `merge` (see [`merge.md`](merge.md)).

## Inputs

The build runs against the build request the CLI prepared at `.emery/slices/<slice>/build/request.yaml`; the adapter core renders its `inputs` manifest into each leg's user prompt as `### input:` sections.

- `inputs.artifacts.proposal` (`proposal.md`) — authorship mode (author vs import), source material, interface scope, producer/consumer roles.
- `inputs.artifacts.specs[]` (`specs/<domain>/spec.md`) — behavioural requirements: endpoints / channels / payloads / errors (one file per `proposal.md ## Domains` entry). Provenance lines tell the build whether the slice is author-driven (`Sources: [intent | <doc-key>]`) or import-driven (`Sources: [<code-or-contract-source>]`).
- `inputs.artifacts.design` (`design.md`) — the format selection (OpenAPI 3.1 / AsyncAPI 3.0 / JSON Schema), file-layout intent, and any cross-contract dependency notes (see [`guidance.md`](guidance.md)).
- `inputs.artifacts.tasks` (`tasks.md`) — progress tracking.
- `inputs.artifacts.additional[]` — the optional `contracts/` subtree the adapter's `metadata` record declares: partial deltas written by a prior pass, present only when the slice already carries them.
- `deferred[]` — the request's build-scope exclusion set (RFC-86a D4): the slice-local requirement id, title, and requirement digest of every deferred gap requirement. These requirements are **out of the build's obligations**: author no contract artifacts (schemas, paths, channels, operations) for them, invent no placeholder definitions, and leave no TODO markers in the delta. Never claim a deferred id in a phase report's coverage declaration (`covered[]`, where the report shape carries one) — the engine's report gate rejects a report that claims coverage of a deferred requirement (`target-build-deferred-covered`). The baseline spec carries the debt; the contract delta carries nothing. Empty (or absent) when nothing is deferred.
- The root `contracts/` baseline — read-only context for `$ref` reuse and extension authoring; outside the request manifest, not a slice delta.

Build consumes the synthesised Emery artifacts as its primary source. Do not treat raw design documentation as the contract source unless the proposal names it as Source Material and the synthesised `specs/<domain>/spec.md` files have captured the required behaviour.

## Algorithm

### Phase 1 — Classify

Identify the authorship mode from `proposal.md`: author-from-specs, import-existing-contracts, modify-existing-contracts, extract-from-source-code, or mixed. Then classify required formats from `design.md`: JSON Schema (reusable payload vocabulary), OpenAPI 3.1 (HTTP / resource), AsyncAPI 3.0 (evented / pub-sub / streaming / WebSocket).

### Phase 2 — Author or import (fixed format order)

The adapter core runs the format sub-flows in this fixed order — the schema vocabulary is shared and must stabilise before the bindings reference it. Each leg's system prompt appends the owning format sub-prompt:

1. **[build/json-schema.md](build/json-schema.md)** — author or import the minimal JSON Schema delta for reusable payload vocabulary. Owns `$id` assignment, one-type-per-file decomposition, and schema-file naming. Skip when the slice has no shared payload schemas.
2. **[build/openapi.md](build/openapi.md)** — author or import the minimal OpenAPI delta for HTTP / resource interactions. Reuse change-local or baseline `contracts/schemas/` files; do not author competing schemas under different filenames or `$id`s. Skip when the slice has no HTTP interactions.
3. **[build/asyncapi.md](build/asyncapi.md)** — author or import the minimal AsyncAPI delta for evented / pub-sub / streaming / WebSocket-style interactions. Follow the same schema-reuse rule. Skip when the slice has no evented interactions.

Import paths must produce an import report covering lossless changes, lossy changes, unsupported constructs, and manual-review warnings. See [`references/import-upgrade-policy.md`](../references/import-upgrade-policy.md).

Each sub-flow answers the phase-report shape. A format that applies but cannot produce its artifacts — missing source material, an unimportable document, a spec gap — reports what blocked it as a blocking (`important`) finding rather than answering clean; the adapter core folds every leg's findings and writes into the one build report.

**Identity & version.** Every top-level OpenAPI / AsyncAPI document MUST carry a SemVer `info.version` and SHOULD carry a kebab-case `info.x-emery-id`; imports preserve source values verbatim rather than auto-rewriting. The canonical rules live in [`references/contract-identity.md`](../references/contract-identity.md) — the format sub-flows enforce them.

### Phase 3 — Close out

After the format sub-flows, one close-out leg marks the completed task checkboxes in the stage copy of the task list at `$ARTIFACT_STAGE/tasks.md` — never in the authoritative slice tree. Tick only tasks the staged contract delta actually completed; leave everything else untouched.

### No verification in build

Build performs no checking pass. After build returns, the engine dispatches `verify` (one check pass: the adapter's deterministic in-guest contract validator plus the verifier references — see [`verify.md`](verify.md)) and routes any blocking findings to one `repair` dispatch ([`repair.md`](repair.md)). Do not run verifier references, re-enter a sub-flow, or retry inside a build leg.

### No-op behaviour

When the slice's specs describe no API interactions and no Source Material lists importable contract artifacts, every format pass writes nothing and answers `outcome: not-applicable`. The build completes as a clean no-op. This is normal for slices that touch only planning metadata or contract documentation without affecting an API surface.

## Output hygiene

- Only emit `.yaml` files under `$ARTIFACT_STAGE/contracts/`.
- Create `contracts/http/`, `contracts/messages/`, `contracts/schemas/` only when they will contain at least one file.
- Stay inside the stage; baseline `contracts/` and the authoritative slice tree are off-limits to build.

## See also

Every reference below (and the rest of the corpus) is fetchable via the granted MCP references server; fetch on need rather than front-loading.

- [`build/json-schema.md`](build/json-schema.md), [`build/openapi.md`](build/openapi.md), [`build/asyncapi.md`](build/asyncapi.md) — per-format sub-prompts.
- [`verify.md`](verify.md), [`repair.md`](repair.md) — the engine-dispatched checking and repair phases.
- [`references/baseline-vs-delta.md`](../references/baseline-vs-delta.md) — cross-format minimal-delta rules and merge semantics.
- [`references/contract-identity.md`](../references/contract-identity.md) — canonical identity & version rules.
