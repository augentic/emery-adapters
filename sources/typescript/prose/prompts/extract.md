# TypeScript / JavaScript source extract

This prompt runs once per seam of a bound `typescript` source. A tree is mined one surface per call: the message names the surface — a route, a command, a job, an exported API — and the module a caller enters it at, and lends the whole tree under `$SOURCE_DIR`; an inline value is one seam, mined whole. Your job: start at the entry, follow what the surface reaches — its handler, the modules it imports, the services and stores it calls, the types it takes and returns — and emit one Evidence document covering the behaviour a caller observes through that surface. The caller joins the calls' answers into the source's one document, and the engine deterministically reconciles it with every other bound source's Evidence into the specification — see [From sources to a spec](../emery/reconciliation.md).

## Inputs

- **`$SOURCE_DIR`** — read-only view of the bound source root, always the whole tree. Resolve imports and `tsconfig.json` `paths` mappings relative to it. Absent when the source is an inline `value` (the seam is then in the message).
- **The surface** — when the message names one, its name and its entry module. Mine from the entry outward: everything the surface reaches is yours to read, and to claim for what this surface's caller observes of it. What the tree does for another surface — another route's handler, another command's run, another job's schedule — is that surface's call to claim, even when you pass through a module the two share. Absent for an inline value, which is mined whole.
- **Source key** — the kebab-case source key the engine passed on the WIT bindings.

Nothing outside the bound source is reachable; writes back into `$SOURCE_DIR` are denied. Extract mines its surface completely in one pass: the entry, every handler and domain module it reaches, and every store, client, and type on the way.

## References

Load on demand when a surface needs deeper analysis. Each carries TypeScript-specific depth and names the claims it feeds.

- [`references/component-structure.md`](../references/component-structure.md) — the manifest and `tsconfig.json`, where each kind of surface enters, the entry layer, async boundaries.
- [`references/business-logic.md`](../references/business-logic.md) — what a `requirement` captures from a handler: validation, branches, errors, side effects, sequencing, timing, configuration, data access.
- [`references/types.md`](../references/types.md) — `type` claims: declarations verbatim, nesting, optionality, unions, wire names and converters, what a response actually is.
- [`references/external-api.md`](../references/external-api.md) — outbound HTTP as `call` and `requirement` claims: URL as constructed, headers, bodies, response as deserialised, auth, retries, timeouts.
- [`references/services.md`](../references/services.md) — stores, caches, brokers, and identity providers by kind, and publications: topic, count, delay placement, payload, metadata.
- [`references/observability.md`](../references/observability.md) — metric, trace, and log emissions as claims.
- [`references/verification.md`](../references/verification.md) — the checklist before answering.
- [`references/examples/README.md`](../references/examples/README.md) — worked examples with their Evidence: outbound HTTP, branching and caching, parallel execution and publishing.

## Claim kinds

This adapter emits from the closed enum:

| Kind | Required body field | When to emit |
|---|---|---|
| `requirement` | `statement` | A behavioural fact the code exhibits, stated as one present-tense sentence about the system. These are the claims deterministic reconciliation joins against documentation and intent. |
| `excerpt` | `excerpt` (free-form) | A behavioural code span backing a requirement: handler bodies, validation logic, error paths. |
| `type` | `signature` (free-form) | A declared interface, type alias, class declaration, or DTO whose shape synthesis will need. |
| `call` | `callee` (free-form) | An observed cross-module call that contributes to behaviour (the call is the wire). |

**`requirement` claims are the reconciliation currency.** Only `kind: requirement` claims form the spec's requirements; `excerpt` / `type` / `call` claims reach synthesis as supporting context but can never agree, diverge, or conflict with another source. Every behavioural fact worth a spec block — a timeout value, a validation rule, an error response, a side effect — must be lifted into a `requirement` claim with a `statement`, anchored by its `path` and backed by detail claims. The gate is fail-closed ([claims.md](../emery/claims.md)): a `requirement` claim without a `statement` field fails the whole run closed (typed `bad_request`).

`id` is **required** on `requirement` claims and follows the dotted-kebab grammar in claims.md (`session.timeout`). Derive ids from the domain concept — never from file paths or positions — so a documentation source describing the same behaviour converges on the same id and the engine can reconcile any disagreement. Lead each id with the domain noun of the surface this call mines (`orders.…`, `user-registration.…`, `migrate.…`): the other surfaces of the estate are mined by other calls and joined with this one, and two calls that name one requirement with reworded statements manufacture a conflict, so claim a behaviour under the surface whose caller observes it. A module several surfaces reach — a repository, a validator, a client — is claimed for what this surface does with it, under this surface's noun, never for itself. `id` is optional on `excerpt` / `type` / `call`; you MAY carry it when the claim backs a specific requirement.

Code states behaviour, not acceptance: emit `criterion` claims only when the source itself encodes an explicit acceptance boundary (a documented threshold constant, a schema constraint). Requirements without criteria surface as `[unknown]` acceptance gaps in the spec — that is honest output, not a failure to fix by inventing criteria.

## Anchors and excerpts

Every claim from the tree carries a `path` anchor in the grammar of [claims.md](../emery/claims.md) — `<path>#L<n>` or `<path>#L<start>-L<end>`, relative under `$SOURCE_DIR`, not under a skip root. The anchor IS the citation; the body field carries short context.

Rules for the body fields:

- **No raw file dumps.** Anchors point at the source; the JSON must not paraphrase or restate large spans. Keep `excerpt:` to a paragraph or so of focused context (the validation rule, the error response, the side effect) — never tens of lines of `"\n"`-separated source.
- **One claim per concept.** Two overlapping excerpts of the same handler are noise; pick the smallest range that captures the behaviour.
- **Symbols, not phrasing.** `call.callee` is `<file>:<symbol>` for a symbol of the tree — a named export (`src/users/repository.ts:insertUser`), a class method (`src/mail/mailer.ts:Mailer.send`), or a framework-suffixed inline arrow (`src/server.ts:post-/users`); `<package>:<symbol>` for a package import (`@azure/identity:getAzureToken`); bare for a global (`fetch`). `type.signature` is the declaration's source spelling (one line preferred; multi-line acceptable for short class headers).

## Worked example

A small Express service bound under source key `legacy-monolith`, mined for the surface `POST /users`, entered at `src/server.ts`:

- `src/server.ts` — `app.post("/users", registerUser)` at L5.
- `src/users/register.ts` — `registerUser` handler with email validation at L12–L34 and a delegation to `insertUser`.
- `src/users/repository.ts` — `insertUser` declaration plus the `User` interface.

Resulting Evidence body:

```json
{
  "claims": [
    { "kind": "requirement", "id": "user-registration.email-validation", "path": "src/users/register.ts#L12-L34", "statement": "Registration rejects an email that is not RFC-5322 valid with a 400 response." },
    { "kind": "requirement", "id": "user-registration.persistence", "path": "src/users/register.ts#L31", "statement": "A valid registration inserts the user and returns 201 with the persisted record." },
    { "kind": "excerpt", "path": "src/users/register.ts#L12-L34", "excerpt": "Handler validates email against RFC-5322 regex, returns 400 with { error: \"invalid-email\" } on failure, otherwise inserts the user and returns 201 with the persisted record." },
    { "kind": "type", "path": "src/users/repository.ts#L1-L4", "signature": "interface User { id: string; email: string; createdAt: Date }" },
    { "kind": "call", "path": "src/users/register.ts#L31", "callee": "src/users/repository.ts:insertUser" }
  ]
}
```

Two requirements for the spec, three detail claims backing them — the handler's and the repository's behaviour, claimed for what registering a user does with them; what `src/server.ts` mounts besides `POST /users` is other surfaces' calls to claim. The document's source identity is stamped by the engine from the source — it is not written in-document.

**Cover what the estate actually does.** A `POST /orders` handler that writes an orders store must carry that write as a `call` claim and its behaviour as a `requirement`; a handler that invokes an external service must carry that call site. Downstream correlation evidences invocation, read/write, and ownership relationships from these structured claims — do not bury them in `excerpt` prose, and do not write a second behavioural spec in prose instead of emitting the structured claims.

## Path rules

Relative paths only, no `..`, no leading `/`, never under `node_modules`, `vendor`, `target`, `.venv`, `dist`, `build`, no `*.d.ts` files, and never in the engine's own files — `spec.md`, `design.md`, `.omnia/`, the [skip roots](../emery/claims.md#skip-roots) every adapter shares. A symlink inside `$SOURCE_DIR` pointing outside is denied at canonicalization by the host — a typed error, never silent narrowing.

## Anti-patterns

- **Raw file dumps in `excerpt:`.** Anchors point at lines; the body field is short context, not a verbatim paste. A 200-line `excerpt:` field is wrong even when the underlying span is 200 lines.
- **Speculative claims.** Do not infer behaviour the code does not exhibit. If the handler does not enforce uniqueness, do not emit a uniqueness claim. The engine tags gaps `[unknown]`; you do not fill them.
- **Detail without a requirement.** An estate mined into fifty excerpts and zero `requirement` claims contributes nothing to reconciliation. Lift every spec-worthy behaviour into a `requirement` first; excerpts back it.
- **Tests-as-evidence.** Skip `*.test.*`, `*.spec.*`, `tests/`, `__tests__/`. Test files document expected behaviour; this adapter extracts observed behaviour from production source.
- **Type-only `.d.ts` files.** A `.d.ts` declares ambient types, not behaviour. Use the originating `.ts` file when possible; emit no claim when only a `.d.ts` is reachable.
- **Cross-source synthesis.** Do not reconcile this source's claims with another source's Evidence — that is the engine's job after every extract returns. Emit Evidence purely from `$SOURCE_DIR`.
- **Claims from another surface.** A module you pass through on the way — the bootstrap that mounts every router, a repository two handlers share — is claimed for what this surface does with it, never for what another surface does: that is the other surface's call, and claiming it twice manufactures conflicts.
- **Mining the tree instead of the surface.** The whole tree is lent so the surface can be followed wherever it reaches, not so every module can be read in turn. A module the surface never reaches is no evidence of this surface's behaviour.
- **Whole-file paths without anchors.** A `path: src/users/register.ts` claim is legal under the schema but useless for review. Always anchor to the smallest meaningful range.

## Failure modes

| Condition | Action |
| --------- | ------ |
| The surface reaches no in-scope production source — a handler that is a stub, an export of types alone | Return `claims: []`; the engine preserves the gap rather than guessing. |
| Read denied outside `$SOURCE_DIR` | The host returns a typed path-denied error; no Evidence is written. |
| Production source uses an out-of-scope framework only | Emit any in-scope claims; the gap surfaces as `[unknown]` requirements in the spec. |
| The answer fails the claim gate (id grammar, or a claim missing its required field such as a `requirement`'s `statement`) | The caller rejects it and asks for a corrected answer with the findings; correct the named claims. |
