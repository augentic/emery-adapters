# TypeScript / JavaScript source extract

This prompt runs once per seam of a bound `typescript` source. The caller has parsed the tree and decided its structure before this call: the message names the seam's surfaces — where control enters the source from outside the process: a route, a command, a job, a consumer, an exported API, the process bootstrap — each with the module a caller enters it at, the lines that register it, the stem and the id its claims lead with, and the modules it reaches; lays the modules those surfaces reach into the message in full, every line numbered, or lists the ones too large to lay; lists the boundaries those modules spell as values of their own; lists the packages they import; and lists the calls they make through those packages. Your job: read what is laid, follow each surface from its entry through what it reaches, and emit one Evidence document covering the behaviour a caller observes through the seam's surfaces, each behaviour under the surface whose caller observes it, once. A tree small enough for one message is one seam over every module and every surface it exposes; a larger tree is one seam per stem, over the modules its surfaces reach; a tree the caller finds no surface in is one seam over every module under the one stem the message names — or, past the budget of one message, one seam per top-level directory under that directory's name — read as a library is read, for what its exports do for a caller; an inline value is one seam mined whole, with no surface named, read for every surface it exposes. The caller joins the calls' answers into the source's one document, and the engine deterministically reconciles it with every other bound source's Evidence into the specification — see [From sources to a spec](reconciliation.md).

Everything this call needs is in this prompt, in `claims.md`, which the system prompt carries, and in the message. The worked examples under [`references/examples/`](references/examples/README.md) are the only documents to load, and only when a claim's shape is still unclear after the worked example below: the README's table names the one nearest the surface, and one is enough.

## Inputs

- **The surfaces** — one line each: `` Surface `<name>` — entry `<module>` — stem `<stem>`: <notes>; id `<id>`; reaches <modules> ``. The notes say where the surface is registered (the lines, the function or method, the package the receiver comes from), the lines of its handler when it is a function at the registration, the hooks folded into it (`worker.on("failed")`), or the methods an exported class carries. The `id` is what every `requirement` and `criterion` of the surface leads with — the stem alone when the surface is the stem's only one, the stem extended by what tells it apart when several share the stem (`orders.get-id`, `orders.place-order`); a class's line gives `ids`, one more per public method (`money.add`). `reaches` names the modules the surface's code reaches beyond its entry — the ones read for its rules — or says it reaches nothing beyond its entry. `start` is the process bootstrap — the entry module the manifest names or the tree's conventional entry — and owns what runs before each handler is registered and at shutdown. When the caller found no surface, the message says so in place of the surface lines and names the one stem every `requirement` and `criterion` id leads with: read the modules as a library — each export for what it does for a caller — and name each behaviour for the export that exhibits it.
- **The modules** — the production modules the seam reaches, the entry first, laid out in full with every line numbered, or listed by path when they are too large to lay out. Every `path` you anchor names one of them; a claim anchored elsewhere is refused. When a module is laid out, cite `#L<n>` from the numbers shown and read nothing again; when it is listed, read it from `$SOURCE_DIR`.
- **Boundaries** — every value the modules spell as a value of its own, at its exact anchor: a `process.env` read with the default beside it, a named constant, a pattern literal, a class field's initializer. This is the list a `criterion` is drawn from.
- **Packages** — the bare specifiers the modules import, with the names bound to each and the modules binding them.
- **Calls** — every call the modules make through a package, grouped by callee with the lines of its sites (`` `pg:Pool.query` in `src/repositories/orders.ts` at L53-L56, L61 ``), less what is structure: constructions, registrations, mounts, lifecycle calls, and what a module-level declaration makes. This is the list a `call` is drawn from, and its `callee` is spelled as the list spells it.
- **Decision points** — every guard, switch, conditional, throw, catch, and timer the modules turn on, each at its lines with its text and the function it runs in (`` `src/main.ts#L51-L53` — `if (!Config.isSlot)` in `Main.start` ``). This is where a `requirement` anchors: at a decision point, at a `return` that yields a function's result, at a listed call or a package's construction, at a boundary, at the code a stated behaviour names, or at a surface's registration or handler lines — the caller holds every `requirement` to those lines, and one anchored elsewhere comes back to re-anchor or drop. A line that only wires or assigns — a field set, a value passed on, a call into the tree's own module — is no requirement's anchor; what `start` constructs with a boundary's value is one requirement at that construction, named for the boundary. Absent when the modules decide nothing.
- **Stated behaviours** — what the tree's own tests state about the seam's modules, each at its line: a test case's title under its suites' titles (`` `test/key-vault.test.ts#L23` — Test loading configuration › When load secret is empty, should throw error ``), a feature file's scenarios under its feature. The test files follow the laid modules, laid when they fit and listed otherwise, so their assertions can be read for the values they hold. A stated behaviour the laid code confirms is a `requirement` anchored at the code that exhibits it — never at the test — with an id named for that code and a statement that says what the code does, in the present tense, drawing on the test's wording; a stated behaviour the code does not hold is not invented, and a test that states nothing the code does is left alone. Absent when the tree carries no test.
- **`$SOURCE_DIR`** — read-only view of the bound source root, needed only for a module the message lists rather than lays out. Absent when the source is an inline `value` (the seam is then in the message).
- **Source name** — the kebab-case name the engine passed on the WIT bindings, which the specification cites the source by.

