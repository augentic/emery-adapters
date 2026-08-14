# TypeScript / JavaScript source survey

The engine invokes this prompt for a `typescript` source binding. Your job: walk the read-only CID view at `$SOURCE_DIR`, identify the source's framework surfaces using the grammar below, and return one lead block per surface. The caller persists the catalog; you never write `leads.md`.

JavaScript sources (`.js`, `.mjs`, `.cjs`, `.jsx`) fold into this prompt: the framework idioms are the same. Detect the file extension purely to widen the import-graph walk; the prompt content does not branch on it.

## Inputs

- **`$SOURCE_DIR`** — read-only CID view of the bound source root. Walk this tree; resolve `tsconfig.json` `paths` mappings relative to it. Absent when the binding is an inline `value`.
- **Source key** — kebab-case identifier the engine passed on the wire. The caller stamps each lead's `source` from it; this prompt does not emit it.
- **Optional parent lead** — when present, this is a focused survey: the exception path for a parent still coarser than a buildable boundary (a generated mega-handler), not a second walk to recover endpoints unfocused survey already emitted. Return stable child leads under that parent. Inherit parent/focus from the passed record; do not look it up in `leads.md` or `slices/`.

The bound directory is the only filesystem grant; the change home and `$PROJECT_DIR` are unreachable. Do not read `plan.yaml`, `leads.md`, or `slices/`. Treat the tree as read-only — no writes back into `$SOURCE_DIR`. Unfocused survey always returns the complete current set from `$SOURCE_DIR`; do not consult a catalog to decide this is a re-survey.

## Output: lead blocks

Emit one fenced block per identified surface, in the shape the CLI appends under `## Lead inventory`:

```markdown
### <lead>

- lead: <lead>
- synopsis: <reconciliation-grade headline>
- topics: [<optional-kebab-slugs>]
```

`lead` is kebab-case, derived from the surface identifier or handler path (e.g. `POST /users` → `user-registration`, `email.send` queue → `email-send`). It is the stable handle re-survey writes against (keyed by `(source, lead)`). After the CLI stamps `source`, the block validates against `schemas/discovery/lead.schema.json` (kebab-case `lead`, scalar `source`, content-bearing `synopsis`, optional kebab-case `topics`). One block per lead.

`synopsis` SHOULD name the handler's surface (route + method, queue + job, topic) and its salient behaviour/constraint so a same-slug lead from another source can be matched or distinguished on content, not just the shared slug. Prefer one line; it MAY run to a few lines when one is too thin. It stays plan-time headline material — extract-time behaviour belongs in `typescript.extract` claims, not here.

`topics` (optional) is an inline list of kebab-case domain slugs drawn from the surface (e.g. `[identity, http-route]`); author them only when the code clearly supports the classification, omit the bullet otherwise. They are extra grouping context for downstream reconciliation — never a grouping the CLI computes.

## Internal staging

