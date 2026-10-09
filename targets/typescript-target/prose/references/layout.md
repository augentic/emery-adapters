# Layout

The one shape every slice of a TypeScript build follows, so that slices built beside one another in a wave merge clean and the integrated tree compiles as a whole.

## The tree

```
.gitignore
package.json
package-lock.json        written by npm install, never by hand
tsconfig.json
src/
  index.ts               the entry: one line per module
  <stem>/
    index.ts             the module: what the rest of the tree may import
    <more>.ts            the module's own files, re-exported from its index
test/
  <stem>.test.ts         one test per acceptance scenario of the stem
```

- `<stem>` is the first segment of a requirement's dotted id: `orders` for `orders.create`, `start` for `start.listen`. A stem is one slice's and one directory's; two slices never write under one stem.
- A module's directory holds everything its requirements need. A file beside the `index.ts` is private to the module until the `index.ts` re-exports it.
- Relative imports name the file with its extension: `import { createOrder } from "./create.ts";` within a module, `import type { Order } from "../orders/index.ts";` across modules. Node runs the `.ts` files as they are, so a type-only import is spelled `import type`.
- A module imports another module's `index.ts` alone, never a file beneath it.

## The entry

`src/index.ts` is a list, one line per module, in any order:

```ts
export * as orders from "./orders/index.ts";
export * as payments from "./payments/index.ts";
```

A slice adds its own line and changes no other. Two slices adding a line each to the same entry at once merge clean, since the engine keeps both sides' lines of this file; a line that does anything but re-export one module breaks that.

The camel-cased stem is the namespace: `orders`, `paymentMethods` for `payment-methods`.

## The bootstrap

Where the plan has a `start` stem, its module is the program's entry point: `src/start/index.ts` exports a `main()` that wires the modules the design names and runs, and the `start` slice adds `"start": "node src/start/index.ts"` to the manifest's `scripts`. A tree with no `start` stem is a library; its entry is `src/index.ts` and nothing runs it.

## The tests

`test/<stem>.test.ts` over Node's own runner:

```ts
import { test } from "node:test";
import assert from "node:assert/strict";

import { createOrder } from "../src/orders/index.ts";

test("an order with no items is refused", () => {
  assert.throws(() => createOrder("o-1", []), RangeError);
});
```

One `test` per acceptance scenario, named by the scenario, its `WHEN` the arrangement and its `THEN` the assertion. A test reaches the module through its `index.ts`, as any other module would.

## The manifests

Written byte for byte as below by the slice that owns the bootstrap, and by any slice that finds them missing, so two slices laying them at once lay the same file. Nothing else is added at first; a dependency joins `package.json` when a module cannot do without it.

`package.json`:

```json
{
  "name": "app",
  "private": true,
  "type": "module",
  "scripts": {
    "check": "tsc --noEmit",
    "test": "node --test 'test/**/*.test.ts'"
  },
  "devDependencies": {
    "@types/node": "^24",
    "typescript": "^5.8"
  }
}
```

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

`erasableSyntaxOnly` holds the code to what Node runs without a compile step: no `enum`, no `namespace`, no parameter properties on a constructor. `noEmit` with `allowImportingTsExtensions` lets every import name its `.ts` file. `tsc --noEmit` passing means `node` runs the tree as it stands.

## Merging

The engine merges each slice's commit into the integrated tree under these rules, in this order:

| Path | Rule | Why |
| --- | --- | --- |
| `package-lock.json` | the integrated side kept | the next `npm install` regenerates it |
| `src/index.ts`, every `index.ts` | both sides' lines kept | the entry is a list of independent lines |
| everything else | a conflict | the slice is rebuilt over the merged tree |

`package.json` is everything else on purpose: a dependency two slices each added differently is a decision, and the second slice makes it over the first's tree.
