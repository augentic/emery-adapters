# TypeScript / JavaScript source survey

This prompt runs once per bound `typescript` source, before anything is extracted. The caller has parsed the tree already: the message lists every production module, says what the manifest names and whether an entry runs anything when loaded, lists every call that hands a function to something a package provides, every decorated class and method, what the entry modules export, and the packages imported — and lays the modules into the message whole, every line numbered, as far as they fit. Your job: from those facts, name each surface the source exposes — each one thing a caller outside the process does through it — anchor it at the lines where the code registers or declares it, and give it the stem its requirements lead with. You follow no import, group no module, and extract nothing. The caller derives the rest from the anchors you name: the modules each surface reaches, the lines a requirement may anchor at, the seams the [extract prompt](extract.md) mines, whose answers the engine reconciles into the specification — see [From sources to a spec](reconciliation.md).

Everything this call needs is in this prompt and the message. The references the `read_doc` tool offers are the extract prompt's; this call does not need them.

## Inputs

- **The manifest** — what `package.json` names: the package, `main`, `bin`, and the scripts with what each runs. Absent when the tree has none.
- **The bootstrap** — the entry that runs something when loaded, when the tree has one: the first module the manifest names, or the conventional entry the tree holds, that calls something at module level or imports a module for its effect. The caller names the bootstrap `start` itself and owns everything it does — configuration read, clients constructed, routers mounted, consumers subscribed, jobs scheduled, signal handlers, `stop` — so `start` is never a surface you list, and no surface you list leads with the stem `start`. When no entry runs at load, the message says so: the tree is a library, or an application a framework enters by convention.
- **Registrations** — every call, outside any handler, that hands a function to something a package provides, with the literal that led it, the package, and the type the receiver is constructed or typed as, at its lines (`` `src/routes/orders.ts#L12-L14` — `router.post` led by `"/"` handed a function, through `express` as `Router` ``). This is where a framework, a queue, a scheduler, or a CLI is told what to run: most surfaces are registered by one of these lines. A hook on a surface already registered — an error listener, a completion handler, `worker.on("failed")` — is that surface's behaviour, not a surface of its own.
- **Decorators** — every decorator a package provides, on a class or a method, with its literal, at its lines (`` `src/orders.controller.ts#L20-L24` — `@Post("")` on `OrdersController.create`, through `@nestjs/common` ``). A verb decorator on a method is a route; a class decorator carries the prefix.
- **Exports** — what the entry modules export: the modules the manifest names, and the ones no other module imports, each export with its kind and lines. In a tree with no bootstrap these are where a caller enters: a library's public functions and classes, or, under a framework that routes by file — `app/api/<resource>/route.ts` exporting `GET` and `POST`, `pages/api/<name>.ts` exporting a handler, a `functions/` directory of handlers — one surface per exported handler, anchored at the export.
- **Packages** — the bare specifiers the modules import, with the names bound to each and the modules binding them. The package a registration goes through tells what kind of surface it is: an HTTP framework registers routes, a CLI framework registers commands, a queue or broker client registers consumers, a scheduler registers jobs.
- **The modules** — every production module, listed by path; an `anchor` names one of these and nothing else. The leading ones are laid out whole with every line numbered: cite `#L<n>` from the numbers shown rather than reading them again; a module listed but not laid is read from `$SOURCE_DIR` only when the facts above leave its surfaces unclear.
- **`$SOURCE_DIR`** — read-only view of the bound source root, the whole tree, for a module the message lists rather than lays out.

Nothing outside the bound source is reachable; writes back into `$SOURCE_DIR` are denied.

## What a surface is

A surface is one thing the source does for a caller outside the process, reached at a boundary the source itself declares:

- a **route** — `POST /orders`, `GET /orders/:id` — registered on an HTTP framework, mapped by a verb decorator, or exported by a module a framework routes by file;
- a **command** — a CLI verb, a `bin` entry's subcommand;
- a **job** — a scheduled task, a queue worker, a topic or event consumer;
- an **exported API** — what a library's entry module makes importable: one surface per exported function or class, less an error type (`extends Error`, a name ending in `Error` or `Exception`), which is what a surface throws, and less a function whose body registers surfaces, which hosts them.