Survey grammar is **adapter-internal** — there is no `surfaces.json` sibling artifact and no published schema for the intermediate shape. You MAY stage a working JSON document under `$SCRATCH_DIR/staged.json` to keep the framework walk auditable during the run; treat its shape as adapter-private (see [Working JSON shape](#working-json-shape)). Only lead blocks are visible to downstream synthesis; the staged JSON is retained in the scratch lane purely as an audit trail and is never read by any later phase.

## Framework grammar

Each row describes the import + call-site signature that qualifies one surface, the surface `kind` token, and how to compose the lead `id`.

- **Express** — `import` of `express` whose default export (or `Router`) has `.get` / `.post` / `.put` / `.patch` / `.delete` / `.all` / `.use` (mount only when it attaches a handler) called with a path string. Each route registration is one `http-route` surface.
- **Fastify** — `import` of `fastify`. `app.get` / `.post` / `.put` / `.patch` / `.delete` / `.route({ method, url, handler })` with a path string is `http-route`.
- **NestJS** — class decorated with `@Controller(...)` from `@nestjs/common`; each method decorated with `@Get` / `@Post` / `@Put` / `@Patch` / `@Delete` / `@Options` / `@Head` is `http-route`. Methods with `@MessagePattern` / `@EventPattern` from `@nestjs/microservices` are `message-sub`. Classes with `@WebSocketGateway` contribute one `ws-handler` per `@SubscribeMessage` method.
- **Next.js App Router** — files matching `app/**/route.{ts,js,tsx,jsx}` exporting an HTTP-verb function (`GET`, `POST`, `PUT`, `PATCH`, `DELETE`, `OPTIONS`, `HEAD`). The route path derives from the file path relative to `app/` with `[seg]` → `:seg` and `(group)` segments stripped. One `http-route` per verb.
- **Next.js Pages Router** — files under `pages/api/**.{ts,js}` whose default export is a request handler. Each file is one `http-route` (method defaults to `ANY` unless the handler narrows on `req.method`).
- **BullMQ** — `import` of `bullmq`. `new Queue(name, …)` + `queue.add(jobName, …)` is `message-pub` (one surface per `jobName`); the same call with `{ repeat: … }` additionally emits a `scheduled-job` keyed on the repeat expression. `new Worker(name, handler, …)` is `message-sub` per queue.
- **node-cron** — `cron.schedule(<cron>, <handler>)` is `scheduled-job`. Identifier is the cron expression.
- **`ws`** — `new WebSocketServer(...)` + `wss.on("connection", …)` is one `ws-handler` per server.
- **yargs / commander** — `.command(<name>, <desc>, <builder>, <handler>)` or `program.command(...).action(handler)` is one `cli-command` per registered command.
- **`fetch` / `axios` / typed SDK** — outbound HTTP / SDK call sites (`axios.<verb>(url, ...)`, `fetch(url, ...)`, `stripe.charges.create(...)`) whose target is an external URL or service is `external-call-out`. One surface per distinct call site.

Out of scope for v1: tRPC, GraphQL resolvers, gRPC services, AWS Lambda handlers, Cloudflare Workers. If the source uses one of those exclusively, return zero leads and let the operator review.

## Lead grain

1. **Walk the tree.** Survey framework call sites per the grammar above. Skip `node_modules`, `vendor`, `target`, `.venv`, `dist`, `build`, `*.d.ts`, and test directories (`test`, `tests`, `__tests__`, `spec`, `specs`, `*.test.*`, `*.spec.*`).
2. **One lead per framework surface.** One HTTP endpoint, one topic, one job, one CLI command, one WS handler, one outbound integration call site — each is one lead.

A lead is the smallest surface this adapter can name. It is NOT a slice and NOT a system-model element. Never merge surfaces; never cluster toward work units; never size leads by production LOC. Downstream consumers group or correlate leads themselves — the adapter emits what it can name.

## Path rules

Every internal staged reference to a file under `$SOURCE_DIR` MUST be a relative path:

- No leading `/`, no Windows drive letter (`C:\…`).
- No `..` segments.
- Resolves to a file under `$SOURCE_DIR`.
- Not under a skip-root (`node_modules`, `vendor`, `target`, `.venv`, `dist`, `build`).

A symlink inside `$SOURCE_DIR` pointing outside the bound root is denied at canonicalization; the host runner returns `source-survey-path-denied` and the slice stays `refining` per workflow §Extraction reliability.

## Working JSON shape

For internal staging only (not an artifact). Top-level: `{ version: 1, source, language, surfaces[] }`. Each surface: `{ id, kind, identifier, handler, touches[], declared-at[] }`. `kind` is one of `http-route | message-pub | message-sub | ws-handler | scheduled-job | cli-command | ui-route | external-call-out`. `handler` is `<file>:<symbol>` (named export, `<ClassName>.<method>`, verb export, `<file>:<line>` for inline arrows, `<file>:<framework>-handler-<n>` when the framework provides no name). `touches[]` is a static, file-level reach analysis: import-graph walk from the handler file through relative + `tsconfig.json` `paths`-aliased imports, stopping at bare module specifiers; include the handler file itself. `declared-at[]` carries the registration site (`<file>` or `<file>:<line>`).

You never publish this shape. Lead emission reads from it; only the lead blocks reach the caller.

## Worked example

Tiny Express service rooted at `$SOURCE_DIR`:

```
src/
├── server.ts          # Express setup; app.post("/users", registerUser)
├── users/
│   ├── register.ts    # registerUser handler; email validation
│   └── repository.ts  # insertUser
```

Framework signatures fired:

- `import express from "express"` + `app.post("/users", registerUser)` in `src/server.ts` → `http-route` `POST /users`, handler resolves through the named import to `src/users/register.ts:registerUser`, `touches` is `[src/server.ts, src/users/register.ts, src/users/repository.ts]`.

One surface, one lead. Resulting lead block:

```markdown
### user-registration

- lead: user-registration
- synopsis: Registration endpoint accepting email + password with RFC-5322 validation.
```

When a source has many surfaces, emit one block per surface in source order (alphabetical by handler path within the source) so re-survey produces stable diffs.

## Anti-patterns

- **Dead code.** A handler defined but never wired to a framework (no `app.post(...)`, no `@Get()`, no `new Worker(...)`) is not a surface. Survey from registration sites, not from likely-looking functions.
- **Feature-flag-disabled handlers.** A registration unambiguously disabled in production (`if (process.env.ENABLE_LEGACY === "1") app.post(...)`) is not a surface. When the guard is ambiguous, emit it and let the operator decide during plan review.
- **Hallucinated framework signatures.** If `package.json` does not depend on `bullmq`, do not emit BullMQ surfaces. Framework absence is dispositive.
- **Test files.** Skip `*.test.*`, `*.spec.*`, and anything under `tests/` or `__tests__/`. Tests validate production surfaces, they are not production surfaces.
- **Type-only `.d.ts` files in `touches`.** They declare ambient types, not behaviour; keep them out of the reach analysis.
- **Merging surfaces.** Two endpoints sharing a handler file are still two leads. Grouping surfaces into larger units is a downstream judgment, never this prompt's.
- **Cross-source coalescing.** This prompt only sees one source's tree. Cross-source reconciliation happens downstream in the engine — see [From sources to slices](../references/emery-runtime/reconciliation.md#plan-time-leads-become-slices) for how leads reconcile into slices.
- **Writing `leads.md` or `plan.yaml`.** Only lead blocks. The caller owns every lifecycle file.

## Failure modes

| Condition                                              | Action                                                                                                                          |
| ------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------- |
| `$SOURCE_DIR` empty / no recognised framework imports  | Return zero leads. The caller persists the empty set.                                                                  |
| Read denied outside `$SOURCE_DIR`                      | Host runner returns `source-survey-path-denied`; the slice stays `refining`.                                                 |
| Internal staged JSON malformed                         | Repair within the run; lead emission is the final consumer, not an external schema check.                             |
| Surface uses an out-of-scope framework (tRPC, gRPC, …) | Skip it. Return whatever in-scope leads the tree has; document the gap in the synopsis of the relevant source-level lead. |
