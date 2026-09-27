# TypeScript / JavaScript source extract

This prompt runs once per seam of a bound `typescript` source. A tree of several production modules is mined one surface per call: the message names the surface — a route, a command, a job, an exported API — and the module a caller enters it at, and lends the whole tree under `$SOURCE_DIR`. A tree of one production module, and an inline value, are one seam mined whole: the message names no surface, and the seam is everything the module or value holds. Your job, given a surface: start at the entry, follow what the surface reaches — its handler, the modules it imports, the services and stores it calls, the types it takes and returns — and emit one Evidence document covering the behaviour a caller observes through that surface. Given the whole: read the one module or value in full and emit the same document for every behaviour a caller observes through it — every route, command, job, or export it holds. The caller joins the calls' answers into the source's one document, and the engine deterministically reconciles it with every other bound source's Evidence into the specification — see [From sources to a spec](reconciliation.md).

## Inputs

- **`$SOURCE_DIR`** — read-only view of the bound source root, always the whole tree. Resolve imports and `tsconfig.json` `paths` mappings relative to it. Absent when the source is an inline `value` (the seam is then in the message); present, with no surface named, when the tree is one production module.
- **The surface** — when the message names one, its name and its entry module. Mine from the entry outward: everything the surface reaches is yours to read, and to claim for what this surface's caller observes of it. What the tree does for another surface — another route's handler, another command's run, another job's schedule — is that surface's call to claim, even when you pass through a module the two share. A start surface claims what starting the process does — the port, the connections, the registrations — and stops at each registration: what a registered route, job, or command does once reached is that surface's call. Absent when the seam is mined whole — a tree of one production module, or an inline value: there is then no other surface to stop at, and every behaviour the module or value exhibits is this call's to claim, each under the domain noun of the surface it belongs to.
- **Source name** — the kebab-case name the engine passed on the WIT bindings, which the specification cites the source by.

Nothing outside the bound source is reachable; writes back into `$SOURCE_DIR` are denied. Extract mines its seam completely in one pass: given a surface, the entry, every handler and domain module it reaches, and every store, client, and type on the way; mined whole, the one module or value entire.

## References

Each reference carries TypeScript-specific depth and names the claims it feeds. Read one when its trigger holds for the seam in hand, and not otherwise: a reference read for a construct the seam lacks costs a turn and teaches nothing, and this prompt with `claims.md` already says what every claim carries.

- [`references/component-structure.md`](references/component-structure.md) — read when the message names a surface in a tree of several modules: the manifest and `tsconfig.json`, where each kind of surface enters, the entry layer, async boundaries. A tree of one module needs none of it.
- [`references/business-logic.md`](references/business-logic.md) — read before the first `requirement` of a handler with branches, errors, side effects, timing, or configuration: what a `requirement` captures, and what commonly goes missing.
- [`references/types.md`](references/types.md) — read when the seam declares a type with nesting, optionality, unions, decorators, or converters: `type` claims, declarations verbatim, wire names, what a response actually is. A flat interface needs only the kind table below.
- [`references/external-api.md`](references/external-api.md) — read only when the seam performs outbound HTTP (`fetch`, `axios`, an HTTP client): URL as constructed, headers, bodies, response as deserialised, auth, retries, timeouts.
- [`references/services.md`](references/services.md) — read only when the seam reaches a store, cache, broker, or identity provider through a client: each by kind, and publications — topic, count, delay placement, payload, metadata. An in-memory `Map` is not one.
- [`references/observability.md`](references/observability.md) — read only when the seam emits a metric, a trace span, or a structured log.
- [`references/verification.md`](references/verification.md) — read once, before answering: the checklist.
- [`references/examples/README.md`](references/examples/README.md) — read only when a construct's claim shape is still unclear after the references above; its table names the one example nearest the surface, and one is enough.

## Claim kinds

This adapter emits from the closed enum:

| Kind | Required body field | When to emit |
|---|---|---|
| `requirement` | `statement` | A behavioural fact the code exhibits, stated as one present-tense sentence about the system. These are the claims deterministic reconciliation joins against documentation and intent. |
| `excerpt` | `excerpt` (free-form) | A behavioural code span backing a requirement: handler bodies, validation logic, error paths. |
| `type` | `signature` (free-form), `name` | A declared interface, type alias, class declaration, or DTO whose shape synthesis will need, named by its declared identifier in `name` (`User`) — the key the design block renders it under. |
| `call` | `callee` (free-form) | An observed cross-module call that contributes to behaviour (the call is the wire). |

**`requirement` claims are the reconciliation currency.** Only `kind: requirement` claims form the spec's requirements; `excerpt` / `type` / `call` claims reach synthesis as supporting context but can never agree, diverge, or conflict with another source. Every behavioural fact worth a spec block — a timeout value, a validation rule, an error response, a side effect — must be lifted into a `requirement` claim with a `statement`, anchored by its `path` and backed by detail claims. The gate is fail-closed ([claims.md](claims.md)): a `requirement` claim without a `statement` field fails the whole run closed (typed `bad_request`).

`id` is **required** on `requirement` claims and follows the dotted-kebab grammar in claims.md (`session.timeout`). Derive ids from the domain concept — never from file paths or positions — so a documentation source describing the same behaviour converges on the same id and the engine can reconcile any disagreement. Lead each id with the domain noun of the surface the behaviour belongs to — the one the message names, or, mined whole, the one the module exposes it through (`orders.…`, `user-registration.…`, `migrate.…`): the other surfaces of the estate are mined by other calls and joined with this one, and two calls that name one requirement with reworded statements manufacture a conflict, so claim a behaviour under the surface whose caller observes it. A module several surfaces reach — a repository, a validator, a client — is claimed for what this surface does with it, under this surface's noun, never for itself. `id` is optional on `excerpt` / `type` / `call`; you MAY carry it when the claim backs a specific requirement.

Code states behaviour, not acceptance. A `criterion` is an acceptance boundary the source spells as a value of its own — a named threshold constant, a schema or validator definition, a validation pattern such as a regex literal — one a documentation source could state verbatim; its id extends its requirement's. An inline comparison in a guard (`items.length === 0`, `quantity < 1`, `state === "shipped"`) is behaviour: state it, with its value, in the `requirement` it belongs to and emit no criterion for it. Decide once per boundary by that rule and do not revisit it. Requirements without criteria surface as `[unknown]` acceptance gaps in the spec — that is honest output, not a failure to fix by inventing criteria.

## Anchors and excerpts

Every claim from the tree carries a `path` anchor in the grammar of [claims.md](claims.md) — `<path>#L<n>` or `<path>#L<start>-L<end>`, relative under `$SOURCE_DIR`, not under a skip root. The anchor IS the citation; the body field carries short context.

Rules for the body fields:

- **No raw file dumps.** Anchors point at the source; the JSON must not paraphrase or restate large spans. Keep `excerpt:` to a paragraph or so of focused context (the validation rule, the error response, the side effect) — never tens of lines of `"\n"`-separated source.
- **One claim per concept.** Two overlapping excerpts of the same handler are noise; pick the smallest range that captures the behaviour.
- **Symbols, not phrasing.** `call.callee` is `<file>:<symbol>` for a symbol of the tree — a named export (`src/users/repository.ts:insertUser`), a class method (`src/mail/mailer.ts:Mailer.send`), or a framework-suffixed inline arrow (`src/server.ts:post-/users`); `<package>:<symbol>` for a package import (`@azure/identity:getAzureToken`); bare for a global (`fetch`). `type.signature` is the declaration's source spelling (one line preferred; multi-line acceptable for short class headers).

## Worked example