The grain is a reviewer's: one surface per route, per command, per job, per export. Several surfaces may anchor in one module — a router registering four routes is four surfaces at four anchors in one file, a CLI module registering three commands is three. A module no surface is registered or declared in — a service, a repository, a mapper, a client wrapper, a config loader — is no surface: the caller reaches it from the surfaces whose anchors import it, and mines it through them.

## Anchors

A surface's `anchor` is where the code registers or declares it, in the claim `path` grammar of [claims.md](claims.md): the module, relative to `$SOURCE_DIR`, and the lines of the registration call, the decorated method, or the exported declaration — `src/routes/orders.ts#L12-L14`, `src/orders.controller.ts#L20-L24`, `app/api/orders/route.ts#L5-L20`. Cite the lines the message gives for the registration, the decorator, or the export; a module alone, with no lines, anchors a surface the whole file declares and nothing else does. The caller follows the imports the anchored lines reference to find what the surface reaches, so an anchor at the registration — where the handler is named — reaches the handler's module; one at an unrelated line reaches the wrong code.

## Stems

The stem is the first segment of every `requirement` and `criterion` id the surface's extraction answers, and the engine slices the build plan by it, so two runs over one tree must name the same stems. Derive each by this convention, lowercase kebab-case, and by nothing else:

- a **route**'s stem is the first path segment that names a resource, past any prefix a mount adds, with `api`, `rest`, `internal`, and a version (`v1`, `v2`) skipped and a parameter (`:id`) never counted: `GET /api/v1/orders/:id` → `orders`; `POST /customers` → `customers`; `GET /health/live` → `health`;
- a **command**'s stem is its literal's first word: `command("import <file>")` → `import`; `reconcile --from` → `reconcile`;
- a **job**'s or **consumer**'s stem is the name its registration led with — the queue, topic, or job name — as its first word: `new Worker("invoices", …)` → `invoices`; `schedule("0 2 * * *", nightly)` led by a schedule rather than a name → the stem the handler's name gives, `nightly`;
- a registration led by no literal takes the stem of the type its receiver is constructed or typed as, kebab-cased: a `KafkaConsumer` subscribed with a callback → `kafka-consumer`; a `Router` with no path → the module's name;
- an **exported API**'s stem is the export's name kebab-cased: `class OrderService` → `order-service`; `function parseCsv` → `parse-csv`;
- a route a framework routes by file takes its stem from the path segments the file spells, by the route rule: `app/api/orders/[id]/route.ts` → `orders`.

Routes on one resource share one stem — `GET /orders`, `POST /orders`, `GET /orders/:id` are three surfaces under `orders` — and the caller tells them apart by their names. Middleware mounted for every route — a global guard, an error handler, a request log — is the bootstrap's behaviour under `start`, which the caller owns; a guard on one route is that route's. Name nothing `start`.

## Unreached modules

Every production module is reached by some surface — through the imports its anchor's lines reference, the bootstrap's among them — or it is not. List under `unreached` each module the surfaces you name do not reach and the bootstrap does not: dead code, a script no entry runs, a module you cannot place. The caller checks the coverage and returns any module in neither set as a finding: name the surface it belongs to, anchored at the registration whose lines import it, or list it. Never invent a surface to cover a module.

## Output

One JSON object matching the survey schema: `surfaces`, each with a `name`, an `anchor`, and a `stem`; and `unreached`, the modules no surface reaches, empty when every module is reached.

Rules:

- Every `anchor` names a module the message lists, relative to `$SOURCE_DIR`, with lines the file holds.
- Every `stem` is lowercase kebab-case, derived by the convention above.
- Every surface has a name, and no two surfaces share one; the name is what a caller does — `POST /orders`, `import command`, `invoices worker`, `OrderService` — never a description of the code.
- No surface leads with `start` when the tree has a bootstrap.
- A module no surface is registered or declared in is not a surface, however central.
- An empty `surfaces` is the answer when the tree declares no boundary — nothing registers a route, a command, a job, or a consumer, and no entry exports anything a caller would call. Do not invent one; the caller mines such a tree as a library.

## Worked example

A commander CLI bound as the source `ledger-cli`. The message says the manifest names `ledger-cli` with `bin` `dist/cli.js` and a `start` script running `tsx src/cli.ts`; the bootstrap is `src/cli.ts`, the caller's `start`; and lists:

