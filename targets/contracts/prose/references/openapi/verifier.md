# OpenAPI — Verifier

> **When to read this.** Read this when verifying an OpenAPI artefact — invoked by the contracts adapter build prompt in `single` mode after the author or importer sibling produces output, by the contracts adapter merge prompt in `cross-project` mode against the merged baseline (the contracts adapter merge contract), or directly by an operator running validation against an existing artefact. Skip this file when authoring (use [`author.md`](./author.md)) or normalising an external document (use [`importer.md`](./importer.md)).

The verifier is **read-only**. It MUST NOT generate, modify, or delete any files. Its sole output is a list of issues rendered as a validation report.

## Modes

The verifier accepts a `--mode {single, cross-project}` flag. The mode determines the report shape and the exit semantics.

| Mode               | Caller                                         | Trigger                                           | Scope                                                             | Output                                                |
| ------------------ | ---------------------------------------------- | ------------------------------------------------- | ----------------------------------------------------------------- | ----------------------------------------------------- |
| `single` (default) | contracts adapter build prompt in `/spec:build` | Post-author or post-import                        | One slice's `contracts/http/` inside one project                  | Markdown report for the verify-repair loop            |
| `cross-project`    | contracts adapter merge prompt                  | Producer-side merge of an OpenAPI contract change | Walk the merged `contracts/` baseline; enforce contract identity/version validation | Deterministic findings from the adapter's in-guest contract validator |

`single` mode feeds the build's verify-repair loop. `cross-project` mode describes the adapter's deterministic in-guest contract validator — the merge gate runs it itself and surfaces its findings; the verifier does not implement its own cross-baseline check. Both modes share the read-only contract.

`--mode` is an internal flag of the format-specific verifier. Cross-project consumer-impact analysis is deferred until a real consumer workflow exists; this verifier owns deterministic single-slice and merged-baseline checks.

## Inputs

### `single` mode

Inferred from the active slice context — no positional arguments required:

```text
$SLICE_DIR          = .specify/slices/<slice-name>
$CHANGE_CONTRACTS    = $SLICE_DIR/contracts/
$BASELINE_CONTRACTS  = contracts/
$CHANGE_SPECS        = $SLICE_DIR/specs/
```

### `cross-project` mode

Caller passes the merged baseline directory:

```text
$BASELINE_CONTRACTS = $PROJECT_ROOT/contracts   # the merged baseline, post-`specify slice merge`
```

The adapter's in-guest contract validator walks every top-level OpenAPI 3.1 / AsyncAPI 3.0 document under `$BASELINE_CONTRACTS` and enforces the contract identity/version validation rules. No producer / consumer arguments are accepted — the tool's scope is the baseline as a whole.

## Prerequisites

### `single` mode

- The author or importer sibling has completed and produced artefacts under `$CHANGE_CONTRACTS/http/`.
- `.specify/project.yaml` exists (Specify is initialised).

If `$CHANGE_CONTRACTS/http/` does not exist or contains no files, report all checks as passed — there is nothing to verify.

### `cross-project` mode

- The contracts adapter is bound as the slice's target (the validator is embedded in its guest).
- `$BASELINE_CONTRACTS` (`$PROJECT_ROOT/contracts`) is the directory the validator walks. An absent directory validates clean (there are no top-level documents to walk); callers MUST NOT pre-stat the path.

## Single-mode checks

Three independent checks run against `$CHANGE_CONTRACTS/http/` and the schemas it references. Run them in order; collect findings; produce a single markdown report at the end.

### Check 1 — `$ref` resolution

All `$ref` pointers in OpenAPI files must resolve to existing schema files. Resolution scope spans both the slice directory and the baseline:

- `$CHANGE_CONTRACTS/schemas/`
- `$BASELINE_CONTRACTS/schemas/`

For each `.yaml` file in `$CHANGE_CONTRACTS/http/`:

1. Read the file and find every `$ref` value (request bodies, response bodies, parameters, security schemes that reference shared definitions).
2. For each `$ref`, resolve the path relative to the file's location (e.g. `../schemas/user-registration.yaml`).
3. Check whether the resolved target exists in `$CHANGE_CONTRACTS` **or** `$BASELINE_CONTRACTS`. Either is a valid resolution scope.
4. Report any `$ref` that does not resolve.

