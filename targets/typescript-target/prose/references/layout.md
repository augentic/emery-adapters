# Layout

The one shape every slice of a TypeScript build follows, so that slices built beside one another in a wave merge clean and the integrated tree compiles as a whole.

## The tree

```
.gitignore
package.json             the root manifest: static, names no package of the program's
package-lock.json        written by npm install, never by hand
tsconfig.json
src/
  index.ts               the entry: one line per stem
  main.ts                the runner, written with the manifests by the start slice
  <stem>/                a stem one slice builds whole
    package.json         the module's manifest: the packages its files import
    index.ts             the module: what the rest of the tree may import
    <more>.ts            the module's own files, re-exported from its index
  <stem>/                a stem several slices build
    index.ts             a list alone: one line per part
    <part>/
      package.json       the part's manifest, as a module's
      index.ts           the part's module
      <more>.ts
test/
  <stem>.test.ts         one test per acceptance scenario of a stem built whole
  <stem>/
    <part>.test.ts       one test per acceptance scenario of a part
```

- `<stem>` is the first segment of a requirement's dotted id: `orders` for `orders.create`, `start` for `start.listen`.
- A slice named by its stem — the slice `orders` holding `orders.*` — builds the stem whole and owns `src/<stem>/`.
- Any other slice builds a part of each stem its requirements lead with and owns `src/<stem>/<part>/`, where `<part>` is the slice's name less the stem and the hyphen after it where the name leads with them, else the name whole: `reading` for the slice `orders-reading`, `application-bootstrap` for the slice of that name under `start`. Under the stem, it writes that directory, its lines in the stem's `index.ts`, and nothing else.
- Slice names are unique in the plan, so two slices never own one directory, and at most one builds a stem flat.
- A directory holds everything its requirements need: its files, and its manifest naming the packages they import. A file beside the `index.ts` is private to the directory until the `index.ts` re-exports it.
- Relative imports name the file with its extension: `import { createOrder } from "./create.ts";` within a directory, `import type { Order } from "../orders/index.ts";` across modules. Node runs the `.ts` files as they are, so a type-only import is spelled `import type`.
- A module imports another stem through that stem's `index.ts` alone, never a file beneath it; a part reaches a sibling part the same way, through their stem's `index.ts`.
- A module another slice owns is not yours to change. Where the design has yours depend on it and the tree lacks it, implement against the design's types in your own directory and leave the import for the slice that lands it.

## The lists

`src/index.ts` is a list, one line per stem, in any order:

```ts
export * as orders from "./orders/index.ts";
export * as paymentMethods from "./payment-methods/index.ts";
export * as start from "./start/index.ts";
```

A slice adds its stem's line and changes no other; a part adds its stem's line where it is missing, the same line whichever part writes it. The engine keeps both sides' lines of every `index.ts`, so two slices adding a line each at once merge clean; a line that does anything but re-export breaks that. The camel-cased stem is the namespace: `orders`, `paymentMethods`.

A stem built in parts has the same list one level down, `src/<stem>/index.ts`, one line per part, and, where the stem is `start`, one more from the part that holds `main()`:

```ts
export * as bootstrap from "./bootstrap/index.ts";
export * as httpMiddleware from "./http-middleware/index.ts";
export { main } from "./bootstrap/index.ts";
```

## The wiring

Where the plan has a `start` stem, its module is the program: `src/start/index.ts` exports `main()`, which builds the application the design names, wires every module into it, and runs it. The root manifest's `start` script runs `src/main.ts`, two lines the slice holding `main()` writes with the manifests, byte for byte:

```ts
import { main } from "./start/index.ts";

await main();
```

A tree with no `start` stem is a library: its entry is `src/index.ts`, nothing runs it, and no slice writes `src/main.ts`.

The wiring is one surface, and the bootstrap never names a module. A module the application reaches — one that attaches routes, middleware, handlers, or subscribers — exports `register(app)`, taking the application the design names as the framework's own instance type (`import type { Express } from "express"`, a Fastify instance, a `node:http` server), and attaches its own to it. The bootstrap imports the entry whole (`../index.ts` from `src/start/`, `../../index.ts` from a part of it) and calls every `register` it finds, a stem's or a part's, in name order, since a module namespace lists its exports sorted:

```ts
import type { Express } from "express";

import * as modules from "../index.ts";

type Registering = { register: (app: Express) => void };

function registering(value: unknown): value is Registering {
  return typeof value === "object" && value !== null
    && typeof (value as Partial<Registering>).register === "function";
}

export function wire(app: Express): void {
  for (const stem of Object.values(modules)) {
    if (registering(stem)) {
      stem.register(app);
    } else if (typeof stem === "object" && stem !== null) {
      for (const part of Object.values(stem)) {
        if (registering(part)) {
          part.register(app);
        }
      }
    }
  }
}
```

