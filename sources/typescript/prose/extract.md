# TypeScript / JavaScript source extract

This prompt runs once per seam of a bound `typescript` source. A tree of several production modules is mined one surface per call: the message names the surface — a route, a command, a job, an exported API — and the module a caller enters it at, and lends the whole tree under `$SOURCE_DIR`. A tree of one production module, and an inline value, are one seam mined whole: the message names no surface, and the seam is everything the module or value holds. Your job, given a surface: start at the entry, follow what the surface reaches, and emit one Evidence document covering the behaviour a caller observes through that surface. Given the whole: read the one module or value in full and emit the same document for every behaviour a caller observes through it. The caller joins the calls' answers into the source's one document, and the engine deterministically reconciles it with every other bound source's Evidence into the specification — see [From sources to a spec](reconciliation.md).

Everything this call needs is in this prompt and in `claims.md`, which the system prompt carries. The worked examples under [`references/examples/`](references/examples/README.md) are the only documents to load, and only when a claim's shape is still unclear after the worked example below: the README's table names the one nearest the surface, and one is enough.

## Inputs

- **`$SOURCE_DIR`** — read-only view of the bound source root, always the whole tree. Resolve imports and `tsconfig.json` `paths` mappings relative to it. Absent when the source is an inline `value` (the seam is then in the message); present, with no surface named, when the tree is one production module.
- **The surface** — when the message names one, its name and its entry module. Absent when the seam is mined whole.
- **Source name** — the kebab-case name the engine passed on the WIT bindings, which the specification cites the source by.

Nothing outside the bound source is reachable; writes back into `$SOURCE_DIR` are denied.

## Reading the tree

Read once, then answer: the entry, then each module the surface imports on the way to what it reaches, once each. `$SOURCE_DIR` is the root of every path you read and every `path` you anchor; what the tree does not hold — a package's internals, a declaration the tree does not carry — is not read and not invented.

**The manifest.** `package.json` `dependencies` name the frameworks in play (`express`, `fastify`, `@nestjs/core`, `koa`, `hono`, `commander`, `yargs`, `bullmq`, `kafkajs`, `@azure/service-bus`), which decide what a route, a command, or a consumer looks like. `tsconfig.json` `paths` and `baseUrl` resolve the aliases imports use (`@app/users` → `src/users`).

**Where behaviour starts.** A route's handler or controller method; a command's action function; a job's or consumer's callback — the `cron.schedule` callback, the queue `Worker` handler, the `consumer.run` or `onMessage` function; an exported API's exported functions and classes. From there follow imports outward: the services the handler calls, the repositories and clients they use, the types they take and return. Under a dependency-injection container (Nest `@Injectable()`, `inversify`, `tsyringe`) a constructor parameter names a token; what runs is the provider the production module binds it to — follow the binding, not the parameter type.

**The entry layer.** What sits between the caller and the handler is this surface's behaviour wherever the surface passes through it: middleware the route mounts (authentication, CORS, rate limits, body parsing, `zod` / `joi` / `class-validator` validation), error mapping (an error class translated to a status or an exit code), parameter sourcing (path, query, header, or body, and the coercion applied — `Number(req.query.limit)`). Global middleware every route passes through is claimed under this surface's noun for what it does to this surface's requests, never as a surface of its own.

**The boundary between surfaces.** Decide it once, by this rule, and do not revisit it:

- The **start surface** — the process's bootstrap: a `start` script, `src/index.ts`, `main.ts` — owns what runs before each handler is registered and at shutdown: configuration read, clients constructed, connections opened, routes mounted, consumers subscribed, jobs scheduled, signal handlers, `stop`.
- A **handler surface** — a route, a job, a consumer, a command, an export — owns everything from its handler's invocation onward: validation, branches, the calls it makes, what it publishes or returns, how it acknowledges.
- A value the start surface establishes and the handler uses — a client, a topic, a token, a cache — is named in the handler surface's requirement by the key or field the source spells (`Config.kafka.producer.topic`, `KAFKA_DEST_VP_TOPIC`); its construction is the start surface's and is not re-claimed.
- A module both reach — a repository, a validator, a client wrapper — is claimed for what this surface does with it, under this surface's noun, never for itself.

