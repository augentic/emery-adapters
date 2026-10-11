# target.verify

Verify the integrated project tree after a wave of slices has merged into it, repair what the checks find, and answer a verdict.

## Inputs

- `$WORKSPACE` — the integrated tree, lent writable with the shell. Read it through the workspace tools, run the checks below in it, and repair what they find through this call's `write_files` tool, each file rewritten whole and a file removed through `delete`; then run the checks again. The shell runs the checks and nothing else: a file it writes is not a repair. What you write, and what a check leaves behind, is sealed as the wave's own commit by the engine, never by you.

Nothing outside `$WORKSPACE` is reachable.

## Checks

The tree is strict TypeScript on Node in the [layout](references/layout.md): one directory per slice under `src/`, each with its own manifest where it imports a package, the entry `src/index.ts`, the tests under `test/`, and the three manifests at the root. Run each check in `$WORKSPACE`, in this order, and read what it prints:

1. **It installs.** `npm install` exits 0. The merge may have kept one slice's `package-lock.json` over another's; this regenerates it.
2. **It compiles.** `npx tsc --noEmit` exits 0 over the whole tree: every module, the entry, and the tests, under `strict`.
3. **The tests pass.** `npm test` exits 0, every test in `test/` passing. Run them through the script alone, never `node --test` by hand: its flags end the run when the tests do.
4. **The checks wrote nothing else.** `git status --porcelain` in `$WORKSPACE` names `package-lock.json` and what you repaired at most. Any other path is something a check or a test wrote into the tree.

A check that fails ends the sequence where it is useful: a tree that does not install cannot compile, one that does not compile cannot run its tests, and the fourth runs whatever came before.

## Repairs

A failure in the merged tree is the seam between two slices, and the repair is the smallest change that closes it: two slices typing one design type differently, one importing a module at a path the other did not lay, a list missing a line, a manifest missing a package a file imports, a test asserting what another slice's module no longer does. Repair it where the layout puts it — in the slice's own directory, its lines, its manifest, its test — and change nothing a passing check covers. Then run the checks again from the one that failed.

Leave what a repair would have to invent: a requirement no module implements, a test for a scenario the tree does not hold, a module the plan has a later wave land. A failure that remains after the repairs you can justify is reported, not patched over.

## Verdict

Answer with one JSON object:

- `passed` — true when every check passed on its last run, false otherwise.
- `failures` — one entry per check that still failed, naming the check and quoting the tail of what it printed, the first error lines of `tsc` or the failing test names; empty when `passed` is true.

## Worked example

A wave whose `payments` slice built against the design's `Money` type while the `orders` slice, merged beside it, declared the type with another shape; `orders`' declaration was repaired to the design's and `tsc` run again clean, and the tests then found a transition `payments` does not implement, left for the report:

```json
{
  "passed": false,
  "failures": [
    "check 3 (npm test): test/payments.test.ts — 'a cancelled order cannot be paid' fails: expected 409, got 200"
  ]
}
```

Report what the checks found on their last run, never what the tree should hold.
