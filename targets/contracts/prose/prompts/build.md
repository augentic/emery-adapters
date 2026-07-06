# contracts.build

> The contracts adapter core inlines this document into the system prompt of every build leg — the three format sub-flows, the bounded verify-repair loop, and the report — alongside the leg's own format sub-prompt under [`build/`](build/). Leg sequencing lives in the adapter core (`crates/core/src/operations.rs`), not here: each leg's user prompt names the phases of this document to follow.

Build authors and validates machine-readable contract artifacts under the slice-local `contracts/` directory across three per-format sub-prompts (`build/json-schema.md`, `build/openapi.md`, `build/asyncapi.md`); each carries an internal author / import / verify intent table that fans out to references under [`../references/<format>/`](../references/).

## Scope

Build writes only change-local contract deltas under `.specify/slices/<slice>/contracts/`:

- `contracts/schemas/*.yaml` — reusable JSON Schema payload vocabulary (one named type per file).
- `contracts/http/*.yaml` — OpenAPI 3.1 HTTP / resource-style documents.
- `contracts/messages/*.yaml` — AsyncAPI 3.0 evented / pub-sub / streaming / WebSocket documents.

Build MUST NOT edit the root `contracts/` baseline directly. Baseline updates happen only during `merge` (see [`merge.md`](merge.md)).

## Inputs

The build runs against the build request the CLI prepared at `.specify/slices/<slice>/build/request.yaml`; the adapter core renders its `inputs` manifest into each leg's user prompt as `### input:` sections. Every artifact path resolves against `inputs.root` (the slice tree).

- `inputs.artifacts.proposal` (`proposal.md`) — authorship mode (author vs import), source material, interface scope, producer/consumer roles.
- `inputs.artifacts.specs[]` (`specs/<domain>/spec.md`) — behavioural requirements: endpoints / channels / payloads / errors (one file per `proposal.md ## Domains` entry). Provenance lines tell the build whether the slice is author-driven (`Sources: [intent | <doc-key>]`) or import-driven (`Sources: [<code-or-contract-source>]`).
- `inputs.artifacts.design` (`design.md`) — the format selection (OpenAPI 3.1 / AsyncAPI 3.0 / JSON Schema), file-layout intent, and any cross-contract dependency notes (see [`guidance.md`](guidance.md)).
- `inputs.artifacts.tasks` (`tasks.md`) — progress tracking.
- `inputs.artifacts.additional[]` — the optional `contracts/` subtree declared by [`adapter.yaml`](../adapter.yaml): partial deltas written by a prior pass, present only when the slice already carries them.
- The root `contracts/` baseline — read-only context for `$ref` reuse and extension authoring; outside the request manifest, not a slice delta.

Build consumes the synthesised Specify artifacts as its primary source. Do not treat raw design documentation as the contract source unless the proposal names it as Source Material and the synthesised `specs/<domain>/spec.md` files have captured the required behaviour.

## Algorithm

### Phase 1 — Classify

Identify the authorship mode from `proposal.md`: author-from-specs, import-existing-contracts, modify-existing-contracts, extract-from-source-code, or mixed. Then classify required formats from `design.md`: JSON Schema (reusable payload vocabulary), OpenAPI 3.1 (HTTP / resource), AsyncAPI 3.0 (evented / pub-sub / streaming / WebSocket).

### Phase 2 — Author or import (fixed format order)

The adapter core runs the format sub-flows in this fixed order — the schema vocabulary is shared and must stabilise before the bindings reference it. Each leg's system prompt appends the owning format sub-prompt:

1. **[build/json-schema.md](build/json-schema.md)** — author or import the minimal JSON Schema delta for reusable payload vocabulary. Owns `$id` assignment, one-type-per-file decomposition, and schema-file naming. Skip when the slice has no shared payload schemas.
2. **[build/openapi.md](build/openapi.md)** — author or import the minimal OpenAPI delta for HTTP / resource interactions. Reuse change-local or baseline `contracts/schemas/` files; do not author competing schemas under different filenames or `$id`s. Skip when the slice has no HTTP interactions.
3. **[build/asyncapi.md](build/asyncapi.md)** — author or import the minimal AsyncAPI delta for evented / pub-sub / streaming / WebSocket-style interactions. Follow the same schema-reuse rule. Skip when the slice has no evented interactions.

Import paths must produce an import report covering lossless changes, lossy changes, unsupported constructs, and manual-review warnings. See [`references/import-upgrade-policy.md`](../references/import-upgrade-policy.md).