Mined whole there is no other surface: every behaviour the module or value exhibits is this call's to claim, each under the domain noun of the surface it belongs to.

## Claim kinds

This adapter emits from the closed enum:

| Kind | Required body field | When to emit |
|---|---|---|
| `requirement` | `statement` | One behavioural fact the code exhibits, stated as one present-tense sentence about the system. These are the claims deterministic reconciliation joins against documentation and intent. |
| `criterion` | `criterion` (free-form) | An acceptance boundary the source spells as a value of its own, covering the requirement whose id it extends — decided by the one rule below. |
| `excerpt` | `excerpt` (free-form) | A paragraph of focused context backing a requirement: the exact values, keys, defaults, and spans behind it. |
| `type` | `signature` (free-form), `name` | A declared interface, type alias, class, enum, or DTO the surface takes, returns, persists, or publishes, named by its declared identifier in `name` (`User`). |
| `call` | `callee` (free-form) | A call that leaves the process: outbound HTTP, a store, broker, cache, or identity-provider client, a metric or structured-log emission. |

**Lift every behavioural fact into a `requirement`.** Only `requirement` claims reach reconciliation ([reconciliation.md](reconciliation.md)); `excerpt` / `type` / `call` claims reach synthesis as supporting context. Every behavioural fact worth a spec block — a validation rule, an error response, a side effect, a timing — is a `requirement` with a `statement`, anchored by its `path` and backed by detail claims.

### Requirements

Read each handler for:

- **Inputs and validation** — which fields are read, which are required, and what happens when one is missing or malformed. A security check — path-traversal rejection, sanitisation, an authorisation check — is a validation like any other.
- **Branches and early returns** — every branch the caller can observe: a `return` inside an `if`, a `switch` arm, a ternary that changes the response. A branch left out is a behaviour the specification does not have.
- **Errors** — what is caught and what propagates; what is returned, thrown, or logged; whether execution continues. The extent of a `try` block is behaviour: a cache write inside it fails the way the fetch beside it fails. In a loop, whether one item's failure ends the operation or is recorded and the rest proceed.
- **Side effects** — every write, publish, delete, and cache set, with its key or topic as constructed and the condition under which it happens. One that completes before the caller's response and one after are different behaviours.
- **Sequencing and timing** — `await` in sequence is sequential; `Promise.all` is parallel; a call without `await` completes after the response. Delays and their placement (`sleep 5 s, then publish, repeated twice` is not `publish, sleep 5 s, publish`), intervals, timeouts, TTLs, retry counts and backoff, with the values the source spells.
- **Transformations** — what a `map`, `filter`, or `reduce` keeps, drops, or computes, where the result is what the caller gets.
- **Configuration** — the behaviour that depends on a `process.env` key or a constant, spelled exactly as the source spells it (`CC_STATIC_URL`, never a tidier name), and whether the handler fails without it. When a constant is a lookup table filtered at runtime, the requirement states the filter and the active subset, not the table.

**Stating a requirement.** One rule per statement, one sentence, present tense, about the system as the caller sees it: `Registration rejects an email that is not RFC-5322 valid with a 400 response.` A statement carries the value that is the rule — the status code, the limit, the duration, the count — and no more: a default, a hostname, a GUID, a URL template, a list of env keys is the backing `excerpt`'s to carry, named in the statement by its key. A handler that does five things is five requirements, not one sentence of five clauses. Nothing the code does not exhibit: a handler that does not enforce uniqueness has no uniqueness requirement; the engine renders the gap, you do not fill it.

**Ids.** Ids follow [claims.md](claims.md): `<surface>.<behaviour>`, the first segment the domain noun of the surface the behaviour belongs to — the one the message names, or, mined whole, the one the module exposes it through (`orders.…`, `user-registration.…`, `migrate.…`) — and the second the source's own name for the behaviour: the function, branch, error type, metric, or key that exhibits it (`orders.validate-quantity`, `train-update.no-actual-update`, `start.kafka-producer`), so a second run over the same code derives the same id. Never a paraphrase, a position, or a counter. Two calls that name one requirement with reworded statements manufacture a conflict, so claim a behaviour under the surface whose caller observes it, and once.

