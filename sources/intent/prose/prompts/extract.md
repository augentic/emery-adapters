# intent.extract

Emit one `Evidence` document from the operator's free-form brief, with `authority: intent` so the engine treats it as the highest-priority signal under the authority precedence (`intent > documentation > behaviour`) defined in [`authority.md`](../references/emery-runtime/synthesis/authority.md). The engine reconciles it with every other bound source's Evidence into the specification — see [From sources to a spec](../references/emery-runtime/reconciliation.md).

## Inputs

- **Inline value** — the operator's brief, verbatim (no `$SOURCE_DIR` is lent), **or** a one-file tree whose single file's contents are the brief; the message names which and carries the string either way.
- **Source key** — the authored binding key the engine passed on the wire (typically `intent`).

Nothing outside the bound material is reachable; extract works only from this value.

## Claim kinds

| Kind | Required body field | When to emit |
|---|---|---|
| `intent` | `statement` | Exactly one: the operator's whole brief, verbatim. `id` equals the source key. |
| `requirement` | `statement` | One per distinct behavioural directive the brief states about the system. |
| `criterion` | `criterion` | Only when the brief itself states an acceptance criterion. |

The verbatim `intent` claim preserves the operator's words for the reviewer. The `requirement` claims are what deterministic reconciliation joins against other sources: only `requirement` claims form spec requirement rows, so a directive left solely inside the `intent` echo can never override a documentation or code claim — the authority precedence acts through matching `requirement` ids.

## `id` derivation

- The `intent` claim's `id` is the source key, keeping the document deterministic and idempotent — re-running the same `(key, value)` pair yields a byte-identical Evidence document.
- `requirement` ids follow the shared cross-source rules ([reconciliation.md](../references/emery-runtime/reconciliation.md)): dotted-kebab, derived from the domain concept the directive governs (`session.timeout`, `search.filter`), never positional. When the brief overrides something the docs or code also describe, converging on the same id is what lets intent win the group.
- A `criterion` id must equal its requirement's id or extend it with a dotted suffix.

## Output contract

Return one JSON object matching the gated schema — the Evidence body:

```json
{
  "authority": "intent",
  "claims": [
    { "kind": "intent", "id": "<source-key>", "statement": "<brief, verbatim>" },
    { "kind": "requirement", "id": "<dotted-kebab-id>", "statement": "<one directive, present tense>" }
  ]
}
```

Rules:

- `authority` MUST be the literal string `intent`. The `intent` adapter is the only first-party source that emits this authority class.
- Exactly one `kind: intent` claim, first, carrying the brief verbatim in `statement` — no summarising, no splitting, no grammatical cleanup. The reviewer must see exactly what the operator wrote.
- One `requirement` claim per distinct behavioural directive, in brief order. Quote the operator's wording as one present-tense sentence; do not merge directives or invent ones the brief does not state. A brief that is pure context with no directive yields the `intent` echo claim alone.
- Do not emit a `path:` on any claim. The intent source has no filesystem locus.
- Operators who want to express independent briefs supply more than one intent string, each its own binding.

## Worked example

Input:

- Source key = `intent`
- Inline value = `Sessions must expire after 30 minutes of inactivity. Add a search filter to the user list.`

Output — the Evidence body:

```json
{
  "authority": "intent",
  "claims": [
    { "kind": "intent", "id": "intent", "statement": "Sessions must expire after 30 minutes of inactivity. Add a search filter to the user list." },
    { "kind": "requirement", "id": "session.timeout", "statement": "Sessions must expire after 30 minutes of inactivity." },
    { "kind": "requirement", "id": "user-list.search-filter", "statement": "The user list offers a search filter." }
  ]
}
```

## Notes

- Empty `claims: []` is schema-valid for sources with nothing to say, but the intent adapter is never legitimately empty — the binding exists because the operator supplied a brief. Treat an empty value as an extract failure, never an empty success.
- The engine's load gate is fail-closed: a `requirement` claim without a `statement` field fails the whole run closed (typed `bad_request`). There is no fallback to `synopsis`.
