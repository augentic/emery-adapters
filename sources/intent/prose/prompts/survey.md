# intent.survey

Emit exactly one lead — the intent string. The `intent` source is degenerate by construction — it does not crawl, parse, or infer anything. It echoes the operator's intent string as the single lead. The engine persists the lead; this prompt returns the lead block only.

## Inputs

- `Source` — the source binding bound to this adapter. `Source.value` carries the operator's free-form intent string. `Source.path` is absent for `intent` bindings; no filesystem root is preopened.

## Output contract

Return exactly one lead block:

```markdown
### <lead>

- lead: <lead>
- synopsis: <Source.value, one line, verbatim>
```

Rules:

- `lead` is a stable kebab-case slug derived from the intent string itself: take the intent's salient verb-plus-object phrase, lowercase it, strip punctuation, and join with `-` (keep it compact — two to five words). Re-running against the same intent string MUST yield the same slug; stability matters more than prettiness.
- Do not emit `source`. The engine stamps each lead's `source` from the survey binding (the key the source was registered under, typically `intent`); attribution is engine-owned.
- `synopsis` MUST be the operator's intent string, verbatim. Collapse internal whitespace to single spaces; do not paraphrase, truncate, or annotate. A multi-line intent MAY stay multi-line when folding to one line would lose discriminating content the operator wrote.
- Do not emit `topics`. The optional per-lead `topics` field is for sources that read and classify material; `intent` infers nothing, so it always omits the bullet.

## Worked example

Input — the binding's inline `value`:

```
Add a search filter to the user list.
```

Output — one lead block:

```markdown
### add-search-filter

- lead: add-search-filter
- synopsis: Add a search filter to the user list.
```

## Notes

- Re-running `intent.survey` against the same source replaces the lead by its `(source, lead)` pair. Editing the intent string and re-running yields a lead with an updated synopsis (and, when the salient phrase changed, a new slug).
- Downstream consumers group or correlate leads themselves; this prompt only emits the single intent lead.