**Criteria.** Code states behaviour, not acceptance. A `criterion` is an acceptance boundary the source spells as a value of its own — a named threshold constant, a schema or validator definition, a validation pattern such as a regex literal — one a documentation source could state verbatim; its id extends its requirement's. An inline comparison in a guard (`items.length === 0`, `quantity < 1`, `state === "shipped"`) is behaviour: state it, with its value, in the `requirement` it belongs to and emit no criterion for it. Decide once per boundary and do not revisit it. Requirements without criteria surface as `[unknown]` acceptance gaps in the spec — that is honest output, not a failure to fix by inventing criteria.

### Calls

A `call` is a call that leaves the process, at the site this surface makes it: outbound HTTP (`fetch`, `axios`, an HTTP client); a store — SQL through `pg`, `mysql2`, `typeorm`, `prisma`, or `knex`; a managed table, document, or blob store through its SDK (`@azure/data-tables`, `mongodb`, `@azure/storage-blob`, `@aws-sdk/client-s3`); a cache (Redis, Memcached, an in-memory cache with a TTL); a message broker (Kafka, RabbitMQ, Service Bus, SQS); an identity provider (a token endpoint, `@azure/identity`); a WebSocket; and a metric, span, or structured-log emission that is part of the surface's contract. A call from one module of the tree to another is followed, not claimed: the store call inside a repository is the `call`; the handler's call to the repository is not.

- `path` at the call site; `callee` the client function — bare for a global (`fetch`), `<package>:<symbol>` for a package import (`axios:post`, `@azure/identity:getAzureToken`, `kafkajs:Producer.send`), `<file>:<symbol>` for a wrapper of the tree that makes the call (`src/lib/http.ts:HttpClient.post`); `synopsis` the operation and its target as constructed — the URL template, the topic, the entity and key columns, the metric name.
- Read from an outbound HTTP call site: the URL as constructed (base, path, and query, the exact template); method and headers with where each value comes from; the request body's type (a `type` claim); the response as the code deserialises and reads it (`await response.json()` assigned to `string[]`; `body.items[0].id`), never a broader declared interface; authentication and where the identity comes from; which statuses count as success and what the rest become; retries and timeouts as spelled. Best-effort and audit calls count: fire-and-forget is not unspecified.
- Read from a publish: the topic or queue as constructed (`${env}-${TOPIC}` is not `TOPIC`), the count from the loop bounds, the delay placement, the payload's type and whether it changes between rounds, the key and headers, and whether the caller's response waits for it.
- Read from a store access: the operation and the entity or table with its key columns (`SELECT TripsChanges WHERE (trip_id, start_date, start_time)`); the key pattern and TTL for a cache (`get data:{id}; on miss fetch and set with TTL 3600s`). A managed table store reached over HTTP by its SDK is a store, not an outbound API.

The behaviour a caller observes through the call — the timeout, the retry, the fallback, what the response becomes — is the `requirement` the call backs.

### Types

One `type` per interface, type alias, class, enum, or DTO the surface takes, returns, persists, or publishes: `name` its declared identifier, `signature` the declaration copied as written — never a paraphrase, which drifts (`number` for a `string` id, an optional field made required, a union flattened to its first member). Follow the shape down: a field typed by another interface is that interface's claim too; carry array and record element types, every union variant and its discriminant, enum members and their values. Where a decorator maps a property to a wire name — `@Expose({ name: "haEntrado" })`, `@JsonProperty("horaEntrada")`, a `@Transform` or `@JsonConverter` — carry the decorated declaration verbatim, and say in the synopsis what format the type crosses the wire in and which mappings are one-way (`@Exclude()`, `toClassOnly`, `toPlainOnly`). Optionality the code adds — `obj?.field`, `field ?? default`, `a || b` — goes in the synopsis when it disagrees with the declaration. A converter (`"true"` / `"false"` to boolean) is an `excerpt`, not a type. An imported type is claimed from its declaration where the tree holds it; a declaration the tree does not hold is not invented. A cast is not a check: `response.data as User` asserts a shape where a `zod` parse verifies it — the synopsis says which.

### Excerpts

An `excerpt` is a paragraph of focused context backing a requirement — the validation rule, the error response, the side effect, the keys and defaults the requirement names — never a paste of the span: the anchor is the citation. One claim per concept; two overlapping excerpts of one handler are noise.