Report format (one entry per failure):

```
FAIL: contracts/http/user-api.yaml — $ref "../schemas/missing-type.yaml" does not resolve (checked change contracts/schemas/ and baseline contracts/schemas/)
```

`$ref` pointers using `#/components/...` (in-document) are also checked — they must resolve to a sibling key inside the same file. The verifier does not chase external URL `$ref`s; it flags them as `WARN` (cross-format URL refs are out of scope).

### Check 2 — Schema metadata

Every JSON Schema file in `$CHANGE_CONTRACTS/schemas/` referenced by an OpenAPI operation in `$CHANGE_CONTRACTS/http/` must have the required metadata fields defined in [`../../references/json-schema-conventions.md`](../../references/json-schema-conventions.md):

| Field         | Rule                                                                                                                                                                       |
| ------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `$id`         | Present and a valid URI (URN format: `urn:specify:schemas/<name>`).                                                                                                        |
| `title`       | Present and non-empty, matching the type name.                                                                                                                             |
| `description` | Present and non-empty. The placeholder `"[imported — description pending review]"` counts as present but **emits a `WARN`** to surface the gap for the verify-repair loop. |

Report format (one entry per failure):

```
FAIL: contracts/schemas/user-registration.yaml — missing required field "$id"
FAIL: contracts/schemas/error-response.yaml — "description" is empty
WARN: contracts/schemas/user.yaml — "description" is "[imported — description pending review]"; replace before merge
```

### Check 3 — Binding completeness

Every schema that appears as a top-level request body, response body, or parameter shape in a spec scenario must have at least one OpenAPI operation that references it.

Resolution scope for the binding:

- `$CHANGE_CONTRACTS/http/` — operations added by this slice.
- `$BASELINE_CONTRACTS/http/` — operations already in the platform baseline.

#### Determining spec-referenced schemas

Read the `*.md` files under `$CHANGE_SPECS` and identify schemas that the spec mentions as:

- Request body payloads (e.g. "accept a `UserRegistration` payload").
- Response body payloads (e.g. "respond with a `User` payload").
- Parameter shape payloads (rare; usually inline).

Cross-reference these against the schema files in `$CHANGE_CONTRACTS/schemas/` and the operations in `$CHANGE_CONTRACTS/http/` and `$BASELINE_CONTRACTS/http/`.

#### Shared vocabulary exemption

Shared vocabulary types that appear only as `$ref` targets inside other schemas are exempt — they are reusable building blocks, not standalone payloads. A schema qualifies as shared vocabulary if both:

1. It is referenced via `$ref` from within other schema files (not directly from path / response definitions), AND
2. It does not appear as a top-level request / response body in any spec scenario.

Common examples: `error-response.yaml`, `pagination.yaml`.

Report format:

```
FAIL: contracts/schemas/user-registration.yaml — appears as request body in spec scenario REQ-001 but has no OpenAPI path binding
WARN: contracts/schemas/oauth-token.yaml — appears in spec but has no protocol binding (may be shared vocabulary — verify intent)
```

Use `FAIL` when the schema is unambiguously a top-level payload in a spec scenario. Use `WARN` when classification is ambiguous — the verify-repair loop surfaces the warning for human review.

When the slice has **no specs**, skip Check 3 — there are no scenarios to cross-reference. Record this in the report so the build knows the check was deliberately bypassed.

### Check 4 — Identity & version (contract identity/version validation)

For every top-level OpenAPI document under `$CHANGE_CONTRACTS/http/` (root key `openapi:`), enforce the contract identity/version validation rules:

1. **`info.version` MUST parse as SemVer.** Per [semver.org](https://semver.org), including optional prerelease labels (`1.0.0-draft.1`). Missing, non-string, or non-SemVer values are `FAIL`.
2. **`info.x-specify-id` (when present) MUST match `^[a-z][a-z0-9-]*$` and be ≤ 64 characters.** Format violations are `FAIL`.
3. **Within the slice directory, `info.x-specify-id` values MUST be unique.** When two top-level OpenAPI documents in `$CHANGE_CONTRACTS/http/` declare the same id, both are `FAIL`.

The cross-repo uniqueness check (the same id declared by a top-level contract somewhere else under root `contracts/`) is **not** part of single mode — it is the merge-phase gate's job, run by the adapter's in-guest contract validator against the merged baseline (the contracts adapter merge contract). The single-mode skill only flags duplicates inside the slice to keep the verifier deterministic and self-contained.

Report format (one entry per failure):

```
FAIL: contracts/http/user-api.yaml — info.version `2024-01-15` is not valid SemVer
FAIL: contracts/http/billing-api.yaml — info.x-specify-id `Billing-API` must match `^[a-z][a-z0-9-]*$` and be ≤ 64 characters
FAIL: contracts/http/admin-api.yaml — info.x-specify-id `shared` is also declared by contracts/http/legacy-api.yaml in this slice
```

## Single-mode algorithm

1. **Determine scope.**
   - `$CHANGE_CONTRACTS/http/`, `$CHANGE_CONTRACTS/schemas/`.
   - `$BASELINE_CONTRACTS/http/`, `$BASELINE_CONTRACTS/schemas/`.
   - `$CHANGE_SPECS/`.
   - If `$CHANGE_CONTRACTS/http/` is empty or absent, report all checks as passed and stop.
2. **Run Check 1** ($ref resolution) on every `.yaml` file in `$CHANGE_CONTRACTS/http/`.
3. **Run Check 2** (schema metadata) on every `.yaml` file in `$CHANGE_CONTRACTS/schemas/` referenced by an OpenAPI operation.
4. **Run Check 3** (binding completeness) by cross-referencing spec scenarios with OpenAPI operations across change and baseline. Skip if no specs.
5. **Run Check 4** (identity & version) on every top-level OpenAPI document in `$CHANGE_CONTRACTS/http/`.
6. **Collect findings** and produce the markdown validation report.

## Single-mode output format

When issues are found:

```markdown
## Validation Report (HTTP)

### $ref Resolution
- ✗ contracts/http/user-api.yaml — $ref "../schemas/missing-type.yaml" does not resolve
- ✓ 11 of 12 $ref pointers resolve

### Schema Metadata
- ✗ contracts/schemas/user-registration.yaml — missing "description"
- ✓ 5 of 6 schemas have complete metadata

### Binding Completeness
- ✓ All spec-referenced schemas have OpenAPI bindings

### Summary
- **Checks passed:** 1 of 3
- **Issues found:** 2
```

When all checks pass:

```markdown
## Validation Report (HTTP)

All checks passed (12 $ref pointers, 6 schemas, 4 operations verified).
```

`single` mode preserves its existing exit semantics: zero on clean reports, non-zero on read errors.

## Cross-project mode

`cross-project` mode runs **after** a producer's contract change merges. The contracts adapter merge prompt invokes it as the post-merge baseline gate (the contracts adapter merge contract); `/spec:execute` re-uses the same gate per project after a producer-side merge (the workspace execution contract).

The mode describes the adapter's deterministic in-guest contract validator. The verifier sibling does not implement an independent cross-project algorithm — the merge gate runs the embedded validator and consumes its findings directly. The deterministic checks the validator enforces are the contract identity/version validation rules:

- `contract.version-is-semver` — every top-level OpenAPI 3.1 / AsyncAPI 3.0 document's `info.version` parses as SemVer (per [semver.org](https://semver.org), prerelease labels included).
- `contract.id-format` — when `info.x-specify-id` is present, the value matches `^[a-z][a-z0-9-]*$` and is ≤ 64 characters.
- `contract.id-unique` — every present `info.x-specify-id` is unique across all top-level contracts under `$BASELINE_CONTRACTS`.

### Outcomes

- **clean** — no findings; the baseline is well-formed.
- **findings present** — the gate treats the merge as `failure`.
- **validator could not run** — the gate treats the merge as `failure` and journals diagnostics.

### JSON envelope

The validator findings project into a JSON envelope (the shape captured in logs and operator-facing reports):

```json
{
  "envelope-version": 2,
  "contracts-dir": "<absolute-baseline-path>",
  "ok": false,
  "findings": [
    { "path": "contracts/http/user-api.yaml", "rule-id": "contract.version-is-semver", "detail": "info.version `2024-01-15` is not valid SemVer (must parse per semver.org, including optional prerelease labels)" },
    { "path": "contracts/http/billing-api.yaml", "rule-id": "contract.id-unique", "detail": "info.x-specify-id `shared` also appears in contracts/messages/legacy-events.yaml" }
  ],
  "exit-code": 1
}
```

Field semantics:

- `envelope-version` — currently `2`; bumps follow the contract envelope versioning policy. Callers MUST validate this before parsing the rest of the envelope.
- `contracts-dir` — the absolute path the tool walked, echoing the positional argument.
- `ok` — `true` iff `findings` is empty.
- `findings[].path` — repo-relative when the parent of `<baseline-dir>` matches the path's prefix, otherwise absolute. Suitable for verbatim rendering in operator-facing reports.
- `findings[].rule-id` — one of `contract.version-is-semver`, `contract.id-format`, `contract.id-unique`.
- `findings[].detail` — single-line human-readable description.
- `exit-code` — legacy outcome code retained for envelope stability (`0` clean / `1` findings / `2` validator error).

Callers that surface post-merge validator failures (the merge prompt on a blocking finding) parse `findings[]` and include `{ rule-id, path, detail }` triples in the stop hint's `paths` field. The load-bearing finding is typically `findings[0].rule-id` plus a one-line restatement of `findings[0].detail`; the full envelope is captured at the log path referenced in the stop hint.

When a caller re-surfaces an envelope finding as a `LintFinding` (see `schemas/diagnostics/diagnostic.schema.json` embedded in the `specify` binary from [`augentic/specify`](https://github.com/augentic/specify)), the mapping is: `findings[].rule-id` → `rule-id`, `findings[].path` → `location.path`, `target-adapter: contracts`. The contract-domain payload (`findings[].rule-id`, `path`, `detail`, plus any compatibility classification such as `additive` / `breaking` / `ambiguous` / `unverifiable`) lives inside `evidence.kind: structured` with the contract data under `evidence.data`. The closed `LintFinding` severity enum (`critical` / `important` / `suggestion` / `optional`) is separate from any compatibility classification — classifiers remain contract-domain evidence fields, not severity.

### Outcome semantics

| Outcome | Meaning | Gate action |
| --------- | --------- | --------- |
| clean | No findings. | Proceed to the next merge step. |
| findings | One or more findings. | Record `failure`; the merge parks. The slice's deltas remain unmerged. |
| validator error | The validator could not run (unreadable tree, internal error). | Record `failure`; journal the diagnostic. The slice's deltas remain unmerged. |

The mode is **deterministic**: the gate is `validate_baseline` in `contracts-core` ([`core/src/validate.rs`](../../core/src/validate.rs)), embedded in the adapter guest. Repeated invocations against the same baseline produce identical findings.

### Why an in-guest gate?

The contracts adapter owns merge gating through its embedded validator: a deterministic, adapter-owned gate that runs inside the guest without crossing the core boundary or re-introducing concern-specific behavior into engine crates.

The deterministic baseline check is the canonical post-merge gate.

## Edge cases

### `single` mode

| Scenario                                                   | Behavior                                                                                               |
| ---------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| Change directory has no `contracts/http/`                  | Pass — nothing to verify.                                                                              |
| Baseline has HTTP contracts but change does not            | Pass — verifier only checks change-level artefacts.                                                    |
| `$ref` target exists in baseline but not in change         | Pass — baseline is a valid resolution target.                                                          |
| `$ref` target exists in change but not in baseline         | Pass — change-level schemas are valid resolution targets.                                              |
| Mixed resolution: some targets in baseline, some in change | Pass — both directories are valid resolution scope.                                                    |
| No spec files in the slice                                 | Skip Check 3; record the skip in the report.                                                           |
| Schema referenced only via `$ref` from other schemas       | Exempt from Check 3 (shared vocabulary).                                                               |
| Operation uses `components/schemas` (legacy)               | `$ref` resolution still verified inside the document; emit `WARN` recommending importer normalisation. |

### `cross-project` mode

| Scenario                                                   | Behavior                                                                                                                                                    |
| ---------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `$BASELINE_CONTRACTS` is absent                            | The validator treats an absent directory as clean (no top-level documents to walk). |
| `$BASELINE_CONTRACTS` is empty (no top-level contracts)    | The validator reports no findings. Treated as clean.                                                                                                       |
| Top-level contract has non-SemVer `info.version`           | Finding `contract.version-is-semver`; the gate records `failure`.                                                                                   |
| Top-level contract has malformed `info.x-specify-id`       | Finding `contract.id-format` (blocking finding).                                                                                                                     |
| Two top-level contracts share the same `info.x-specify-id` | Finding `contract.id-unique` against each colliding path (blocking finding).                                                                                         |
| YAML file under `$BASELINE_CONTRACTS` is malformed         | Skipped by the validator (the format-verifier owns YAML diagnostics in `single` mode); does not surface as a cross-project finding.                         |

## Guardrails

- **Read-only.** Never create, modify, or delete files. Both modes share this contract.
- Report every issue with the file path and a description of the problem.
- When uncertain whether a schema is shared vocabulary or a standalone payload, use `WARN` rather than `FAIL` (in `single` mode).
- Do not attempt to fix issues — report them. Repair belongs to the author or importer sibling.
- **`cross-project` mode is fatal.** Treat findings and validator errors as `failure` per the contracts adapter merge contract. The merge leg MUST halt; the slice's deltas remain unmerged until the operator resolves the finding.
- Do not re-implement the validator's checks. The verifier sibling's `cross-project` mode is descriptive; the canonical algorithm lives in [`targets/contracts/core/src/validate.rs`](../../core/src/validate.rs).

## Verification checklist

### `single` mode

Before completing the run:

- [ ] All `.yaml` files in `$CHANGE_CONTRACTS/http/` scanned for `$ref` resolution.
- [ ] All `.yaml` files in `$CHANGE_CONTRACTS/schemas/` referenced by HTTP operations checked for `$id`, `title`, `description`.
- [ ] Spec scenarios cross-referenced against OpenAPI bindings (when specs exist).
- [ ] Shared vocabulary exemption applied correctly.
- [ ] Identity & version (Check 4) enforced on every top-level OpenAPI document in `$CHANGE_CONTRACTS/http/`: SemVer `info.version`, kebab-case + ≤64-char `info.x-specify-id` when present, in-change uniqueness on declared ids.
- [ ] Validation report produced with per-check results and summary.
- [ ] No files created or modified.

### `cross-project` mode

Before completing the run:

- [ ] The adapter's in-guest contract validator ran exactly once against `$PROJECT_ROOT/contracts`.
- [ ] Stdout (the JSON envelope) captured for the caller (typically the merge prompt's `--context`).
- [ ] Exit code propagated verbatim to the caller (`0` clean / `1` findings / `2` tool or validator error).
- [ ] No findings re-classified, suppressed, or downgraded — the tool's output is authoritative.
- [ ] No files created or modified.

## See also

- [`../../references/openapi-conventions.md`](../../references/openapi-conventions.md) — OpenAPI 3.1 structure rules.
- [`../../references/json-schema-conventions.md`](../../references/json-schema-conventions.md) — schema metadata rules.
- [`../../references/artifact-structure.md`](../../references/artifact-structure.md) — directory layout for the slice-local delta and the baseline.
- [`../../references/report-shape.md`](../../references/report-shape.md) — single-mode markdown report shape this verifier emits.
- [`../../prompts/merge.md`](../../prompts/merge.md) — merge prompt that owns the post-merge in-guest validator gate and the §Merge and adoption contract three-branch outcome wiring.
- [`author.md`](./author.md) — sibling for spec-driven authoring.
- [`importer.md`](./importer.md) — sibling for normalising external documents.
