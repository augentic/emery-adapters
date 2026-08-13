# intent.survey

Emit exactly one lead. The `intent` source is degenerate by construction — it does not crawl, parse, or infer anything. It echoes the operator's intent string into a single lead.

## Inputs

- **Source key** — the plan source-binding key the engine passed on the wire (typically `intent`). The caller stamps each lead's `source` from it; this prompt does not emit it.
- **Inline value** — the operator's free-form intent string. No `$SOURCE_DIR` is lent.
- **Optional parent lead** — when present, this is a focused survey: return stable child leads under that parent. Inherit parent/focus from the passed record. Intent is degenerate, so a focused call typically returns no children unless the value itself names separately acceptable child boundaries.

The change home and `$PROJECT_DIR` are unreachable. Do not read `plan.yaml`, `leads.md`, `discovery.md`, or `slices/`.

## Unfocused output

Return exactly one top-level lead:

```markdown
### <lead>

- lead: <kebab-slug-from-the-intent>
- synopsis: <inline value, one line, verbatim>
```

Rules:

- `lead` is a stable kebab-case slug derived from the intent string (lowercase, strip punctuation, replace whitespace with `-`). Keep it stable across re-surveys of the same value.
- Do not emit `source`. The caller stamps each lead's `source` from the survey binding.
- `synopsis` MUST be the operator's intent string, verbatim. Collapse internal whitespace to single spaces; do not paraphrase, truncate, or annotate. A multi-line intent MAY stay multi-line when folding to one line would lose discriminating content the operator wrote.
- Do not emit `topics`, `parent`, or `focus` on the unfocused lead. Intent infers nothing.

## Focused output

When a parent lead is passed, return stable child leads under it (or none). Stamp each child's `parent` and `focus` to the focused lead id. Inherit the parent's synopsis as context; do not look the parent up in `leads.md`.

## Worked example

Input — inline value:

```
Add a search filter to the user list.
```

Unfocused output:

```markdown
### add-search-filter

- lead: add-search-filter
- synopsis: Add a search filter to the user list.
```

## Notes

- Re-running `intent.survey` against the same value replaces the lead by its `(source, lead)` pair. Editing the intent string and re-running yields the same lead with an updated synopsis when the slug is unchanged.
- The caller persists the catalog; this prompt never writes `leads.md` or `discovery.md`.
- See [From sources to slices](../references/emery-runtime/reconciliation.md#plan-time-leads-become-slices) for how leads reconcile into slices.
