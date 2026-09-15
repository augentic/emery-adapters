# `documentation.extract`

Mine the seam this call is given — the whole bound documentation tree, or the documents the message lists beneath `$SOURCE_DIR` — and return one `Evidence` document of structured claims. A large tree is mined one directory per call; the caller joins the answers into the source's one document, so each call covers its seam completely and nothing else. The engine deterministically reconciles the result with every other bound source's into the specification — see [From sources to a spec](../references/emery-runtime/reconciliation.md).

## Inputs

- `$SOURCE_DIR` — read-only view of the bound documentation tree, or of the one directory this call mines. Absent when the source is an inline `value` (the seam is then in the message).
- **The documents to mine** — when the message lists them, those documents and no others; otherwise every document under `$SOURCE_DIR`.
- **Source key** — the authored source key the engine passed on the WIT bindings.

Nothing outside `$SOURCE_DIR` is reachable. Extract mines its seam completely in one pass: every listed document, top to bottom.

## Claim kinds

Closed for this adapter:

| Kind | Required body field | When to emit |
|---|---|---|
| `requirement` | `statement` | A behavioural claim the docs state about the system (one sentence, present tense). |
| `criterion` | `criterion` | An acceptance criterion the docs list (often under "Acceptance:" or a bullet list under a requirement). |
| `decision` | `decision` | A design or product decision the docs record (often "Decision:" lines or paragraphs). |
| `section` | (free-form) | A bounded prose section worth carrying into synthesis verbatim when no finer-grained claim fits. |

Other claim kinds are out of scope for this adapter. Ids, `path` anchors, and the fail-closed gate follow [claims.md](../references/emery-runtime/claims.md): `id` is required on `requirement` and `criterion` (dotted-kebab, derived from the docs' own noun phrases — `password-reset.expiry`, not `req-007`), a `criterion` id extends its requirement's id, every claim from the tree carries a `<path>#L<n>` anchor, and a claim missing its required body field fails the whole run closed (typed `bad_request`).

Lead every id with the domain noun of the subject this seam documents (`password-reset.…`, `orders.…`), never with a file or directory name. Other directories of the same tree are mined by other calls and joined with this one; two calls that name one requirement with reworded statements manufacture a conflict, so scope ids to the subject at hand and state each requirement once, where the docs state it.

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

The document's source identity is stamped by the engine from the source — it is not written in-document.

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

- Emit claims in source order (document by document in lexicographic path order, top of document to bottom). Stable order keeps re-runs byte-stable; the caller keeps the order across the directories it joins.
- Quote statements / criteria / decisions verbatim from the docs where possible. Light grammatical normalisation (capitalisation, terminal punctuation) is allowed; rephrasing is not — the `statement` value is what reconciliation compares across sources, so paraphrase drift manufactures false conflicts.

## Guardrails

- `$SOURCE_DIR` is read-only; never attempt to read or write outside it.
- When the message lists the documents to mine, mine those and no others: a document beside them belongs to another call, and mining it twice manufactures conflicts.
- Skip the engine's own files wherever they appear in the tree — `spec.md`, `design.md`, `.omnia/` — the [skip roots](../references/emery-runtime/claims.md#skip-roots) every adapter shares. A projection of the current revision is output, not documentation to mine.
- Never emit claim kinds outside `{requirement, criterion, decision, section}` from this adapter. Behaviour kinds (`excerpt`/`type`/`call`) belong to code source adapters.
