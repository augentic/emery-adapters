# contracts.merge

Merge prompt for slices that target the `contracts` adapter — the contracts adapter core inlines this document into the system prompt of the postflight repair leg. The engine dispatches the adapter's merge operation twice around its deterministic core merge: `preflight` before the engine promotes the slice's `contracts/` delta into the root `contracts/` baseline, `postflight` after the commit and archive. Delta promotion, baseline coherence validation, the lifecycle transition, and the archive move all stay with the engine; the contracts target adds **two deterministic validator gates** around that commit, with one bounded repair leg on the postflight side. Every artefact under `specs/` and `contracts/` is promoted by the engine's deterministic merge.

## Preflight — staged delta validation

The preflight dispatch is fully deterministic: the adapter runs its compiled-in contract validator against the slice's staged delta (`.emery/change/slices/<slice>/contracts/` in-place) and answers without a judgment leg. Blocking findings mean `status: failure`, and the engine aborts the merge with the slice still at `built` — the same delta the build phase already validated, re-checked so drift between build and merge cannot land.

## Postflight — merged-baseline validation

After the engine has promoted the slice's `contracts/` delta into root `contracts/`, the adapter runs the same deterministic validator against the now-updated baseline, with one bounded repair leg; residual findings force `status: failure`.

The validator enforces the contract validation rules across every top-level OpenAPI 3.1 / AsyncAPI 3.0 document under the lent workspace's `contracts/` (the merged baseline lives in the workspace tree, not the operator checkout):

- `contract.version-is-semver` — `info.version` parses as SemVer per [semver.org](https://semver.org).
- `contract.id-format` — when `info.x-emery-id` is present, the value matches `^[a-z][a-z0-9-]*$` and is ≤ 64 characters.
- `contract.id-unique` — every present `info.x-emery-id` is unique across the baseline.

The JSON envelope is the canonical shape callers parse. Field reference (matches the verifier siblings' [`cross-project` mode](../references/openapi/verifier.md#cross-project-mode)):

```json
{
  "envelope-version": 2,
  "contracts-dir": "<absolute-baseline-path>",
  "ok": false,
  "findings": [
    { "path": "contracts/http/user-api.yaml", "rule-id": "contract.id-unique", "detail": "..." }
  ],
  "exit-code": 1
}
```

When the slice does not touch `contracts/` at all (e.g. a planning-metadata-only contracts slice), the postflight validator still runs — the baseline as a whole must remain well-formed, and the check is cheap on a clean baseline. An absent `contracts/` directory validates clean (there are no top-level documents to walk).

If the operator pipeline (CI annotations, dashboards) needs to re-surface envelope findings as `Diagnostic` records (see `schemas/diagnostics/diagnostic.schema.json` and [`../references/report-shape.md`](../references/report-shape.md#relationship-to-diagnostic)), the mapping is: `findings[].rule-id` → `rule-id`, `findings[].path` → `location.path`, `target-adapter: contracts`, and the contract-domain payload (`detail`, any compatibility classification such as `additive` / `breaking` / `ambiguous` / `unverifiable`) lives inside `evidence.kind: structured` with the contract data under `evidence.data`. The closed `Diagnostic` severity enum (`critical` / `important` / `suggestion` / `optional`) is separate from any compatibility classification — classifiers remain contract-domain evidence fields.

The postflight gate intentionally validates the merged baseline, not the staged delta, because cross-repo id uniqueness only resolves once the deltas are promoted; the preflight gate covers the staged side.

## Postflight repair leg

When the postflight validator reports blocking findings, one bounded repair leg receives this prompt plus the findings: repair the merged `contracts/` baseline files in place (the collision-shaped fixes — usually an `x-emery-id` rename or a version correction), then answer with the corrected report body. The validator re-runs deterministically after the answer; residual findings force `status: failure`.

## Failure semantics

A blocking preflight finding aborts the merge before anything is promoted: the slice stays `built` and the plan entry stays `in-progress`. A blocking postflight finding is a terminal diagnostic, not a park: the engine has already committed and archived the merge, so the report surfaces the regression for a follow-up repair slice — never attempt to roll back the merge or transition the lifecycle from this prompt.

### Consumer-project pin updates

When the slice's contributions need to flow into downstream consumer projects (per the registry's workspace clones), publish the prepared workspace branches **after** the postflight gate clears:

1. `emery workspace push` — push the workspace clones' branches that already received the merged contract deltas.
2. Operator PR merge — review and merge those PRs through the forge UI, `gh pr merge`, or the team's normal merge queue.

Pin updates that surface drift the merge cannot auto-resolve (e.g. a consumer's workspace clone has uncommitted local edits, a consumer project is offline, or `workspace push` reports `no-branch` because the clone is not on the prepared `emery/<change-name>` branch) require operator reconciliation and stay operator-owned.
