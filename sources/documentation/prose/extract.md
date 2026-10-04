# `documentation.extract`

Mine the seam this call is given — the whole bound documentation tree, or the documents the message lists beneath `$SOURCE_DIR` — and return one `Evidence` document of structured claims. A large tree is mined in several calls — one per directory, or one per stem where a survey has named the tree's subjects; the caller joins the answers into the source's one document, so each call covers its seam completely and nothing else. The engine deterministically reconciles the result with every other bound source's into the specification — see [From sources to a spec](reconciliation.md).

## Inputs

- `$SOURCE_DIR` — read-only view of the bound documentation tree, whole, whichever part of it this call mines. Absent when the source is an inline `value` (the seam is then in the message).
- **The documents to mine** — when the message lists them, those documents and no others, each named relative to `$SOURCE_DIR`; otherwise every document under `$SOURCE_DIR`.
- **The subjects** — when the message lists them: each subject a survey named in the documents, with the lines it spans, its stem, and the id its `requirement` and `criterion` claims lead with. Mine the spans listed and nothing outside them; a document the message says the survey placed under no subject is context, never a `path`. When the message names other stems as other calls', claim nothing under them.
- **The lines a `requirement` may anchor at** — when the message lists them: the paragraphs, list items, table rows, steps, and quotations within the subjects, by document. A `requirement` at any other line is refused and comes back with the nearest listed lines named; re-anchor it at the line that states the rule, or leave it out.
- **Source name** — the name the engine passed on the WIT bindings, which the specification cites the source by.

Nothing outside `$SOURCE_DIR` is reachable. Extract mines its seam completely in one pass: every listed document, top to bottom.

## Claim kinds

Closed for this adapter:

| Kind | Required body field | When to emit |
|---|---|---|
| `requirement` | `statement` | A behavioural claim the docs state about the system (one sentence, present tense). |
| `criterion` | `criterion` | An acceptance criterion the docs list (often under "Acceptance:" or a bullet list under a requirement). |
| `decision` | `decision` | A design or product decision the docs record (often "Decision:" lines or paragraphs). |
| `section` | (free-form) | A bounded prose section worth carrying into synthesis verbatim when no finer-grained claim fits. |

Other claim kinds are out of scope for this adapter. Ids, `path` anchors, and the gate follow [claims.md](claims.md); derive each id from the docs' own noun phrases (`password-reset.expiry`).

Lead every id with the domain noun of the subject this seam documents (`password-reset.…`, `orders.…`), never with a file or directory name; where the message gives a subject the id its claims lead with (`returns.late-returns`), lead with that, and put what tells the claim apart after it (`returns.late-returns.grace-period`). Other parts of the same tree are mined by other calls and joined with this one; two calls that name one requirement with reworded statements manufacture a conflict, so scope ids to the subject at hand and state each requirement once, where the docs state it.

## Anchors

A claim anchors at the lines that state it, never at a heading, which introduces a subject and states nothing. A `requirement` anchors at the paragraph, list item, table row, step, or quotation that states the rule; cite the span of the statement, `#L10-L13` for a paragraph of four lines, `#L15` for one item. A `criterion` anchors at the item that lists it, and the anchor rule does not change its kind: a bullet list beneath a rule, the cases a sentence introduces, an acceptance list under a story, are criteria of that rule under its id, not requirements of their own. A `decision` anchors where the document records it. A rule a heading announces and a paragraph beneath it states anchors at the paragraph.

## Output

Return one JSON object matching the claims schema the request carries:

```json
{
  "claims": [
    { "kind": "requirement", "id": "<dotted-kebab-id>", "path": "<relative-path>#L<n>", "statement": "..." },
    { "kind": "criterion", "id": "<requirement-id>.<suffix>", "path": "<relative-path>#L<n>", "criterion": "..." },
    { "kind": "decision", "path": "<relative-path>#L<n>", "decision": "..." }
  ]
}
```

## Worked example

Input (`password-reset.md` in `$SOURCE_DIR`):

```markdown
# Password reset

The account service should let a registered user request a password reset link by email.

Acceptance:
- Unknown email addresses receive the same outward response as known users.
- Reset links expire after 30 minutes.

Decision: use the existing transactional email provider rather than introducing a new notification service.
```

Output:

```json
{
  "claims": [
    { "kind": "requirement", "id": "password-reset.request", "path": "password-reset.md#L3", "statement": "The account service should let a registered user request a password reset link by email." },
    { "kind": "criterion", "id": "password-reset.request.response-privacy", "path": "password-reset.md#L6", "criterion": "Unknown email addresses receive the same outward response as known users." },
    { "kind": "criterion", "id": "password-reset.request.expiry", "path": "password-reset.md#L7", "criterion": "Reset links expire after 30 minutes." },
    { "kind": "decision", "path": "password-reset.md#L9", "decision": "Use the existing transactional email provider rather than introducing a new notification service." }
  ]
}
```

## Determinism

- Emit claims in source order (document by document in lexicographic path order, top of document to bottom). Stable order keeps re-runs byte-stable; the caller keeps the order across the calls it joins.
- Quote statements / criteria / decisions verbatim from the docs where possible. Light grammatical normalisation (capitalisation, terminal punctuation) is allowed; rephrasing is not — the `statement` value is what reconciliation compares across sources, so paraphrase drift manufactures false conflicts.

## Guardrails

- `$SOURCE_DIR` is read-only; never attempt to read or write outside it.
- When the message lists the documents to mine, mine those and no others: a document beside them belongs to another call, and mining it twice manufactures conflicts.
- Skip the [skip roots](claims.md#skip-roots) every adapter shares wherever they appear in the tree.
- Never emit claim kinds outside `{requirement, criterion, decision, section}` from this adapter. Behaviour kinds (`excerpt`/`type`/`call`) belong to code source adapters.