> - `src/cli.ts#L14-L24` — `program.command` led by `"import <file>"` handed a function, through `commander` as `Command`
> - `src/cli.ts#L26-L38` — `program.command` led by `"reconcile"` handed a function, through `commander` as `Command`
> - `src/jobs/nightly.ts#L34-L47` — `cron.schedule` led by `"0 2 * * *"` handed a function, through `node-cron`
> - `src/workers/invoices.ts#L33-L48` — `new Worker` led by `"invoices"` handed a function, through `bullmq` as `Worker`
> - `src/workers/invoices.ts#L50` — `worker.on` led by `"failed"` handed a function, through `bullmq` as `Worker`
>
> Packages … `commander` — `Command` — in `src/cli.ts`; `node-cron` — `cron` (default) — in `src/jobs/nightly.ts`; `bullmq` — `Worker`, `Queue` — in `src/workers/invoices.ts`, `src/queues.ts`; `pg` — `Pool` — in `src/lib/db.ts` …

The modules are `src/cli.ts`, `src/config.ts`, `src/jobs/nightly.ts`, `src/lib/csv.ts`, `src/lib/db.ts`, `src/queues.ts`, `src/services/reconcile.ts`, `src/workers/invoices.ts`, and `scripts/seed.ts`, which nothing imports and no entry runs.

Resulting survey answer:

```json
{
  "surfaces": [
    { "name": "import command", "anchor": "src/cli.ts#L14-L24", "stem": "import" },
    { "name": "reconcile command", "anchor": "src/cli.ts#L26-L38", "stem": "reconcile" },
    { "name": "nightly job", "anchor": "src/jobs/nightly.ts#L34-L47", "stem": "nightly" },
    { "name": "invoices worker", "anchor": "src/workers/invoices.ts#L33-L48", "stem": "invoices" }
  ],
  "unreached": ["scripts/seed.ts"]
}
```

Four surfaces, each at the registration the message lists, each stem by the convention: the commands' literals' first words, the cron job's handler name since its literal is a schedule, the worker's queue name. `worker.on("failed")` is a hook on the worker already registered, so it is the worker's behaviour and no surface. `src/cli.ts` is the bootstrap: the caller lists it as `start` and owns the pool it opens and the Redis connection it drops, so it is not listed here, and the two commands anchored inside it lead with their own stems. `src/config.ts`, `src/lib/*`, `src/queues.ts`, and `src/services/reconcile.ts` are reached through the surfaces' anchors and the bootstrap, so they are named by none; `scripts/seed.ts` is reached by nothing and is listed as unreached rather than given a surface.

## Anti-patterns

- **Naming `start`.** The bootstrap is the caller's. What it constructs, mounts, and awaits is `start`'s behaviour by the caller's rule; you name what it registers, not the bootstrap itself.
- **Naming a module.** A service, a repository, a helper is not a surface; it is reached through the surfaces whose anchors import it.
- **Cutting by directory or layer.** `src/routes/*` is not a surface, nor is `src/services/*`. A surface is what a caller reaches, wherever the code sits.
- **Inventing a stem.** The stem is derived by the convention from the literal, the type, or the export — not chosen for elegance. `invoice-worker` for a worker on the queue `invoices` is wrong; `invoices` is right.
- **Anchoring at a handler's body.** The anchor is the registration, the decorator, or the export — where the code declares the surface — not the function that implements it.
- **Inventing a surface.** A module nothing reaches is listed under `unreached`; a tree that declares no boundary is answered `surfaces: []`.
- **Extracting.** This call names surfaces and nothing else; claims are the extract call's.

## Failure modes

| Condition | Action |
| --------- | ------ |
| The tree has one surface | Answer it alone. |
| A router registers many routes | One surface per route, all under the resource's stem; the caller tells them apart by name. |
| The tree is a subtree — routers without the bootstrap that mounts them | Answer the surfaces the tree itself registers; no bootstrap means no `start`, and a mounted prefix you cannot see is not guessed. |
| A framework routes by file and nothing registers | One surface per exported handler, anchored at the export, stem by the route rule over the file's path. |
| The tree declares no boundary | Answer `surfaces: []` and list nothing as unreached; the caller mines it as a library. |
| A module is reached by no surface you named and is not listed as unreached | The caller returns it as a finding: anchor the surface whose lines import it, or list it. |
| The answer names a module the tree lacks, a stem outside kebab-case, a surface twice, or a surface under `start` | The caller rejects it and asks again with the findings; correct the named surfaces. |
