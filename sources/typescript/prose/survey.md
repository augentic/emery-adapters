# TypeScript / JavaScript source survey

This prompt runs once per bound `typescript` source, before anything is extracted. The caller lends the source tree as `$SOURCE_DIR` and asks which surfaces the source exposes: each one thing a caller outside the source reaches, and the module the caller enters it at. Your job: find the boundary, and nothing behind it. You follow no import, group no module, and extract nothing. The caller mines each surface in its own call under the [extract prompt](extract.md), lending the whole tree and naming the surface and its entry; that call starts at the entry and follows what the surface reaches. The calls' answers are joined into the source's one Evidence document — see [From sources to a spec](reconciliation.md).

## Inputs

- **`$SOURCE_DIR`** — read-only view of the bound source root, the whole tree. Read what declares the boundary: `package.json`, the bootstrap, routers, command registries, schedulers and consumers, public barrels.
- **An entry** — a `/`-separated path relative to `$SOURCE_DIR` to a production module: a `.ts`, `.tsx`, `.mts`, `.cts`, `.js`, `.jsx`, `.mjs`, or `.cjs` file of the source's own. Tests (`*.test.*`, `*.spec.*`, `tests/`, `__tests__/`), declaration files (`*.d.ts`), dependencies (`node_modules/`, `vendor/`), build output (`dist/`, `build/`, `target/`), and dot entries are not modules and enter nothing; the caller checks every entry against the tree and refuses one named there, or at no file.

Nothing outside the bound source is reachable; writes back into `$SOURCE_DIR` are denied.

## What a surface is

A surface is one thing the source does for a caller outside it, reached at a boundary the source itself declares:

- a **route** or route family — `POST /orders`, the `/users` routes — registered on an HTTP framework, or a serverless handler export;
- a **command** — a CLI verb, a `bin` entry, the module a `package.json` script runs (`start`, `serve`, `worker`, `migrate`); starting the process is a command, and what starting does — the port, the connections, the routers mounted — is its behaviour;
- a **job** — a scheduled task, a queue or topic consumer, an event handler the source subscribes;
- an **exported API** — what a library's `package.json` `exports` / `main` / `module`, or its public barrel, makes importable: one surface per export path, or per cohesive export when one barrel exposes several.

A surface's **entry** is the production module where the caller's request first meets the source's own code: the module that registers the route and holds or names its handler, the command's module, the consumer's module, the export path's target. Several surfaces may enter at one module — a router registering four routes is four surfaces at one entry, or one route family when the routes are one resource's operations over one handler module. A module no surface enters — a service, a repository, a mapper, a logger, a config loader — is no surface: it is reached from the surfaces that use it and mined through them.

Choose the grain a reviewer would name. Each surface is mined in its own call, which leads every requirement id with the surface's domain noun, so a surface should be one thing a caller does and no two surfaces should reach the same behaviour under different nouns.

## Method

1. **Read the manifest.** `package.json`: `bin` names commands; `exports` / `main` / `module` name what a library makes importable; `scripts` name what is run and its entry module. `tsconfig.json` `paths` resolve the aliases you meet on the way. [Component structure](references/component-structure.md) is the fuller procedure.
2. **Find the bootstrap.** The module the manifest or its scripts start — `src/index.ts`, `src/server.ts`, `src/main.ts`, a framework's `app.ts` — and what it mounts, registers, schedules, or subscribes. Each is a surface, entered at the module that declares it, and the bootstrap itself is the start command's entry.
3. **Stop at the boundary.** Do not follow a handler into its services and stores — the extract call does, from the entry you name. You need only where each surface is entered.
4. **Name each surface for what the caller does.** `POST /orders`, `/users routes`, `nightly reconciliation job`, `migrate command`, `start script`, `@acme/client` — the name is the seam's identity: the extract call leads every requirement id with its domain noun, so two surfaces never name one thing.

## Output

