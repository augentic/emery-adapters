# TypeScript / JavaScript source survey

This prompt runs once per bound `typescript` source, before anything is extracted. The caller has listed the production modules under `$SOURCE_DIR` and asks how to cut them into materials: the groups of modules each extract call will mine as one. Your job: read the tree and group the listed modules by the externally visible surface they serve. You extract nothing. The caller mines each group under the [extract prompt](extract.md), lending the whole tree and naming the group's modules, then joins the calls' answers into the source's one Evidence document — see [From sources to a spec](../references/emery-runtime/reconciliation.md).

## Inputs

- **`$SOURCE_DIR`** — read-only view of the bound source root, the whole tree. Read what you need to decide: entry points, routers, command registries, schedulers and queue consumers, `package.json` scripts, `tsconfig.json` `paths`.
- **The candidate modules** — the message lists every production module the caller will mine, named relative to `$SOURCE_DIR`. Name each exactly as listed. Anything else under the tree — tests, declaration files, dependencies, build output — is not yours to group.
- **The floor** — the message states how many modules a group needs before it is mined on its own. A smaller group, and every module you leave out, join one remainder the caller mines together.

Nothing outside the bound source is reachable; writes back into `$SOURCE_DIR` are denied.

## What a surface is

An externally visible surface is one thing the estate does for a caller outside it, together with the modules whose behaviour the caller observes through it:

- a **route** or route family — `POST /orders`, the `/users/*` router — with its handler, its validation, and the services and repositories the handler reaches to do its work;
- a **command** — a CLI verb, a `package.json` script's entry module — with what it runs;
- a **job** — a scheduled task, a queue or topic consumer, an event handler — with what it does;
- an **exported API** — a library's public entry module and the modules it composes.

Group by surface, not by directory. `routes/orders.ts` and `services/orders.ts` serve one surface; `routes/orders.ts` and `routes/users.ts` serve two, even though they share a directory. The extract call over a group claims the behaviour that group exhibits and leads every requirement id with the surface's domain noun, so a group that is one surface yields claims that name one thing, and two groups never name the same behaviour twice.

## Method

1. **Find the entry points.** The app's bootstrap (`index.ts`, `server.ts`, `main.ts`), the routers it mounts, the commands it registers, the schedulers and consumers it starts, the `exports` of a library's manifest. [Component structure](../references/component-structure.md) is the fuller procedure.
2. **Walk each surface from its handler.** Follow imports from the handler to the modules it reaches — a service, a repository, a mapper, a client — and take into the group those that exist for this surface. [Business logic](../references/business-logic.md) describes the handler-first walk.
3. **Place a shared module once, or not at all.** A module several surfaces reach — a shared repository, a logger, a config loader, a base client, the bootstrap that mounts every router — belongs to the one surface whose behaviour it exhibits, if there is one, and otherwise to no group. Leave it out: the remainder mines it, so nothing is lost and nothing is claimed twice.
4. **Name each group for its surface.** `POST /orders`, `orders router`, `nightly reconciliation job`, `migrate command`, `public client API` — the name is for the reviewer; the modules are for the caller.
5. **Mind the floor.** A surface of one module folds into the remainder however you group it; a group is worth naming when it has enough modules to be mined on its own, and a surface can still be named with one module when that is what it is.

## Output

One JSON object matching the survey schema: `groups`, each with a `name` and its `files`.

Rules:

- Every file is a candidate module, spelled exactly as the message listed it.
- No file appears in two groups, and no group is empty.
- A candidate you place in no group is fine: it joins the remainder. Leaving a module out is never a miss; naming one that was not offered, or naming one twice, is, and the caller will send the answer back with the finding.
- Do not invent a group for the leftovers. The remainder is the caller's.

## Worked example

A small Express service whose candidate modules are:

- `src/index.ts` — creates the app, mounts `usersRouter` and `ordersRouter`, starts the nightly job.
- `src/users/router.ts`, `src/users/register.ts`, `src/users/repository.ts` — the `/users` routes and what they reach.
- `src/orders/router.ts`, `src/orders/create.ts`, `src/orders/repository.ts` — the `/orders` routes and what they reach.
- `src/jobs/nightly.ts` — the scheduled reconciliation, reading both repositories.
- `src/lib/db.ts`, `src/lib/logger.ts` — the connection pool and the logger, reached by everything.

Resulting survey answer:

```json
{
  "groups": [
    { "name": "/users routes", "files": ["src/users/router.ts", "src/users/register.ts", "src/users/repository.ts"] },
    { "name": "/orders routes", "files": ["src/orders/router.ts", "src/orders/create.ts", "src/orders/repository.ts"] },
    { "name": "nightly reconciliation job", "files": ["src/jobs/nightly.ts"] }
  ]
}
```

Three surfaces. `src/index.ts`, `src/lib/db.ts`, and `src/lib/logger.ts` serve every surface and none in particular, so they are left out and the remainder mines them; under a floor of two, the one-module job folds into that remainder too, and the caller mines three materials: the users surface, the orders surface, and the rest.

## Anti-patterns

- **Grouping by directory.** `src/routes/*` as one group and `src/services/*` as another splits every surface in two, so two extract calls each see half of one behaviour and name it twice. The directory cut is what this survey replaces.
- **Forcing every module in.** A shared module placed in a surface it does not belong to is claimed under that surface's noun; left out, it is mined on its own terms in the remainder.
- **Inventing modules.** A module reached by an import but not among the candidates — a test, a declaration file, a dependency — is not yours to name. The caller refuses it.
- **Extracting.** This call decides the cut and nothing else; claims are the extract call's, over each group in turn.

## Failure modes

| Condition | Action |
| --------- | ------ |
| The tree has one surface | Answer one group holding its modules; the remainder takes the rest. |
| No surface is discernible — a bag of utilities, no entry point | Answer `groups: []`; the caller mines the whole tree as one remainder. |
| A candidate is unreadable or empty | Leave it out; the remainder mines it, and the extract call there decides what it exhibits. |
| The answer names a file not offered, a file twice, or an empty group | The caller rejects it and asks again with the findings; correct the named groups. |