Nothing outside the bound source is reachable; writes back into `$SOURCE_DIR` are denied.

## Reading the modules

Read once, then answer: each surface's registration and handler at the lines the surface line gives, then each module its line says it `reaches`, once each. A module a surface reaches is read for that surface's rules as the handler is: a fetch's retry and what a failure leaves in place, a secret parsed and refused when empty, a mapping's arms, an interval that re-runs a fetch — each is a `requirement` under the surface that reaches it, never a `call` alone. What the tree does not hold — a package's internals, a declaration the tree does not carry — is not read and not invented.

**Where behaviour starts.** The surface line points at it: a route's handler or controller method; a command's action; a job's or consumer's callback; an exported API's function or class. From there follow imports outward through the laid modules: the services the handler calls, the repositories and clients they use. Under a dependency-injection container (Nest `@Injectable()`, `inversify`, `tsyringe`) a constructor parameter names a token; what runs is the provider the production module binds it to — follow the binding, not the parameter type.

**The entry layer.** What sits between the caller and the handler is this surface's behaviour where the surface itself mounts it: middleware the route or router mounts (a guard on one route, a rate limit on one router, a body parser or `zod` / `joi` / `class-validator` validation at the route), parameter sourcing (path, query, header, or body, and the coercion applied — `Number(req.query.limit)`), and what the handler itself returns on failure. Middleware mounted for every route — `app.use(auth)`, an error handler that maps every error class to a status, a request log — is `start`'s: claimed once, where it is mounted, for what it does to every request. A route seam names it by its key (`authenticate`, `errorHandler`) where a behaviour depends on it — the 401 a route's caller sees is the global guard's, not the route's — and never claims it again.

**The boundary between surfaces.** Decide it once, by this rule, and do not revisit it:

- The **`start` surface** owns what runs before each handler is registered and at shutdown: configuration read, clients constructed, connections opened, routes mounted, middleware mounted for every route and what it does to each request — the global guard's 401, the error mapping's statuses, the request log's line — consumers subscribed, jobs scheduled, signal handlers, `stop`.
- A **handler surface** — a route, a job, a consumer, a command, an export — owns everything from its handler's invocation onward: validation, branches, the calls it makes, what it publishes or returns, how it acknowledges. Its hooks (`worker.on("failed")`) are its behaviour.
- A value `start` establishes and a handler uses — a client, a topic, a token, a cache — is named in the handler surface's requirement by the key or field the source spells (`Config.kafka.producer.topic`, `KAFKA_DEST_VP_TOPIC`); its construction is `start`'s and is not re-claimed.
- A module several surfaces reach — a repository, a validator, a client wrapper — is claimed for what this seam's surfaces do with it, under their stems, never for itself. What it does for a surface of another seam is that seam's call to claim.

