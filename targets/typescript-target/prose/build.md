# target.build

Build one slice of the plan into the lent project tree as strict TypeScript on Node, check it compiles and its tests pass, and report what you covered and wrote.

## Inputs

- `$WORKSPACE` — the project tree, lent writable, sitting on the commit the brief names. Read it through the workspace tools; write it through this call's `write_files` tool alone, one or more files per call, each created or replaced whole, at a `/`-separated path relative to `$WORKSPACE`, and remove a file through the same call's `delete`. Lay the module, its lines, its test, and any manifest in one call; a fix the loop below asks for is a later call rewriting the file whole. The shell runs the checks below in `$WORKSPACE` and nothing else: a file the shell writes, through a redirect, a heredoc, `cp`, or a script, is outside the tool and is not yours. Nothing outside `$WORKSPACE` is reachable.
- **The slice's plan entry** — its id, name, requirements, the design types it owns, and the slices it depends on.
- **The specification, cut to the slice** — the requirements to implement, each with its acceptance scenarios.
- **The design, whole** — the types and sections every slice shares.

Build this slice completely in one pass. Other slices of the wave are building beside you from the same commit; what you write merges with theirs under the rules the [layout](references/layout.md) states, so keep to its shape.

## What to write

The [layout](references/layout.md) is the one shape every slice follows. In brief:

- **Your directory.** `<stem>` is the first segment of your requirement ids (`orders` for `orders.create`). A slice named by its stem builds it whole in `src/<stem>/`; any other slice builds a part of each stem its requirements lead with, in `src/<stem>/<part>/`, `<part>` your slice's name less the stem and its hyphen where the name leads with them (`reading` for `orders-reading`). Everything your requirements need lives in that directory, its `index.ts` re-exporting what the rest of the tree may use.
- **Your lines.** One line in the entry, `src/index.ts`: `export * as <camelStem> from "./<stem>/index.ts";`. A part adds one more in the stem's list, `src/<stem>/index.ts`: `export * as <camelPart> from "./<part>/index.ts";`. Add your lines and change no other; where a list is missing, write it holding your line alone.
- **Your wiring.** A module the application reaches exports `register(app)` from its `index.ts`, taking the framework's own instance type the design names, and attaches its routes, middleware, or handlers to it; the bootstrap calls every `register` the entry reaches. Never edit `src/start/`, or another stem's directory, to be reached.
- **Your test file**, `test/<stem>.test.ts`, or `test/<stem>/<part>.test.ts` for a part, over `node:test` and `node:assert/strict`: one `test` per acceptance scenario, named by the scenario, asserting its `THEN` through your module's exports. A scenario the specification gives you is a test; invent none. An HTTP scenario is driven through a server on port `0` and `fetch`; a service the tree does not run is a fake handed in the arrangement.
- **The root manifests**, `package.json`, `tsconfig.json`, and `.gitignore`, exactly as the layout spells them, when `$WORKSPACE` holds none; a slice that finds them in the tree leaves them as they are. The root `package.json` is static and names no package of the program's: nothing is added to it, not a dependency and not a script, by any slice.
- **Your manifest**, `package.json` in your directory, as the layout spells it, naming every package your files import and its `@types/*` package where one ships no types; none where your files import Node's own modules and the tree's own alone. The root manifest's `workspaces` installs it from the root, so `npm install` in `$WORKSPACE` resolves your packages for every file of the tree.
- **The bootstrap**, where your slice holds `main()` under the `start` stem: `main()` in your directory, `src/main.ts` as the layout spells it, and, for a part, the `export { main }` line in the stem's list.
- **Another slice's module is not yours to change.** Import it where the design has yours depend on it, at the path the layout gives; where it is not in the tree yet, implement against the design's types and leave the import for the slice that lands it.

Read `src/` before writing: a slice built before this one, or a resumed build, may already hold a module or a list.

## The loop

Once the tree holds your files, run each check in `$WORKSPACE` and read what it prints, in this order:

1. `npm install` — once the manifests are in place; it writes `node_modules/`, which `.gitignore` keeps out of the tree, and `package-lock.json`, which stays.
2. `npx tsc --noEmit` — the whole tree under `strict`; a failure in a module not yours is a type the design names and your module misuses, or a module that is not there yet, which you work around in yours.
3. `npm test` — every test in `test/`; your tests fail only for what your module does not yet do. Run the tests through this script alone, never `node --test` by hand: its flags end the run when the tests do.

Fix what a check reports, through `write_files`, and run it again until all three pass. Report only once they pass, or once a check fails for a reason outside your slice, which the report then leaves uncovered.

## Report

Answer with one JSON object:

- `covered` — each requirement id the specification above holds that your module now implements and its test confirms, once each. Leave an id out rather than claim what the tree does not hold or a test does not pass.
- `written` — the files `write_files` wrote, once each, as a `/`-separated path relative to `$WORKSPACE`. A file a check wrote, `package-lock.json`, is the tree's whether or not you list it.

## Worked example

A slice `orders-reading` (`SLICE-006`) implementing `REQ-030` and `REQ-031`, a part of the `orders` stem another slice also builds this wave, over a tree an earlier wave has already laid the root manifests in, its module importing `express`; `REQ-031` depends on a `customers` module another slice is building this wave, so its test cannot pass yet and it is left uncovered:

```json
{
  "covered": ["REQ-030"],
  "written": [
    "src/orders/reading/package.json",
    "src/orders/reading/index.ts",
    "src/orders/index.ts",
    "src/index.ts",
    "test/orders/reading.test.ts"
  ]
}
```
