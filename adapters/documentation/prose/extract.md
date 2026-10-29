# `documentation.extract`

Mine the seam this call is given — the whole bound documentation tree, or the documents the message lists beneath `$SOURCE_DIR` — and return one `Evidence` document of structured claims. A large tree is mined in several calls — one per directory, or one per stem where a survey has named the tree's subjects; the caller joins the answers into the source's one document, so each call covers its seam completely and nothing else. The engine deterministically reconciles the result with every other bound source's into the specification — see [From sources to a spec](reconciliation.md).

## Inputs

- `$SOURCE_DIR` — read-only view of the bound documentation tree, whole, whichever part of it this call mines. Absent when the source is an inline `value` (the seam is then in the message).
- **The documents to mine** — when the message lists them, those documents and no others, each named relative to `$SOURCE_DIR`; otherwise every document under `$SOURCE_DIR`.
- **The subjects** — when the message lists them: each subject a survey named in the documents, with the lines it spans, its stem, and the id its `requirement`, `criterion`, and `retire` claims lead with. Mine the spans listed and nothing outside them; a document the message says the survey placed under no subject is context, never a `path`. When the message names other stems as other calls', claim nothing under them.
- **Source name** — the name the engine passed on the WIT bindings, which the specification cites the source by.

Nothing outside `$SOURCE_DIR` is reachable. Extract mines its seam completely in one pass: every listed document, top to bottom.

## Claim kinds

Closed for this adapter:

| Kind | Required body field | When to emit |
|---|---|---|
| `requirement` | `statement` | A behavioural claim the docs state about the system, in the present tense: the behaviour and the conditions under which it succeeds or is refused, as the passage states them, each thing the system reaches named by its role. |
| `criterion` | `criterion` | An acceptance criterion the docs list (often under "Acceptance:" or a bullet list under a requirement). |
| `decision` | `decision` | A design or product decision the docs record (often "Decision:" lines or paragraphs), and every passage that prescribes a technology, platform, or product, verbatim. |
| `retire` | `statement` | A passage that withdraws a behaviour — deprecates, removes, or states it no longer applies — its `id` the behaviour withdrawn, by role, and its `statement` the withdrawal as the passage states it. |
| `section` | (free-form) | A bounded prose section worth carrying into synthesis verbatim when no finer-grained claim fits. |

Other claim kinds are out of scope for this adapter. Ids, `path` anchors, and the gate follow [claims.md](claims.md); derive each id from the docs' own noun phrases (`password-reset.expiry`).

A passage's kind is what the passage is, and one passage can be two. A decision record — a `## Decision` paragraph, a `Decision:` line — is a `decision`, and each rule it decides, stated there or under its consequences, is a `requirement` under the record's stem as well: a record yields both, and neither stands in for the other. A bullet list beneath a rule, the items a sentence introduces with a colon, and an acceptance list under a story are `criterion`s under that rule's id, never requirements of their own.

