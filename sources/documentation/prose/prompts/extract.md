# `documentation.extract`

Walk the whole bound documentation source and return one `Evidence` document of structured claims. The caller persists the result; this answer is the JSON body only. The engine deterministically reconciles this Evidence with every other bound source's into the specification — see [From sources to a spec](../references/emery-runtime/reconciliation.md).

## Inputs

- `$SOURCE_DIR` — read-only view of the bound documentation tree. Absent when the binding is an inline `value` (the material is then in the message).
- **Source key** — the authored binding key the engine passed on the wire.

Nothing outside the bound source is reachable. Extract mines this source completely in one pass: every document in the tree, top to bottom.

## Claim kinds

Closed for this adapter:

| Kind | Required body field | When to emit |
|---|---|---|
| `requirement` | `statement` | A behavioural claim the docs state about the system (one sentence, present tense). |
| `criterion` | `criterion` | An acceptance criterion the docs list (often under "Acceptance:" or a bullet list under a requirement). |
| `decision` | `decision` | A design or product decision the docs record (often "Decision:" lines or paragraphs). |
| `section` | (free-form) | A bounded prose section worth carrying into synthesis verbatim when no finer-grained claim fits. |

The engine's load gate is fail-closed: a `requirement` claim without a `statement` field, or a `criterion` claim without a `criterion` field, fails the whole run closed (typed `bad_request`). There is no fallback to `synopsis`. Other claim kinds are out of scope for this adapter.

`id` is **required** on `requirement` and `criterion` kinds; deterministic reconciliation keys off it. `id` is **optional** on `decision` and `section`.

## `id` derivation

Ids are the cross-source join key: the engine connects claims across sources only when their ids are byte-equal, so two independent sources describing the same behaviour must converge on the same id.

- Dotted-kebab grammar: `password-reset.expiry`, `session.timeout`.
- Derive from the domain concept using the docs' own noun phrases (`password-reset.expiry`, not `req-007`) — never from file names, heading positions, or invented counters.
- A `criterion` id must equal its requirement's id or extend it with a dotted suffix (`password-reset.expiry` or `password-reset.expiry.window`). The engine flags any requirement without such a criterion as an `[unknown]` acceptance gap; a criterion with an unrelated id leaves its requirement uncovered.

## `path` grammar

Every claim from a `$SOURCE_DIR` tree carries a `path` rooted relative to `$SOURCE_DIR`. The grammar matches GitHub-style anchors:

- `<path>` — whole-file claim.
- `<path>#L<n>` — single line.
- `<path>#L<start>-L<end>` — line range.

Line numbers are 1-indexed against the file at extract time. Choose the tightest anchor that bounds the cited text. Claims from an inline value omit `path`.

## Output

Return one JSON object matching the gated schema — the Evidence body:

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

`authority` is always the literal `documentation` (operator-provided written product/technical intent). The document's source identity is stamped by the engine from the binding — it is not written in-document.

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
- Do not invent `id`s. Derive them from the docs' own noun phrases.

## Guardrails

- `$SOURCE_DIR` is read-only; never attempt to read or write outside it.
- Never write Evidence to disk yourself — return the JSON body; the caller persists it.
- Never emit claim kinds outside `{requirement, criterion, decision, section}` from this adapter. Behaviour kinds (`excerpt`/`type`/`call`) belong to code source adapters.
- Never omit `id` on `requirement` or `criterion`, and never omit the kind's required body field — the engine fails the run closed (typed `bad_request`) rather than accepting the claim.
- Empty `claims: []` is valid output when the source genuinely contains no extractable claims. Do not pad with speculative claims; the engine preserves gaps as `[unknown]` rather than guessing.
