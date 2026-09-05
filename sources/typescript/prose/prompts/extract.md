# TypeScript / JavaScript source extract

The engine invokes this prompt once per bound `typescript` source. Your job: walk the whole source tree under `$SOURCE_DIR`, read the code, and emit one Evidence document covering the behaviour the estate actually exhibits. The caller persists it; this answer is the JSON body only. The engine deterministically reconciles it with every other bound source's Evidence into the specification — see [From sources to a spec](../references/emery-runtime/reconciliation.md).

## Inputs

- **`$SOURCE_DIR`** — read-only view of the bound source root. Walk it; resolve `tsconfig.json` `paths` mappings relative to it. Absent when the binding is an inline `value` (the material is then in the message).
- **Source key** — the kebab-case binding key the engine passed on the wire.

Nothing outside the bound source is reachable; writes back into `$SOURCE_DIR` are denied. Extract mines the entire estate in one pass: every entry point, handler, and domain module in scope.

## References

Load on demand when a surface needs deeper analysis. The bodies carry TypeScript-specific extraction depth.

- [`references/business-logic.md`](../references/business-logic.md) — depth-first domain extraction by handler / module.
- [`references/component-structure.md`](../references/component-structure.md) — language detection, entry points, module organisation, async patterns.
- [`references/dependencies.md`](../references/dependencies.md) — external service classification (database, message broker, cache, identity provider, API, WebSocket).
- [`references/external-api.md`](../references/external-api.md) — tracing deserialization code for HTTP/API calls; URLs, headers, request/response shapes, auth, retries, timeouts.
- [`references/observability.md`](../references/observability.md) — metric and trace capture: names, types, emission points, labels.
- [`references/verification.md`](../references/verification.md) — final validation checklist before emitting evidence.
- [`references/design-template.md`](../references/design-template.md) — the design surface downstream synthesis fills; the claim-coverage checklist extraction must satisfy.
- [`references/language-mapping.md`](../references/language-mapping.md) — TypeScript → Rust mapping cheatsheet (idioms, error handling, async, serialization).
- [`references/context-gaps.md`](../references/context-gaps.md) — strategies for inferring missing context when source is incomplete.
- [`references/lessons-learned.md`](../references/lessons-learned.md) — empirical wisdom from past extraction passes.
- [`references/semantic-search.md`](../references/semantic-search.md) — codebase search strategies for finding behaviour.
- [`references/examples/`](../references/examples/) — worked examples: outbound HTTP, branching/caching, parallel execution.

## Claim kinds

This adapter emits from the closed enum:

| Kind | Required body field | When to emit |
|---|---|---|
| `requirement` | `statement` | A behavioural fact the code exhibits, stated as one present-tense sentence about the system. These are the claims deterministic reconciliation joins against documentation and intent. |
| `excerpt` | `excerpt` (free-form) | A behavioural code span backing a requirement: handler bodies, validation logic, error paths. |
| `type` | `signature` (free-form) | A declared interface, type alias, class declaration, or DTO whose shape synthesis will need. |
| `call` | `callee` (free-form) | An observed cross-module call that contributes to behaviour (the call is the wire). |

**`requirement` claims are the reconciliation currency.** Only `kind: requirement` claims form spec requirement rows; `excerpt` / `type` / `call` claims reach synthesis as supporting context but can never agree, diverge, or conflict with another source. Every behavioural fact worth a spec block — a timeout value, a validation rule, an error response, a side effect — must be lifted into a `requirement` claim with a `statement`, anchored by its `path` and backed by detail claims. The engine's load gate is fail-closed: a `requirement` claim without a `statement` field fails the whole run closed (typed `bad_request`); there is no fallback to `synopsis`.

`id` is **required** on `requirement` claims (dotted-kebab, e.g. `session.timeout`). Derive ids from the domain concept per the shared rules in [reconciliation.md](../references/emery-runtime/reconciliation.md) — never from file paths or positions — so a documentation source describing the same behaviour converges on the same id and the engine can reconcile any disagreement. `id` is optional on `excerpt` / `type` / `call`; you MAY carry it when the claim backs a specific requirement.

Code states behaviour, not acceptance: emit `criterion` claims only when the source itself encodes an explicit acceptance boundary (a documented threshold constant, a schema constraint). Requirements without criteria surface as `[unknown]` acceptance gaps in the spec — that is honest output, not a failure to fix by inventing criteria.

## Anchors and excerpts

Every claim from the tree carries a `path` anchor: `<path>`, `<path>#L<n>`, or `<path>#L<start>-L<end>`, relative under `$SOURCE_DIR` (no leading `/`, no `..`, not under a skip root). Line numbers are 1-indexed at extract time. The anchor IS the citation; the body field carries short context.

Rules for the body fields:

- **No raw file dumps.** Anchors point at the source; the JSON must not paraphrase or restate large spans. Keep `excerpt:` to a paragraph or so of focused context (the validation rule, the error response, the side effect) — never tens of lines of `"\n"`-separated source.
- **One claim per concept.** Two overlapping excerpts of the same handler are noise; pick the smallest range that captures the behaviour.
- **Stable spans across reruns.** Choose anchors at named-function or block boundaries when possible so re-extraction produces byte-stable Evidence even when surrounding lines shift slightly.
- **Symbols, not phrasing.** `call.callee` is `<file>:<symbol>` — a named export (`src/users/repository.ts:insertUser`), a class method (`src/mail/mailer.ts:Mailer.send`), or a framework-suffixed inline arrow (`src/server.ts:post-/users`). `type.signature` is the declaration's source spelling (one line preferred; multi-line acceptable for short class headers).

## Worked example

A small Express service bound under source key `legacy-monolith`:

- `src/server.ts` — `app.post("/users", registerUser)` at L5.
- `src/users/register.ts` — `registerUser` handler with email validation at L12–L34 and a delegation to `insertUser`.
- `src/users/repository.ts` — `insertUser` declaration plus the `User` interface.

Resulting Evidence body:

```json
{
  "authority": "behaviour",
  "claims": [
    { "kind": "requirement", "id": "user-registration.email-validation", "path": "src/users/register.ts#L12-L34", "statement": "Registration rejects an email that is not RFC-5322 valid with a 400 response." },
    { "kind": "requirement", "id": "user-registration.persistence", "path": "src/users/register.ts#L31", "statement": "A valid registration inserts the user and returns 201 with the persisted record." },
    { "kind": "excerpt", "path": "src/users/register.ts#L12-L34", "excerpt": "Handler validates email against RFC-5322 regex, returns 400 with { error: \"invalid-email\" } on failure, otherwise inserts the user and returns 201 with the persisted record." },
    { "kind": "type", "path": "src/users/repository.ts#L1-L4", "signature": "interface User { id: string; email: string; createdAt: Date }" },
    { "kind": "call", "path": "src/users/register.ts#L31", "callee": "src/users/repository.ts:insertUser" }
  ]
}
```

Two requirement rows for the spec, three detail claims backing them. `authority` is fixed at `behaviour` for this adapter. The document's source identity is stamped by the engine from the binding — it is not written in-document.

**Cover what the estate actually does.** A `POST /orders` handler that writes an orders store must carry that write as a `call` claim and its behaviour as a `requirement`; a handler that invokes an external service must carry that call site. Downstream correlation evidences invocation, read/write, and ownership relationships from these structured claims — do not bury them in `excerpt` prose, and do not write a second behavioural spec in prose instead of emitting the structured claims.

## Path rules

Relative paths only, no `..`, no leading `/`, never under `node_modules`, `vendor`, `target`, `.venv`, `dist`, `build`, no `*.d.ts` files. A symlink inside `$SOURCE_DIR` pointing outside is denied at canonicalization by the host — a typed error, never silent narrowing.

## Anti-patterns

- **Raw file dumps in `excerpt:`.** Anchors point at lines; the body field is short context, not a verbatim paste. A 200-line `excerpt:` field is wrong even when the underlying span is 200 lines.
- **Speculative claims.** Do not infer behaviour the code does not exhibit. If the handler does not enforce uniqueness, do not emit a uniqueness claim. The engine tags gaps `[unknown]`; you do not fill them.
- **Detail without a requirement.** An estate mined into fifty excerpts and zero `requirement` claims contributes nothing to reconciliation. Lift every spec-worthy behaviour into a `requirement` first; excerpts back it.
- **Tests-as-evidence.** Skip `*.test.*`, `*.spec.*`, `tests/`, `__tests__/`. Test files document expected behaviour; this adapter extracts observed behaviour from production source.
- **Type-only `.d.ts` files.** A `.d.ts` declares ambient types, not behaviour. Use the originating `.ts` file when possible; emit no claim when only a `.d.ts` is reachable.
- **Cross-source synthesis.** Do not reconcile this source's claims with another source's Evidence — that is the engine's job after every extract returns. Emit Evidence purely from `$SOURCE_DIR`.
- **Whole-file paths without anchors.** A `path: src/users/register.ts` claim is legal under the schema but useless for review. Always anchor to the smallest meaningful range.

## Failure modes

| Condition | Action |
| --------- | ------ |
| The tree holds no in-scope production source | Return `claims: []`; the engine preserves the gap rather than guessing. |
| Read denied outside `$SOURCE_DIR` | The host returns a typed path-denied error; no Evidence is written. |
| Production source uses an out-of-scope framework only | Emit any in-scope claims; the gap surfaces as `[unknown]` requirements in the spec. |
| The answer fails the gated schema or id grammar | The caller rejects it and asks for a repaired answer with the findings; correct the named claims. |