A boundary a dependency serves for the source — a plugin's health route, a library's metrics endpoint — has no handler of the source's own: `start` claims that it is mounted, where, and with which options, and nothing of what it answers. An inline value names no surface: find the surfaces it exposes as the caller would — what it registers, decorates, or exports — and claim each behaviour under that surface's noun.

## Claim kinds

This adapter emits from the closed enum:

| Kind | Required body field | When to emit |
|---|---|---|
| `requirement` | `statement` | One behavioural fact the code exhibits, stated as one present-tense sentence about the system. These are the claims deterministic reconciliation joins against documentation and intent. |
| `criterion` | `criterion` (free-form) | An acceptance boundary the source spells as a value of its own — one of the message's Boundaries — covering the requirement whose id it extends. |
| `excerpt` | `excerpt` (free-form) | A paragraph of focused context backing a requirement: the exact values, keys, defaults, and spans behind it. |
| `call` | `callee` (free-form) | A call that leaves the process: outbound HTTP, a store, broker, cache, or identity-provider client, a metric or structured-log emission. |

**Emit no `type`.** The caller copies every `interface`, `type`, `enum`, and class the seam's modules export from the code, verbatim, into the source's document after this call answers; a `type` this call answers is dropped. Where a shape decides a behaviour — a union a branch switches on, a field a decorator maps to a wire name, a field the code makes optional that the declaration does not, a shape a module keeps to itself — state it in the `requirement` or the `excerpt` that needs it.

**Lift every behavioural fact into a `requirement`.** Only `requirement` claims reach reconciliation ([reconciliation.md](reconciliation.md)); `excerpt` and `call` claims reach synthesis as supporting context. Every behavioural fact worth a spec block — a validation rule, an error response, a side effect, a timing — is a `requirement` with a `statement`, anchored by its `path` and backed by detail claims.

### Requirements

Read each handler, and each module it reaches, for:

- **Inputs and validation** — which fields are read, which are required, and what happens when one is missing or malformed. A security check — path-traversal rejection, sanitisation, an authorisation check — is a validation like any other.
- **Branches and early returns** — every branch the caller can observe: a `return` inside an `if`, a `switch` arm, a ternary that changes the response. A branch left out is a behaviour the specification does not have.
- **Errors** — what is caught and what propagates; what is returned, thrown, or logged; whether execution continues. The extent of a `try` block is behaviour: a cache write inside it fails the way the fetch beside it fails. In a loop, whether one item's failure ends the operation or is recorded and the rest proceed.
- **Side effects** — every write, publish, delete, and cache set, with its key or topic as constructed and the condition under which it happens. One that completes before the caller's response and one after are different behaviours.
- **Sequencing and timing** — `await` in sequence is sequential; `Promise.all` is parallel; a call without `await` completes after the response. Delays and their placement (`sleep 5 s, then publish, repeated twice` is not `publish, sleep 5 s, publish`), intervals, timeouts, TTLs, retry counts and backoff, with the values the source spells.
- **Transformations** — what a `map`, `filter`, or `reduce` keeps, drops, or computes, where the result is what the caller gets.
- **Configuration** — the behaviour that depends on a `process.env` key or a constant, spelled exactly as the source spells it (`CC_STATIC_URL`, never a tidier name), and whether the handler fails without it. The Boundaries list is where each is read and what it defaults to. When a constant is a lookup table filtered at runtime, the requirement states the filter and the active subset, not the table.

**Stating a requirement.** One rule per statement, one sentence, present tense, about the system as the caller sees it: `Registration rejects an email that is not RFC-5322 valid with a 400 response.` A statement carries the value that is the rule — the status code, the limit, the duration, the count — and no more: a default, a hostname, a GUID, a URL template, a list of env keys is the backing `excerpt`'s to carry, named in the statement by its key. A handler that does five things is five requirements, not one sentence of five clauses. Nothing the code does not exhibit: a handler that does not enforce uniqueness has no uniqueness requirement; the engine renders the gap, you do not fill it.