## Anchors and paths

Every claim from the tree carries a `path` anchor in the grammar of [claims.md](claims.md): relative under `$SOURCE_DIR`, the tightest range that bounds the cited text, never a whole file where a range exists. Never under the [skip roots](claims.md#skip-roots) every adapter shares, nor under this adapter's own — `node_modules`, `vendor`, `target`, `.venv`, `dist`, `build`, `test`, `tests`, `__tests__` — nor in a `*.test.*`, `*.spec.*`, or `*.d.ts` file. Tests document expected behaviour; this adapter extracts observed behaviour from production source. A `.d.ts` declares ambient types, not behaviour: use the originating `.ts` where the tree holds it, and claim nothing when only a `.d.ts` is reachable.

## Worked example

A small Express service bound as the source `legacy-monolith`, mined for the surface `POST /users`, entered at `src/server.ts`:

- `src/server.ts` — `app.post("/users", registerUser)` at L5.
- `src/users/register.ts` — `registerUser` handler with email validation at L12–L34, the RFC-5322 regex it tests the email against at L14, and a delegation to `insertUser` at L31.
- `src/users/repository.ts` — the `User` interface at L1–L4 and `insertUser` at L6–L10, whose `pool.query` at L8 inserts the row.

Resulting Evidence body:

```json
{
  "claims": [
    { "kind": "requirement", "id": "user-registration.email-validation", "path": "src/users/register.ts#L12-L34", "statement": "Registration rejects an email that is not RFC-5322 valid with a 400 response." },
    { "kind": "criterion", "id": "user-registration.email-validation.pattern", "path": "src/users/register.ts#L14", "criterion": "The email matches the handler's RFC-5322 address regex." },
    { "kind": "requirement", "id": "user-registration.persistence", "path": "src/users/register.ts#L31", "statement": "A valid registration inserts the user and returns 201 with the persisted record." },
    { "kind": "excerpt", "path": "src/users/register.ts#L12-L34", "excerpt": "Handler validates email against RFC-5322 regex, returns 400 with { error: \"invalid-email\" } on failure, otherwise inserts the user and returns 201 with the persisted record." },
    { "kind": "type", "path": "src/users/repository.ts#L1-L4", "name": "User", "signature": "interface User { id: string; email: string; createdAt: Date }" },
    { "kind": "call", "path": "src/users/repository.ts#L8", "callee": "pg:Pool.query", "synopsis": "INSERT INTO users (id, email, created_at) VALUES ($1, $2, $3) RETURNING *" }
  ]
}
```

Two requirements for the spec, one criterion covering the first, and three detail claims backing them. The regex is the one boundary the handler spells as a value of its own, so it is the one criterion; the 400 and the 201 are the rules' values and sit in the statements. The `call` is the store write inside the repository — the one call that leaves the process — not the handler's call to `insertUser`, which is followed to reach it. What `src/server.ts` mounts besides `POST /users` is other surfaces' calls to claim.

## Before answering

Check the Evidence against the source, not against a picture of the finished document:

- Every behaviour a caller observes through this surface is a `requirement`: one rule, one present-tense sentence, an `id` led by this surface's noun and named for the code that exhibits it, a `path` at the span.
- Every branch, early return, error path, and side effect the caller can observe is stated.
- Every call that leaves the process is a `call` at its site with its target as constructed; no call between modules of the tree is one.
- Every type the surface takes, returns, persists, or publishes is a `type`, declaration verbatim, nested types included.
- Every key, constant, duration, count, status code, and topic is spelled as the source spells it — the rule's value in the statement, the rest in the excerpt.
- Nothing is claimed the code does not exhibit; nothing for another surface; nothing from a test, a `.d.ts`, a dependency, build output, or the engine's own files.
- Every `requirement` has a `statement`; every `criterion` has a `criterion` and an id extending its requirement's; every id is dotted kebab-case.

## Failure modes

| Condition | Action |
| --------- | ------ |
| The seam reaches no in-scope production source — a handler that is a stub, an export of types alone | Return `claims: []`; the engine preserves the gap rather than guessing. |
| Production source uses an out-of-scope framework only | Emit any in-scope claims; the gap surfaces as `[unknown]` requirements in the spec. |
