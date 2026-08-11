# contracts.verify

> The contracts adapter core inlines this document into the system prompt of the verify phase's single model leg. The engine dispatches `verify` exactly once per round (RFC-90): one check pass over the lent candidate, one phase report, no loop. The adapter's deterministic in-guest contract validator (`contract.version-is-semver`, `contract.id-format`, `contract.id-unique`) runs beside this leg in adapter code and its findings are merged into the same report — a verify report with a model leg is therefore `source: hybrid`; when only the in-guest validator ran, the adapter reports `deterministic` without spending this leg.

## One pass, read-only

- Run the verifier reference of each format that owns artifacts in the staged contract delta (the user prompt names its path); skip formats with no staged artifacts.
- This pass is read-only. Do not create, modify, or delete any file — in the staged delta, the workspace, or anywhere else.
- Report findings and stop. The engine routes blocking findings to one `repair` dispatch and re-verifies itself; never fix files, re-enter a build sub-flow, or re-run checks from this pass.

## What to check

Each format's verifier reference owns its complete algorithm — run it in `mode: single` against the staged delta, with the baseline as read-only cross-reference context:

- [`references/json-schema/verifier.md`](../references/json-schema/verifier.md) — `contracts/schemas/` (`$ref` resolution, metadata completeness, duplicate-`$id` collisions, cross-format consumer compatibility).
- [`references/openapi/verifier.md`](../references/openapi/verifier.md) — `contracts/http/` (`$ref` resolution, schema metadata completeness, binding coverage).
- [`references/asyncapi/verifier.md`](../references/asyncapi/verifier.md) — `contracts/messages/` (`$ref` resolution, message metadata completeness, binding coverage).

For mixed-format slices, also check cross-format `$ref` consistency and duplicate schema identities. The identity & version rules ([`references/contract-identity.md`](../references/contract-identity.md)) are enforced deterministically by the in-guest validator; do not suppress or restate what it already reports. Cross-repo id uniqueness is **not** this pass's job — it is the merge gate's (see [`merge.md`](merge.md)).

## Report

Answer with the phase report: `outcome: completed`, `source: model-assisted` (the adapter core merges the deterministic validator's findings and labels the merged phase `hybrid`), empty `outputs`, no `ui-surface`, no writes. Map each verifier finding into the structured finding shape per [`references/report-shape.md`](../references/report-shape.md), carrying a `location.path` wherever the verifier names a file. A clean pass answers with an empty `findings` list — never invent findings to look thorough, and never omit a real one.