A passage that withdraws a behaviour is a `retire`, as [Withdrawals](claims.md#withdrawals) has it — a `Deprecated:` line, a changelog's `Removed` entry, a sentence saying a path is no longer offered — never a `requirement` stating the negative, and never for a behaviour the docs are merely silent on. A passage saying a behaviour stays as it is is a `requirement` restating that behaviour, so the engine classes it `unchanged`, never a `retire`; a passage that withdraws and restates one behaviour — replacing what it did with what it does now — is a `requirement` alone under that behaviour's id, since the engine reads a `retire` beside its own `requirement` as the restatement anyway. Its `id` leads with the subject's stem as a `requirement`'s does — as the standing requirement or the other sources spell it — and names the behaviour withdrawn, so the withdrawal matches the requirement it withdraws. A withdrawal that also prescribes a technology — `mirroring orders to S3 is retired` — is a `retire` by role and a `decision` verbatim at the same lines; a `retire` naming a technology comes back as a finding to restate.

The technology is the design's, as [claims.md](claims.md) has it. A passage that prescribes one — a decision record choosing the broker, a page saying the secrets live in one vault, a line naming the framework — is a `decision` alone, verbatim, and no `requirement`: it states what was chosen, not what the system does. A rule that mentions a technology in passing — `reset tokens are kept in Redis and expire with the link` — is a `requirement` by the role the technology plays (`the cache`), and the mention a `decision` verbatim at the same line, so the specification keeps the rule and the design the product. A `requirement` or `criterion` naming a technology — in its `id` or its body, however the docs spell it — comes back as a finding to restate.

A requirement's statement is the rule whole. A sentence that qualifies the one before it — which cases succeed, which are refused, a value or a threshold — is part of that requirement's statement, however many sentences the paragraph spends on it: never a second requirement under the same stem, and never a criterion, which is what the docs list beneath the rule, not what they state in its own paragraph. Cut the statement at its first sentence and the condition that tells this source from another is lost to reconciliation, which compares statements alone.

Lead every id with the domain noun of the subject this seam documents (`password-reset.…`, `orders.…`), never with a file or directory name; where the message gives a subject the id its claims lead with (`returns.late-returns`), lead with that, and put what tells the claim apart after it (`returns.late-returns.grace-period`). Other parts of the same tree are mined by other calls and joined with this one; two calls that name one requirement with reworded statements manufacture a conflict, so scope ids to the subject at hand and state each requirement once, where the docs state it.

## Anchors

A claim anchors at the lines that state it, never at a heading, which introduces a subject and states nothing. A `requirement` anchors at the paragraph, list item, table row, step, or quotation that states the rule; cite the span of the statement, `#L10-L13` for a paragraph of four lines, `#L15` for one item. A `criterion` anchors at the item that lists it, a `decision` where the document records it, a `retire` at the passage that withdraws, and the anchor rule changes no claim's kind: the kinds above decide that. A rule a heading announces and a paragraph beneath it states anchors at the paragraph.

## Output

Return one JSON object matching the claims schema the request carries:

```json
{
  "claims": [
    { "kind": "requirement", "id": "<dotted-kebab-id>", "path": "<relative-path>#L<n>", "statement": "..." },
    { "kind": "criterion", "id": "<requirement-id>.<suffix>", "path": "<relative-path>#L<n>", "criterion": "..." },
    { "kind": "decision", "path": "<relative-path>#L<n>", "decision": "..." },
    { "kind": "retire", "id": "<dotted-kebab-id>", "path": "<relative-path>#L<n>", "statement": "..." }
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

A user resets the password by following the link. Following an unexpired
link sets the new password and signs the user in; following an expired
link is refused with an answer that says the link has expired.

Reset tokens are kept in Redis and expire with the link.

Decision: use the existing transactional email provider rather than introducing a new notification service.

Deprecated: security questions are no longer offered as a way to reset a password.
```

Output:

```json
{
  "claims": [
    { "kind": "requirement", "id": "password-reset.request", "path": "password-reset.md#L3", "statement": "The account service should let a registered user request a password reset link by email." },
    { "kind": "criterion", "id": "password-reset.request.response-privacy", "path": "password-reset.md#L6", "criterion": "Unknown email addresses receive the same outward response as known users." },
    { "kind": "criterion", "id": "password-reset.request.expiry", "path": "password-reset.md#L7", "criterion": "Reset links expire after 30 minutes." },
    { "kind": "requirement", "id": "password-reset.complete", "path": "password-reset.md#L9-L11", "statement": "A user resets the password by following the link. Following an unexpired link sets the new password and signs the user in; following an expired link is refused with an answer that says the link has expired." },
    { "kind": "requirement", "id": "password-reset.token-store", "path": "password-reset.md#L13", "statement": "Reset tokens are kept in the cache and expire with the link." },
    { "kind": "decision", "path": "password-reset.md#L13", "decision": "Reset tokens are kept in Redis and expire with the link." },
    { "kind": "decision", "path": "password-reset.md#L15", "decision": "Use the existing transactional email provider rather than introducing a new notification service." },
    { "kind": "retire", "id": "password-reset.security-questions", "path": "password-reset.md#L17", "statement": "Security questions are no longer offered as a way to reset a password." }
  ]
}
```

The second paragraph is one requirement: its second sentence states the conditions of the first, so the statement runs to the end of the paragraph and the expired-link refusal is no criterion. The third names the store's product in passing, so it is two claims at one line: the rule, a `requirement` by the role the product plays, and the product, a `decision` verbatim. The last line withdraws a reset path, so it is a `retire` under the subject's stem naming that path, and no `requirement` states that the questions are not offered.

## Determinism

- Emit claims in source order (document by document in lexicographic path order, top of document to bottom). Stable order keeps re-runs byte-stable; the caller keeps the order across the calls it joins.
- Quote statements / criteria / decisions verbatim from the docs where possible. Light grammatical normalisation (capitalisation, terminal punctuation) is allowed, and so is the one substitution of a technology's name for its role in a `statement` or a `criterion`; rephrasing is not — the `statement` value is what reconciliation compares across sources, so paraphrase drift manufactures false conflicts.

## Guardrails

- `$SOURCE_DIR` is read-only; never attempt to read or write outside it.
- When the message lists the documents to mine, mine those and no others: a document beside them belongs to another call, and mining it twice manufactures conflicts.
- Skip the [skip roots](claims.md#skip-roots) every adapter shares wherever they appear in the tree.
- Never emit claim kinds outside `{requirement, criterion, decision, retire, section}` from this adapter. Behaviour kinds (`excerpt`/`type`/`call`) belong to code source adapters.
