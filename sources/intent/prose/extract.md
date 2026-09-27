# intent.extract

Emit one `Evidence` document from the operator's free-form brief. The engine reconciles it with every other bound source's Evidence into the specification — see [From sources to a spec](reconciliation.md).

## Inputs

- **Inline value** — the operator's brief, verbatim (no `$SOURCE_DIR` is lent), **or** a one-file tree whose single file's contents are the brief; the message names which and carries the string either way.
- **Source name** — the name the engine passed on the WIT bindings, which the specification cites the source by (typically `intent`).

Nothing outside the bound seam is reachable; extract works only from this value.

## Claim kinds

| Kind | Required body field | When to emit |
|---|---|---|
| `intent` | `statement` | Exactly one: the operator's whole brief, verbatim. `id` equals the source name. |
| `requirement` | `statement` | One per distinct behavioural directive the brief states about the system. |
| `criterion` | `criterion` | Only when the brief itself states an acceptance criterion. |

The verbatim `intent` claim preserves the operator's words for the reviewer. A directive left solely inside it never reaches reconciliation, which joins `requirement` claims alone ([reconciliation.md](reconciliation.md)), so every directive the brief states is also its own `requirement`.

## `id` derivation

- The `intent` claim's `id` is the source name, keeping the document deterministic and idempotent — re-running the same `(name, value)` pair yields a byte-identical Evidence document.
- `requirement` and `criterion` ids follow [claims.md](claims.md), led by the domain concept the directive governs (`session.timeout`, `search.filter`). When the brief overrides something the docs or code also describe, converging on the same id is what lets intent win the group.

## Output contract

Return one JSON object matching the claims schema the request carries:

```json
{
  "claims": [
    { "kind": "intent", "id": "<source-name>", "statement": "<brief, verbatim>" },
    { "kind": "requirement", "id": "<dotted-kebab-id>", "statement": "<one directive, present tense>" }
  ]
}
```

Rules:

- Exactly one `kind: intent` claim, first, carrying the brief verbatim in `statement` — no summarising, no splitting, no grammatical cleanup. The reviewer must see exactly what the operator wrote.
- One `requirement` claim per distinct behavioural directive, in brief order. Quote the operator's wording as one present-tense sentence; do not merge directives or invent ones the brief does not state. A brief that is pure context with no directive yields the `intent` echo claim alone.
- Do not emit a `path:` on any claim. The intent source has no filesystem locus.

## Worked example

Input:

- Source name = `intent`
- Inline value = `Sessions must expire after 30 minutes of inactivity. Add a search filter to the user list.`

Output:

```json
{
  "claims": [
    { "kind": "intent", "id": "intent", "statement": "Sessions must expire after 30 minutes of inactivity. Add a search filter to the user list." },
    { "kind": "requirement", "id": "session.timeout", "statement": "Sessions must expire after 30 minutes of inactivity." },
    { "kind": "requirement", "id": "user-list.search-filter", "statement": "The user list offers a search filter." }
  ]
}
```