**Ids.** Ids follow [claims.md](claims.md): `<id>.<behaviour>`. The `<id>` is the surface line's — the stem alone, or the stem extended by what tells the surface apart — and the caller holds every `requirement` and `criterion` id to the stem it leads with; an id under another stem is refused and the answer put again. The `<behaviour>` is the source's own name for the behaviour: the function, branch, error type, metric, or key that exhibits it (`orders.place-order.reject-empty-lines`, `train-update.no-actual-update`, `start.kafka-producer`); the surface's id alone names the surface as a whole — what it does for the caller when nothing intervenes; a class method's behaviour extends the method's id (`money.add.currency-mismatch`). Never a paraphrase, a position, or a counter. Two calls that name one requirement with reworded statements manufacture a conflict, so claim a behaviour under the surface whose caller observes it, and once. Two ids under one stem are two requirements the engine never merges, so one behaviour is one id, however many branches state it. An inline value names no stem: lead each id with the noun of the surface the value exposes it through.

**Criteria.** Code states behaviour, not acceptance. A `criterion` is an acceptance boundary the source spells as a value of its own — one the message's Boundaries list anchors: a named threshold constant (`MAX_ITEMS = 50`), a schema or validator definition, a pattern literal bound to a name, a configuration default the code falls back to (`PORT = Number(process.env.PORT ?? 3000)`) — one a documentation source could state verbatim. Its `path` is the boundary's anchor as listed; its id extends its requirement's; its `criterion` states the boundary as a checkable sentence with the value and its unit. An inline comparison in a guard (`items.length === 0`, `quantity < 1`, `state === "shipped"`) is behaviour: state it, with its value, in the `requirement` it belongs to and emit no criterion for it. The test is whether the value is declared once and referred to — listed under Boundaries — or written into the branch that uses it. A boundary no requirement rests on is no criterion. Requirements without criteria surface as `[unknown]` acceptance gaps in the spec — that is honest output, not a failure to fix by inventing criteria.

### Calls

A `call` is a call that leaves the process, at the site this seam's surface makes it, drawn from the message's Calls list: outbound HTTP (`fetch`, `axios`, an HTTP client); a store — SQL through `pg`, `mysql2`, `typeorm`, `prisma`, or `knex`; a managed table, document, or blob store through its SDK (`@azure/data-tables`, `mongodb`, `@azure/storage-blob`, `@aws-sdk/client-s3`); a cache (Redis, Memcached, an in-memory cache with a TTL); a message broker (Kafka, RabbitMQ, Service Bus, SQS); an identity provider (a token endpoint, `@azure/identity`); a WebSocket; and a metric, span, or structured-log emission that is part of the surface's contract. The list holds every call the modules make through a package, so read it as candidates: a validator's `parse`, a logger's `info`, a date or XML helper, a framework's response method does not leave the process and is no `call` — unless its emission is the surface's contract, as a metric or an audit log is. A call from one module of the tree to another is followed, not claimed: the store call inside a repository is the `call`; the handler's call to the repository is not. Every listed call that leaves the process has a requirement beside it — the behaviour the caller observes through it — and every one of this seam's surfaces' sites is claimed once.