One JSON object matching the survey schema: `surfaces`, each with a `name` and its `entry`.

Rules:

- Every `entry` is a production module the tree holds, named by its path relative to `$SOURCE_DIR`.
- Every surface has a name, and no two surfaces share one.
- Several surfaces may enter at one module.
- A module no surface enters is not named. An internal module — a service, a repository, a logger, a base client — is reached from surfaces and mined through them, never listed as one.
- An empty `surfaces` is the answer when the tree declares no boundary: nothing registers a route, a command, a job, or an export. Do not invent a surface to have one — the caller refuses such a source as incomplete rather than mining what no caller reaches.

## Worked example

A small Express service whose production modules are:

- `src/index.ts` — creates the app, mounts `usersRouter` at `/users` and `ordersRouter` at `/orders`, starts the nightly job, listens on `PORT`; `package.json` runs it as `start`.
- `src/users/router.ts` — registers `POST /users` and `GET /users/:id`, handlers in `src/users/register.ts` and `src/users/lookup.ts`.
- `src/orders/router.ts` — registers `POST /orders`, handler in `src/orders/create.ts`.
- `src/jobs/nightly.ts` — the scheduled reconciliation, reading both repositories.
- `src/cli.ts` — the `bin` entry, registering a `migrate` command.
- `src/users/register.ts`, `src/users/lookup.ts`, `src/orders/create.ts`, `src/users/repository.ts`, `src/orders/repository.ts`, `src/lib/db.ts`, `src/lib/logger.ts` — what the surfaces reach.

Resulting survey answer:

```json
{
  "surfaces": [
    { "name": "start script", "entry": "src/index.ts" },
    { "name": "/users routes", "entry": "src/users/router.ts" },
    { "name": "POST /orders", "entry": "src/orders/router.ts" },
    { "name": "nightly reconciliation job", "entry": "src/jobs/nightly.ts" },
    { "name": "migrate command", "entry": "src/cli.ts" }
  ]
}
```

Five surfaces, five extract calls. The handlers, the repositories, `src/lib/db.ts`, and `src/lib/logger.ts` are named by none: each extract call reaches them from its surface's entry and claims what its caller observes there. The `/users` routes are one family — two operations on one resource over one router — where `POST /orders` stands alone; `src/index.ts` is both where the boundary is read and the start command's own entry, whose call claims what starting the service does and leaves each route to its own.

## Anti-patterns

- **Grouping.** Listing the modules behind a surface — its handler, its service, its repository — is not this call's. Name the surface and its entry; the extract call follows the rest.
- **Cutting by layer or directory.** `src/routes/*` is not a surface, nor is `src/services/*`. A surface is what a caller reaches, wherever the code sits.
- **Listing internals.** A helper, a repository, a base client is not a surface however central; it is reached through the surfaces that use it, and a call over it alone would claim behaviour no caller observes.
- **Inventing entries.** A file that is not production source of the tree — a test, a declaration file, a dependency, build output — or a path at no file is no entry. The caller refuses it.
- **Inventing surfaces.** A tree that declares no boundary exposes nothing; answer so.
- **Extracting.** This call finds the boundary and nothing else; claims are the extract call's, over each surface in turn.

## Failure modes

| Condition | Action |
| --------- | ------ |
| The tree has one surface | Answer it alone; one extract call mines the source through it. |
| A router registers many routes | A route family per resource when its routes are one handler module's operations; a route of its own when it does one distinct thing or has its own handler. Each surface costs one call. |
| The tree is a subtree of an app — routers without the bootstrap that mounts them — or modules with no manifest | Answer the surfaces the tree itself declares — a router's routes, a module's exports are its own boundary — and no more. |
| The tree declares no boundary — no route, command, job, or export | Answer `surfaces: []`. The caller refuses the source; it does not mine what no caller reaches. |
| The answer names an entry at no production module, a surface twice, or a nameless surface | The caller rejects it and asks again with the findings; correct the named surfaces. |
