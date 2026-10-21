# behaviour.extract

Emit one `Evidence` document stating what one screen's recorded behaviour is for. The message lays out a ui-ir `behaviour.json` and the source files its evidence cites. The engine reconciles the answer with every other bound source — see [From sources to a spec](reconciliation.md).

## Inputs

- **The behaviour document** — the first file laid out. It is the fact layer, written without a model: `triggers` (what the user acts on, `on.match` the element by role, text and href; `does` the effects — `navigate`, `call` with its `operation` method and URL, `validate`, `show` a toast or message, `set` state, `open_url`, `toggle` an overlay — each with `on_success` / `on_failure`), `guards` (a route that sends the user elsewhere when a condition holds), `validation` (a field's rule and the message it shows), `routes`, `state`. An entry's `evidence` is `file#L` in the source; an entry's `observed` list is what recorded browser runs showed: `confirms` names the effects the run saw happen, `contradicts` an effect it did not.
- **The cited source files** — the rest of the files laid out, so a claim's wording can follow the code.

The document is never rewritten and its facts are not restated: the caller joins every `call` and every observed run (`example`) to the answer itself. This call states the meaning.

## Claim kinds

| Kind | Required body field | When to emit |
|---|---|---|
| `requirement` | `statement` | One per trigger that does something the user observes (a sign-in, a navigation to another screen, an error shown), one per guard, and one for the screen's purpose. |
| `criterion` | `criterion` | One per validation rule, and one per observable outcome of a requirement — the destination reached, the message or toast shown — with the exact text the document carries. |

Every claim carries an extra `meaning`, one of `trigger-intent` (what one trigger is for), `flow-intent` (what a sequence across triggers, guards and destinations achieves), `screen-purpose` (what the whole screen is for, exactly one), `component-purpose` (what a shared component's triggers are for), `match-resolution` (which element a trigger whose `on.match_status` is `multiple` or `ambiguous` most likely acts on — never more than a proposal); an extra `behaviour-pointer`, the JSON pointer of the entry it explains (`/triggers/11`, `/guards/0`, `/validation/2`; `""` for the screen); and, where the entry has `observed` runs that confirm it, `observed-in`, the list of their `recording` names.

## Rules

- Every claim has a `path` from the anchors the call gives: the entry's own `evidence`. A claim with no evidence is not made.
- `id`s lead with the screen stem the call names, then the behaviour: `login.sign-in`, `login.sign-in.wrong-credentials`, `login.password.min-length`. A criterion extends its requirement's id.
- State behaviour as the user sees it, in the domain's words ([claims.md](claims.md)): `Signing in with a valid email and password takes the user to the home screen.` The endpoint, the storage key and the library are the facts' to carry, not the statement's.
- A criterion spells the observable value exactly as the document does: the message text, the destination path, the minimum length.
- An entry with no observed run is still stated, from the code; an entry a run `contradicts` is stated as the code has it, and its criterion names the contradiction.
- A trigger whose only effect is outside the product (`open_url` to another site, a theme toggle) is one `component-purpose` requirement for the group, not one per link.
- Nothing the document does not show: no remembered-me, no rate limit, no message the document does not carry.

## Output contract

```json
{
  "claims": [
    { "kind": "requirement", "id": "<screen>.purpose", "path": "<file>#L<n>", "statement": "...", "meaning": "screen-purpose", "behaviour-pointer": "" },
    { "kind": "requirement", "id": "<screen>.<behaviour>", "path": "<file>#L<n>-L<m>", "statement": "...", "meaning": "trigger-intent", "behaviour-pointer": "/triggers/<i>", "observed-in": ["<recording>"] },
    { "kind": "criterion", "id": "<screen>.<behaviour>.<outcome>", "path": "<file>#L<n>", "criterion": "...", "meaning": "trigger-intent", "behaviour-pointer": "/triggers/<i>" }
  ]
}
```

## Failure modes

| Condition | Action |
| --------- | ------ |
| The document has no trigger, guard or rule | Return `claims: []`. |
| An entry's evidence is outside the files laid out | Leave the entry out; the facts still reach the engine. |