A small Express service bound as the source `legacy-monolith`, mined for the surface `POST /users`, entered at `src/server.ts`:

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
    { "kind": "type", "path": "src/users/repository.ts#L1-L4", "name": "User", "signature": "interface User { id: string; email: string; createdAt: Date }" },
    { "kind": "call", "path": "src/users/register.ts#L31", "callee": "src/users/repository.ts:insertUser" }
  ]
}
```

Two requirements for the spec, three detail claims backing them — the handler's and the repository's behaviour, claimed for what registering a user does with them; what `src/server.ts` mounts besides `POST /users` is other surfaces' calls to claim. The document's source identity is stamped by the engine from the source — it is not written in-document.

**Cover what the estate actually does.** A `POST /orders` handler that writes an orders store must carry that write as a `call` claim and its behaviour as a `requirement`; a handler that invokes an external service must carry that call site. Downstream correlation evidences invocation, read/write, and ownership relationships from these structured claims — do not bury them in `excerpt` prose, and do not write a second behavioural spec in prose instead of emitting the structured claims.

## Path rules

Relative paths only, no `..`, no leading `/`, never under `node_modules`, `vendor`, `target`, `.venv`, `dist`, `build`, no `*.d.ts` files, and never in the engine's own files — `spec.md`, `design.md`, `.omnia/`, the [skip roots](claims.md#skip-roots) every adapter shares. A symlink inside `$SOURCE_DIR` pointing outside is denied at canonicalization by the host — a typed error, never silent narrowing.

## Anti-patterns

- **Raw file dumps in `excerpt:`.** Anchors point at lines; the body field is short context, not a verbatim paste. A 200-line `excerpt:` field is wrong even when the underlying span is 200 lines.
- **Speculative claims.** Do not infer behaviour the code does not exhibit. If the handler does not enforce uniqueness, do not emit a uniqueness claim. The engine tags gaps `[unknown]`; you do not fill them.
- **Detail without a requirement.** An estate mined into fifty excerpts and zero `requirement` claims contributes nothing to reconciliation. Lift every spec-worthy behaviour into a `requirement` first; excerpts back it.
- **Tests-as-evidence.** Skip `*.test.*`, `*.spec.*`, `tests/`, `__tests__/`. Test files document expected behaviour; this adapter extracts observed behaviour from production source.
- **Type-only `.d.ts` files.** A `.d.ts` declares ambient types, not behaviour. Use the originating `.ts` file when possible; emit no claim when only a `.d.ts` is reachable.
- **Cross-source synthesis.** Do not reconcile this source's claims with another source's Evidence — that is the engine's job after every extract returns. Emit Evidence purely from `$SOURCE_DIR`.
- **Claims from another surface.** When the message names a surface, a module you pass through on the way — the bootstrap that mounts every router, a repository two handlers share — is claimed for what this surface does with it, never for what another surface does: that is the other surface's call, and claiming it twice manufactures conflicts.
- **Mining the tree instead of the surface.** When the message names a surface, the whole tree is lent so the surface can be followed wherever it reaches, not so every module can be read in turn: a module the surface never reaches is no evidence of this surface's behaviour. A seam mined whole names no surface, and its one module is read in full.
- **Whole-file paths without anchors.** A `path: src/users/register.ts` claim is legal under the schema but useless for review. Always anchor to the smallest meaningful range.

## Failure modes

| Condition | Action |
| --------- | ------ |
| The seam reaches no in-scope production source — a handler that is a stub, an export of types alone | Return `claims: []`; the engine preserves the gap rather than guessing. |
| Read denied outside `$SOURCE_DIR` | The host returns a typed path-denied error; no Evidence is written. |
| Production source uses an out-of-scope framework only | Emit any in-scope claims; the gap surfaces as `[unknown]` requirements in the spec. |
| The answer fails the claim gate (id grammar, or a claim missing its required field such as a `requirement`'s `statement`) | The caller rejects it and asks for a corrected answer with the findings; correct the named claims. |