So a slice wires itself into the application by exporting `register` from its own `index.ts` and its line in the entry, and never edits `src/start/` or another stem's directory to be reached; the `start` slice writes no module's files to reach it. What `main()` attaches itself — body parsing before `wire(app)`, the error handlers after it — is the bootstrap's own, in its own directory.

## The tests

`test/<stem>.test.ts` over Node's own runner, or `test/<stem>/<part>.test.ts` for a part:

```ts
import { test } from "node:test";
import assert from "node:assert/strict";

import { createOrder } from "../src/orders/index.ts";

test("an order with no items is refused", () => {
  assert.throws(() => createOrder("o-1", []), RangeError);
});
```

One `test` per acceptance scenario, named by the scenario, its `WHEN` the arrangement and its `THEN` the assertion. A test reaches the module through its `index.ts`, as any other module would.

A test drives an HTTP application through a server listening on port `0` and `fetch` against the port it took, closed when the test ends, never a hand-built socket or a call into the framework's request handling. A service the tree does not run — a database, a broker, a cache — is reached through a fake in place of its client, handed to the module in the test's arrangement, never a live connection: a client that retries a connection forever is a test that never ends.

## The manifests

The three root manifests are written byte for byte as below by any slice that finds them missing, so two slices laying them at once lay the same file, and no slice edits one after: the root `package.json` names the toolchain and no package of the program's, and nothing is ever added to it, not a dependency and not a script. Its `workspaces` makes each module's directory, and each part's, a package of its own, so the packages a directory's files import are named in that directory's own `package.json`, a file only its slice writes. `npm install` at the root installs every such manifest's packages into the root `node_modules`, where every file of the tree resolves them, and two slices never write one manifest.

`package.json`:

```json
{
  "name": "app",
  "private": true,
  "type": "module",
  "workspaces": ["src/*", "src/*/*"],
  "scripts": {
    "check": "tsc --noEmit",
    "test": "node --test --test-force-exit --test-timeout=5000 'test/**/*.test.ts'",
    "start": "node src/main.ts"
  },
  "devDependencies": {
    "@types/node": "^24",
    "typescript": "^5.8"
  }
}
```

`src/<stem>/package.json`, or `src/<stem>/<part>/package.json`, written by the directory's slice with the packages its files import — the `@types/*` package of one that ships no types beside it — and nothing when the files import Node's own modules and the tree's own modules alone:

```json
{
  "name": "@app/orders",
  "private": true,
  "type": "module",
  "dependencies": {
    "express": "^5",
    "@types/express": "^5"
  }
}
```

A part's `name` joins the stem and the part: `@app/orders-reading`.

`tsconfig.json`:

```json
{
  "compilerOptions": {
    "strict": true,
    "target": "ES2022",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "allowImportingTsExtensions": true,
    "verbatimModuleSyntax": true,
    "erasableSyntaxOnly": true,
    "noEmit": true,
    "types": ["node"],
    "skipLibCheck": true
  },
  "include": ["src", "test"]
}
```

`.gitignore`:

```
node_modules/
dist/
.emery/
```

`erasableSyntaxOnly` holds the code to what Node runs without a compile step: no `enum`, no `namespace`, no parameter properties on a constructor. `noEmit` with `allowImportingTsExtensions` lets every import name its `.ts` file. `tsc --noEmit` passing means `node` runs the tree as it stands. The test script's `--test-force-exit` ends the run when the tests do, whatever a test left open — a client retrying a connection, a server listening — and `--test-timeout` fails a test after five seconds rather than waiting on it; the tests are run through the script alone, never `node --test` by hand.

## Merging

The engine merges each slice's commit into the integrated tree under these rules, in this order:

| Path | Rule | Why |
| --- | --- | --- |
| `package-lock.json` | the integrated side kept | the next `npm install` regenerates it |
| `src/index.ts`, every `index.ts` | both sides' lines kept | each is a list of independent lines |
| everything else | a conflict | the slice is rebuilt over the merged tree |

A path one slice alone changed takes that slice's change under no rule, so two slices that keep to their own directories never conflict. The root `package.json` is everything else on purpose: it is static, every slice that lays it lays the same bytes, and a directory's packages are in a manifest of its own, so no manifest is ever written by two slices. A slice that writes under another's directory, or into `src/start/` to be reached, is the one conflict the layout leaves possible, and it is the slice's to avoid.
