# `documentation.extract`

Walk the whole bound documentation source and return one `Evidence` document of structured claims. The caller persists the result; this answer is the JSON body only. The engine deterministically reconciles this Evidence with every other bound source's into the specification — see [From sources to a spec](../references/emery-runtime/reconciliation.md).

## Inputs

- `$SOURCE_DIR` — read-only view of the bound documentation tree. Absent when the source is an inline `value` (the material is then in the message).
- **Source key** — the authored source key the engine passed on the wire.

Nothing outside the bound source is reachable. Extract mines this source completely in one pass: every document in the tree, top to bottom.

## Claim kinds

Closed for this adapter:

| Kind | Required body field | When to emit |
|---|---|---|
| `requirement` | `statement` | A behavioural claim the docs state about the system (one sentence, present tense). |
| `criterion` | `criterion` | An acceptance criterion the docs list (often under "Acceptance:" or a bullet list under a requirement). |
| `decision` | `decision` | A design or product decision the docs record (often "Decision:" lines or paragraphs). |
| `section` | (free-form) | A bounded prose section worth carrying into synthesis verbatim when no finer-grained claim fits. |

Other claim kinds are out of scope for this adapter. Ids, `path` anchors, and the fail-closed gate follow [claims.md](../references/emery-runtime/claims.md): `id` is required on `requirement` and `criterion` (dotted-kebab, derived from the docs' own noun phrases — `password-reset.expiry`, not `req-007`), a `criterion` id extends its requirement's id, every claim from the tree carries a `<path>#L<n>` anchor, and a claim missing its required body field fails the whole run closed (typed `bad_request`).

## Output

Return one JSON object matching the Evidence schema the request carries — the Evidence body:

```json
{
  "authority": "documentation",
  "claims": [
    { "kind": "requirement", "id": "<dotted-kebab-id>", "path": "<relative-path>#L<n>", "statement": "..." },
    { "kind": "criterion", "id": "<requirement-id>.<suffix>", "path": "<relative-path>#L<n>", "criterion": "..." },
    { "kind": "decision", "path": "<relative-path>#L<n>", "decision": "..." }
  ]
}
```

`authority` is always the literal `documentation` (operator-provided written product/technical intent). The document's source identity is stamped by the engine from the source — it is not written in-document.

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
  "authority": "documentation",
  "claims": [
    { "kind": "requirement", "id": "password-reset.request", "path": "password-reset.md#L3", "statement": "The account service should let a registered user request a password reset link by email." },
    { "kind": "criterion", "id": "password-reset.request.response-privacy", "path": "password-reset.md#L6", "criterion": "Unknown email addresses receive the same outward response as known users." },
    { "kind": "criterion", "id": "password-reset.request.expiry", "path": "password-reset.md#L7", "criterion": "Reset links expire after 30 minutes." },
    { "kind": "decision", "path": "password-reset.md#L9", "decision": "Use the existing transactional email provider rather than introducing a new notification service." }
  ]
}
```

## Determinism

- Emit claims in source order (file by file in lexicographic path order, top of file to bottom). Stable order keeps re-runs byte-stable.
- Quote statements / criteria / decisions verbatim from the docs where possible. Light grammatical normalisation (capitalisation, terminal punctuation) is allowed; rephrasing is not — the `statement` value is what reconciliation compares across sources, so paraphrase drift manufactures false conflicts.

## Guardrails

- `$SOURCE_DIR` is read-only; never attempt to read or write outside it.
- Never emit claim kinds outside `{requirement, criterion, decision, section}` from this adapter. Behaviour kinds (`excerpt`/`type`/`call`) belong to code source adapters.