**Identity & version.** Every top-level OpenAPI / AsyncAPI document emitted into `$SLICE_DIR/contracts/` (root key `openapi:` or `asyncapi:`) MUST set an `info.version` value that parses as SemVer per [semver.org](https://semver.org). New top-level contracts SHOULD set `info.x-specify-id` to a kebab-case slug (typically the file stem; `^[a-z][a-z0-9-]*$`, ≤ 64 characters). The author sub-flows enforce both rules; the import sub-flows preserve any source `info.x-specify-id` verbatim and surface non-SemVer `info.version` values as `[manual review required]` rather than auto-rewriting.

### Phase 3 — Verify

Verification runs the verifier intent of each format sub-prompt that owns artifacts in the slice. Run only the formats that produced artifacts; skip the rest. The verifier siblings live under [`references/<format>/verifier.md`](../references/).

For mixed-format slices, the final verifier pass must check cross-format `$ref` consistency and report duplicate schema identities before build can complete. The format verifiers enforce the identity & version rules inline (SemVer `info.version`; kebab-case + ≤64-char `info.x-specify-id` when present; in-slice uniqueness on declared ids). The **cross-repo** uniqueness check is **not** part of build-time verification; it is the merge gate's job (see [`merge.md`](merge.md)).

Run each format's verifier in `mode: single` against the slice directory. The verifier reads slice-local artefacts plus the baseline for binding-coverage cross-references and emits a markdown alignment report. The verifier siblings are read-only — they MUST NOT create, modify, or delete any files.

### Phase 4 — Verify-repair loop (max 2 iterations)

If a verifier reports failures:

1. Re-enter the same format sub-prompt with the verifier output for targeted repair via the same intent that produced the artifact (author or import).
2. Re-run that format's verifier.
3. If still failing after 2 iterations, stop repairing and write the `status: failure` build report described under `## Build report`, mapping the remaining failures as blocking findings. Do not mark the task complete; a failure report parks the slice for human review.

A clean verification pass with zero issues is the expected outcome.

### Phase 5 — Validator gate

Build's final step is the adapter's deterministic contract validator, run in-guest against the slice's `contracts/` delta (`$SLICE_DIR/contracts`) with a bounded repair loop:

- **clean** — slice deltas are well-formed; write the success build report.
- **findings** — the gate feeds them back for repair; re-enter the failing format sub-prompt per Phase 4. Residual findings after the repair budget force a `status: failure` build report; do not mark the task complete.

The validator's finding shape is documented under [`references/report-shape.md`](../references/report-shape.md).

### No-op behaviour

When the slice's specs describe no API interactions and no Source Material lists importable contract artifacts, every format pass produces an empty delta and the verifiers have nothing to check. The build completes as a no-op and still returns a `status: success` build report (see `## Build report`). This is normal for slices that touch only planning metadata or contract documentation without affecting an API surface.

## Build report

When the algorithm resolves, return a schema-valid build report as the answer to the build's report leg (the schema-gated report answer — no report file is written). This is the build's final deliverable. The build legs never transition the slice lifecycle — the deterministic in-guest report gate checks the answer's coherence against the working tree and the workflow guest owns the `Refined → Built` transition.

```yaml
version: 1
slice: <slice-name>     # matches the build request's `slice`
target: contracts@1.0.0    # this adapter at its manifest version
status: success         # or: failure
findings: []            # structured diagnostics; default []
```

**Success vs failure findings rule.** A `status: success` report carries an empty `findings[]` or only non-blocking findings (`suggestion` / `optional`); the deterministic report gate downgrades a `success` report carrying any blocking (`critical` / `important`) finding to `failure`. A `status: failure` report populates `findings[]` with the blocking violations the target can map from the contract validator / verifier output, and leaves `findings: []` when no specifics are mappable.

- **Clean build** — Phase 5 validator gate clean and verifiers clean → `status: success`, `findings: []` (or only advisory `suggestion` / `optional` findings).
- **Unresolved build** — the verify-repair budget is exhausted (Phase 4) or the validator gate leaves residual findings after its repair budget (Phase 5) → `status: failure` with blocking findings mapped where possible.
- **No-op** — the slice describes no API surface → `status: success`, `findings: []`.

Each `findings[]` item validates against `schemas/diagnostics/diagnostic.schema.json` (the structured-diagnostic shape distributed with the CLI; required fields include `id`, `title`, `severity`, `source`, `artifact`, `evidence`, `impact`, `remediation`, `fingerprint`). Map the contract validator's findings (see [`report-shape.md`](../references/report-shape.md)) into that shape, carrying contract-domain detail under `evidence.kind: structured` with `target-adapter: contracts`.

## Output hygiene

- Only emit `.yaml` files under `$SLICE_DIR/contracts/`.
- Create `contracts/http/`, `contracts/messages/`, `contracts/schemas/` only when they will contain at least one file.
- Stay inside `$SLICE_DIR/contracts/`; baseline `contracts/` is off-limits to build.

## See also

- [`guidance.md`](guidance.md) — synthesis-time idiom guidance for the contracts target.
- [`merge.md`](merge.md) — merge prompt, including the post-merge `contract` WASI tool gate.
- [`build/json-schema.md`](build/json-schema.md), [`build/openapi.md`](build/openapi.md), [`build/asyncapi.md`](build/asyncapi.md) — per-format sub-prompts.
- [`references/artifact-structure.md`](../references/artifact-structure.md) — directory layout for root `contracts/`.
- [`references/baseline-vs-delta.md`](../references/baseline-vs-delta.md) — cross-format minimal-delta rules and merge semantics.
- [`references/import-upgrade-policy.md`](../references/import-upgrade-policy.md) — shared framework for the importer siblings.
- [`references/report-shape.md`](../references/report-shape.md) — single-mode markdown, baseline validator JSON, and compatibility report JSON formats.
- [`references/cross-project-compatibility.md`](../references/cross-project-compatibility.md) — archived vocabulary for future consumer-impact reporting; today use the contract WASI verifier reports.