- `path` at the call site; `callee` as the Calls list spells it — bare for a global (`fetch`), `<package>:<symbol>` for a package import (`axios:post`, `@azure/identity:getAzureToken`, `pg:Pool.query`), and `<file>:<symbol>` for a wrapper of the tree that makes the call when the list shows the site inside the wrapper (`src/lib/http.ts:HttpClient.post`); `synopsis` the operation and its target as constructed — the URL template, the topic, the entity and key columns, the metric name.
- Read from an outbound HTTP call site: the URL as constructed (base, path, and query, the exact template); method and headers with where each value comes from; the response as the code deserialises and reads it (`await response.json()` assigned to `string[]`; `body.items[0].id`), never a broader declared interface; authentication and where the identity comes from; which statuses count as success and what the rest become; retries and timeouts as spelled. Best-effort and audit calls count: fire-and-forget is not unspecified.
- Read from a publish: the topic or queue as constructed (`${env}-${TOPIC}` is not `TOPIC`), the count from the loop bounds, the delay placement, the payload's shape and whether it changes between rounds, the key and headers, and whether the caller's response waits for it.
- Read from a store access: the operation and the entity or table with its key columns (`SELECT TripsChanges WHERE (trip_id, start_date, start_time)`); the key pattern and TTL for a cache (`get data:{id}; on miss fetch and set with TTL 3600s`). A managed table store reached over HTTP by its SDK is a store, not an outbound API.

The behaviour a caller observes through the call — the timeout, the retry, the fallback, what the response becomes, what is kept when it fails — is the `requirement` the call backs; a `call` with no requirement beside it is a behaviour left unstated.

### Excerpts

An `excerpt` is a paragraph of focused context backing a requirement — the validation rule, the error response, the side effect, the keys and defaults the requirement names — never a paste of the span: the anchor is the citation. One claim per concept; two overlapping excerpts of one handler are noise.

## Anchors and paths

