# target.build

Build one slice of the plan into the lent project tree as strict TypeScript on Node, check it compiles and its tests pass, and report what you covered and wrote.

## Inputs

- `$WORKSPACE` — the project tree, lent writable, sitting on the commit the brief names. Read it through the workspace tools; write it through this call's `write_files` tool, one or more files per call, each created or replaced whole, at a `/`-separated path relative to `$WORKSPACE`. Lay the module, its entry line, its test, and any manifest in one call; a fix the loop below asks for is a later call rewriting the file whole. Run the checks below through the shell, in `$WORKSPACE`. Nothing outside `$WORKSPACE` is reachable.
- **The slice's plan entry** — its id, name, requirements, the design types it owns, and the slices it depends on.
- **The specification, cut to the slice** — the requirements to implement, each with its acceptance scenarios.
- **The design, whole** — the types and sections every slice shares.

Build this slice completely in one pass. Other slices of the wave are building beside you from the same commit; what you write merges with theirs under the rules the [layout](references/layout.md) states, so keep to its shape.

## What to write

The [layout](references/layout.md) is the one shape every slice follows. In brief:

- **One module per slice**, `src/<stem>/index.ts`, where `<stem>` is the first segment of your requirement ids (`orders` for `orders.create`). A slice holding several stems writes one module per stem. Everything your requirements need lives in that directory; a second file goes beside the `index.ts`, which re-exports what the rest of the tree may use.
- **One line in the entry**, `src/index.ts`: `export * as <camelStem> from "./<stem>/index.ts";`. Add your line and change no other. Where the file is missing, write it holding your line alone.
- **One test file per stem**, `test/<stem>.test.ts`, over `node:test` and `node:assert/strict`: one `test` per acceptance scenario, named by the scenario, asserting its `THEN` through your module's exports. A scenario the specification gives you is a test; invent none.
- **The manifests**, `package.json`, `tsconfig.json`, and `.gitignore`, exactly as the layout spells them, when `$WORKSPACE` holds none. The slice that owns the bootstrap — the `start` stem, else `SLICE-001` — owns them; a slice building beside it that finds them missing writes the same bytes, so the two merge clean. Edit `package.json` after that only to add a dependency your module cannot do without, and prefer Node's own modules (`node:http`, `node:fs`, `node:crypto`) over one.
- **Another slice's module is not yours to change.** Import it where the design has yours depend on it, at the path the layout gives; where it is not in the tree yet, implement against the design's types and leave the import for the slice that lands it.

Read `src/` before writing: a slice built before this one, or a resumed build, may already hold a module or the entry.

## The loop

Once the tree holds your files, run each check in `$WORKSPACE` and read what it prints, in this order:

1. `npm install` — once the manifests are in place; it writes `node_modules/`, which `.gitignore` keeps out of the tree, and `package-lock.json`, which stays.
2. `npx tsc --noEmit` — the whole tree under `strict`; a failure in a module not yours is a type the design names and your module misuses, or a module that is not there yet, which you work around in yours.
3. `npm test` — every test in `test/`; your tests fail only for what your module does not yet do.

Fix what a check reports and run it again until all three pass. Report only once they pass, or once a check fails for a reason outside your slice, which the report then leaves uncovered.

## Report

Answer with one JSON object:

- `covered` — each requirement id the specification above holds that your module now implements and its test confirms, once each. Leave an id out rather than claim what the tree does not hold or a test does not pass.
- `written` — each file `write_files` wrote, once each, as a `/`-separated path relative to `$WORKSPACE`, and no file the tree does not hold. A file a check wrote, `package-lock.json`, is the tree's whether or not you list it.

## Worked example

A slice `orders` (`SLICE-002`) implementing `REQ-003` and `REQ-004` over a tree the bootstrap slice has already laid the manifests in; `REQ-004` depends on a `payments` module another slice is building this wave, so its test cannot pass yet and it is left uncovered:

```json
{
  "covered": ["REQ-003"],
  "written": ["src/orders/index.ts", "src/index.ts", "test/orders.test.ts"]
}
```
