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
| `requirement` | `statement` | One per distinct behavioural directive the brief states about the system, by the role each thing it names plays. |
| `criterion` | `criterion` | Only when the brief itself states an acceptance criterion. |
| `decision` | `decision` | One per directive that prescribes a technology, platform, or product — the broker, the vault, the framework, the host — carrying the prescription verbatim. |
| `retire` | `statement` | One per directive that withdraws a behaviour — stop, drop, remove, no longer — its `id` the behaviour withdrawn, by role, and its `statement` the withdrawal as one present-tense sentence. |

The verbatim `intent` claim preserves the operator's words for the reviewer. A directive left solely inside it never reaches reconciliation, which joins `requirement` claims alone ([reconciliation.md](reconciliation.md)), so every directive the brief states is also its own `requirement`.

A directive that prescribes a technology is the design's, not the specification's, as [claims.md](claims.md) has it: it is a `decision` with the operator's words verbatim, and the design lands it. One that bundles a behaviour with the technology — `publish each order event to Kafka` — is two claims: the behaviour a `requirement` stated by the role the technology plays (`Each order event is published to the outbound stream.`), and the prescription a `decision` verbatim. A `requirement` or `criterion` naming the technology — in its `id` or its body, however the brief spells it — comes back as a finding to restate.

A directive that withdraws a behaviour is a `retire`, as [Withdrawals](claims.md#withdrawals) has it — `stop sending the weekly digest`, `drop the guest checkout`, `the export is no longer run` — never a `requirement` stating the negative. It is a withdrawal only where the brief states one: a behaviour the brief is silent on is a gap, not a `retire`. A `retire` is stated by role as a `requirement` is, and a withdrawal that also prescribes a technology — `stop mirroring orders to S3` — is two claims, the `retire` by role and a `decision` verbatim; a `retire` naming the technology comes back as a finding to restate.

## `id` derivation

- The `intent` claim's `id` is the source name, keeping the document deterministic and idempotent — re-running the same `(name, value)` pair yields a byte-identical Evidence document.
- `requirement` and `criterion` ids follow [claims.md](claims.md), led by the domain concept the directive governs (`session.timeout`, `search.filter`), never by the technology it names (`orders.publish-events`, not `kafka.publish`). When the brief overrides something the docs or code also describe, converging on the same id is what lets intent win the group.
- A `retire` id names the behaviour withdrawn under the same grammar (`notifications.weekly-digest`), led by the concept the behaviour belonged to, so it meets the id the docs or code state that behaviour under.
- A `decision` carries no `id`.

## Output contract

Return one JSON object matching the claims schema the request carries:

```json
{
  "claims": [
    { "kind": "intent", "id": "<source-name>", "statement": "<brief, verbatim>" },
    { "kind": "requirement", "id": "<dotted-kebab-id>", "statement": "<one directive, present tense>" },
    { "kind": "decision", "decision": "<one technology directive, verbatim>" },
    { "kind": "retire", "id": "<dotted-kebab-id>", "statement": "<one withdrawal, present tense>" }
  ]
}
```

Rules:

- Exactly one `kind: intent` claim, first, carrying the brief verbatim in `statement` — no summarising, no splitting, no grammatical cleanup. The reviewer must see exactly what the operator wrote.
- One `requirement` claim per distinct behavioural directive, in brief order. Quote the operator's wording as one present-tense sentence; do not merge directives or invent ones the brief does not state. The one substitution is a technology's name for its role (`the outbound stream` for `Kafka`, `the secret store` for `Key Vault`), with the technology carried in a `decision` beside it. A brief that is pure context with no directive yields the `intent` echo claim alone.
- One `decision` claim per directive that prescribes a technology, in brief order after the requirement it governs, carrying the operator's words verbatim.
- One `retire` claim per directive that withdraws a behaviour, in brief order, its `statement` the withdrawal as one present-tense sentence under the same substitution, and no `requirement` for the behaviour withdrawn.
- Do not emit a `path:` on any claim. The intent source has no filesystem locus.

## Worked example

Input:

- Source name = `intent`
- Inline value = `Sessions must expire after 30 minutes of inactivity. Add a search filter to the user list. Publish each order event to Kafka. Stop sending the weekly digest email.`

Output:

```json
{
  "claims": [
    { "kind": "intent", "id": "intent", "statement": "Sessions must expire after 30 minutes of inactivity. Add a search filter to the user list. Publish each order event to Kafka. Stop sending the weekly digest email." },
    { "kind": "requirement", "id": "session.timeout", "statement": "Sessions must expire after 30 minutes of inactivity." },
    { "kind": "requirement", "id": "user-list.search-filter", "statement": "The user list offers a search filter." },
    { "kind": "requirement", "id": "orders.publish-events", "statement": "Each order event is published to the outbound stream." },
    { "kind": "decision", "decision": "Publish each order event to Kafka." },
    { "kind": "retire", "id": "notifications.weekly-digest", "statement": "The weekly digest email is no longer sent." }
  ]
}
```

The third directive bundles a behaviour with the technology it prescribes, so it is two claims: the publishing, a `requirement` by the stream's role, which the specification keeps, and the broker, a `decision` verbatim, which the design lands. The fourth withdraws a behaviour, so it is a `retire` naming the digest under the concept it belonged to, and no `requirement` states that the digest is not sent.