Every claim from the tree carries a `path` anchor in the grammar of [claims.md](claims.md): relative under `$SOURCE_DIR`, the tightest range that bounds the cited text, never a whole file where a range exists, within the modules the message laid out or listed. Never under the [skip roots](claims.md#skip-roots) every adapter shares, nor under this adapter's own — `node_modules`, `vendor`, `target`, `.venv`, `dist`, `build` — nor in a `*.d.ts` file: the caller kept none of them, so none is laid, and a claim anchored in one is refused. A test file the message lists under Stated behaviours is read, never anchored by a `requirement` or a `criterion`: tests state expected behaviour; this adapter extracts the behaviour the production source exhibits, and anchors it there.

## Worked example

A small Express service bound as the source `legacy-monolith`, past the budget of one message, so this call is the seam under the stem `user-registration`. The message reads, before the laid modules:

> This call mines the surface under the stem `user-registration` alone:
>
> - Surface `POST /users` — entry `src/server.ts` — stem `user-registration`: registered L5 at module level; through `express`; id `user-registration`; reaches `src/users/register.ts`, `src/users/repository.ts`.
>
> Boundaries the code spells as values of their own, each at its line …
>
> - `src/users/register.ts#L3` — `WELCOME_DELAY_MS = Number(process.env.WELCOME_DELAY_MS ?? 60000)`
> - `src/users/register.ts#L14` — `EMAIL = /^[^\s@]+@[^\s@]+\.[^\s@]+$/`
>
> Packages these modules import, with the names bound to them …
>
> - `express` — `express` (default) — in `src/server.ts`
> - `pg` — `Pool` — in `src/users/repository.ts`
>
> Calls these modules make through a package, each callee with the lines of its sites …
>
> - `pg:Pool.query` in `src/users/repository.ts` at L8

The laid modules: `src/server.ts` — `app.post("/users", registerUser)` at L5; `src/users/register.ts` — the `registerUser` handler at L12–L34, testing the email against `EMAIL` at L15, delegating to `insertUser` at L31, and scheduling the welcome mail at L33 after `WELCOME_DELAY_MS`; `src/users/repository.ts` — the exported `User` interface at L1–L4 and `insertUser` at L6–L10, whose `pool.query` at L8 inserts the row.

Resulting Evidence body:

```json
{
  "claims": [
    { "kind": "requirement", "id": "user-registration.email-validation", "path": "src/users/register.ts#L12-L34", "statement": "Registration rejects an email that does not match the EMAIL pattern with a 400 response." },
    { "kind": "criterion", "id": "user-registration.email-validation.pattern", "path": "src/users/register.ts#L14", "criterion": "The email matches /^[^\\s@]+@[^\\s@]+\\.[^\\s@]+$/." },
    { "kind": "requirement", "id": "user-registration.persistence", "path": "src/users/register.ts#L31", "statement": "A valid registration inserts the user and returns 201 with the persisted record." },
    { "kind": "requirement", "id": "user-registration.welcome-mail", "path": "src/users/register.ts#L33", "statement": "A valid registration schedules a welcome mail after the delay WELCOME_DELAY_MS names, once the response is sent." },
    { "kind": "criterion", "id": "user-registration.welcome-mail.delay", "path": "src/users/register.ts#L3", "criterion": "The welcome mail is scheduled WELCOME_DELAY_MS milliseconds after registration, 60000 when the variable is unset." },
    { "kind": "excerpt", "path": "src/users/register.ts#L12-L34", "excerpt": "Handler tests the email against EMAIL, returns 400 with { error: \"invalid-email\" } on failure, otherwise inserts the user, returns 201 with the persisted record, and schedules the welcome mail WELCOME_DELAY_MS (default 60000 ms) later without awaiting it." },
    { "kind": "call", "path": "src/users/repository.ts#L8", "callee": "pg:Pool.query", "synopsis": "INSERT INTO users (id, email, created_at) VALUES ($1, $2, $3) RETURNING *" }
  ]
}
```

Three requirements for the spec, two criteria, and two detail claims backing them. The pattern and the delay default are the two Boundaries the handler rests on, so they are the two criteria, each at the anchor the list gives; the 400 and the 201 are the rules' values and sit in the statements, and `persistence` stays uncovered rather than gaining an invented criterion. Every id leads with `user-registration`, the id the surface line gives — the stem alone, as the surface is the stem's only one. The `call` is the store write inside the repository — the one call that leaves the process, its `callee` and site as the Calls list spells them — not the handler's call to `insertUser`, which is followed to reach it. The `User` interface is no claim of this answer: the caller copies it from `src/users/repository.ts#L1-L4`. What `src/server.ts` mounts besides `POST /users` is other seams' calls to claim.

## Before answering

Check the Evidence against the source, not against a picture of the finished document:

- Every behaviour a caller observes through this seam's surfaces is a `requirement`: one rule, one present-tense sentence, an `id` led by its surface line's id and named for the code that exhibits it, a `path` at the span.
- Every branch, early return, error path, and side effect the caller can observe is stated.
- Every Boundary a requirement rests on is a `criterion` extending that requirement's id, at the anchor the list gives, with the value and its unit; every inline comparison stays in its requirement's statement; no Boundary is a criterion on its own.
- Every call that leaves the process is a `call` at its site with its target as constructed and its `callee` spelled as the Calls list spells it; no call between modules of the tree is one, and no listed helper, validator, or logger is one unless its emission is the surface's contract.
- No `type` is answered; a shape a behaviour turns on is in the statement or the excerpt that needs it.
- Every key, constant, duration, count, status code, and topic is spelled as the source spells it — the rule's value in the statement, the rest in the excerpt.
- Every `path` names a module the message laid out or listed, with lines the file holds; nothing is anchored in a file the tree does not hold.
- Nothing is claimed the code does not exhibit — a stated behaviour the code confirms is claimed at the code, one it does not hold is not; nothing for another seam's surface; nothing anchored in a test, a `.d.ts`, a dependency, build output, or the engine's own files.
- Every `requirement` has a `statement`; every `criterion` has a `criterion` and an id extending its requirement's; every id is dotted kebab-case.

## Failure modes

| Condition | Action |
| --------- | ------ |
| The seam reaches no in-scope production source — a handler that is a stub, an export of types alone | Return `claims: []`; the engine preserves the gap rather than guessing. |
| Production source uses an out-of-scope framework only | Emit any in-scope claims; the gap surfaces as `[unknown]` requirements in the spec. |
