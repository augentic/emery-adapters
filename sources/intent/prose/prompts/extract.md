# intent.extract

Emit one `Evidence` document carrying a single `kind: intent` claim. The `intent` source is degenerate — `extract` returns the operator's intent string verbatim, with `authority: intent` so downstream synthesis treats it as the highest-priority signal under the authority precedence (`intent > documentation > behaviour`) defined in [`authority.md`](../references/emery-runtime/synthesis/authority.md). Core synthesis reconciles it with any sibling sources' Evidence into the slice's `spec.md` — see [From sources to slices](../references/emery-runtime/reconciliation.md#slice-time-evidence-becomes-a-spec).

## Inputs

- **Terminal lead** — the catalog lead the engine passed on `input.focus` (id, synopsis, optional parent/focus). For the degenerate intent path the lead id is the kebab slug `survey` derived from the value. Do not look it up in `leads.md` or `slices/`.
- **Inline value** — the operator's intent string, verbatim. No `$SOURCE_DIR` is lent.
- **Source key** — the plan source-binding key the engine passed on the wire (typically `intent`).

The change home and `$PROJECT_DIR` are unreachable. Do not read `plan.yaml`, `leads.md`, or `slices/`.

## Output contract

Return one `Evidence` document. The caller persists it; this prompt returns the body only:

```yaml
authority: intent
lead: <lead>
claims:
  - id: <lead>
    kind: intent
    statement: "<inline value, verbatim>"
```

The document's `(slice, source)` identity is path-borne — the caller persists it and stamps the source from the binding. Neither is written in-document.

Rules:

- `authority` MUST be the literal string `intent`. The `intent` adapter is the only first-party source that emits this authority class; `documentation` and code adapters emit `documentation` or `behaviour` per the authority hierarchy.
- `lead` MUST equal the terminal lead's id (not a slice name; the two are equal only on the degenerate single-lead path).
- `claims` MUST contain exactly one entry with `kind: intent`, an `id:` set to the lead id, and a `statement:` field carrying the operator's intent string verbatim. The `id` is the stable anchor synthesis references — although the Evidence schema only *requires* it on `requirement` / `criterion` / `example` kinds, an id-less intent claim is unanchorable, so the slice's sole requirement would render an empty `Sources:` line and fail `emery slice validate` (`spec.requirement-sources-empty`). Setting `id` equal to the lead keeps the document deterministic and idempotent.
- Do not emit a `path:` on the claim. The intent source has no filesystem locus; `path` is reserved for file-backed sources.
- Do not emit additional claims. Operators who want multi-claim intent split the work into multiple slices (the lead per slice handles that).

## Worked example

Input:

- Terminal lead = `add-search-filter`
- Source key = `intent`
- Inline value = `Add a search filter to the user list.`

Output — `Evidence` document:

```yaml
authority: intent
lead: add-search-filter
claims:
  - id: add-search-filter
    kind: intent
    statement: "Add a search filter to the user list."
```

## Notes

- Empty `claims: []` is schema-valid for sources with nothing to say, but the intent adapter is never legitimately empty — the lead exists because the operator supplied an intent string. Treat an empty value as an extract failure and stay `refining` per §Extraction reliability.
- Re-running `intent.extract` is idempotent: the same `(lead, value)` pair yields a byte-identical Evidence document.
