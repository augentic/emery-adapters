# target.verify

Verify the integrated project tree after a wave of slices has merged into it, and answer a verdict.

## Inputs

- `$WORKSPACE` — the integrated tree, lent with the shell. Read it through the workspace tools and run the checks below in it. Write nothing you mean to keep: the tree is the build's, and what a check leaves behind is sealed or discarded by the engine, never by you.

Nothing outside `$WORKSPACE` is reachable.

## Checks

The tree is strict TypeScript on Node in the [layout](references/layout.md): one module per slice under `src/`, the entry `src/index.ts`, the tests under `test/`, and the three manifests at the root. Run each check in `$WORKSPACE`, in this order, and read what it prints:

1. **It installs.** `npm install` exits 0. The merge may have kept one slice's `package-lock.json` over another's; this regenerates it.
2. **It compiles.** `npx tsc --noEmit` exits 0 over the whole tree: every module, the entry, and the tests, under `strict`.
3. **The tests pass.** `npm test` exits 0, every test in `test/` passing.
4. **The checks wrote nothing else.** `git status --porcelain` in `$WORKSPACE` names `package-lock.json` at most. Any other path is something a check or a test wrote into the tree.

A check that fails ends the sequence where it is useful: a tree that does not install cannot compile, one that does not compile cannot run its tests, and the fourth runs whatever came before.

## Verdict

Answer with one JSON object:

- `passed` — true when every check passed, false otherwise.
- `failures` — one entry per check that failed, naming the check and quoting the tail of what it printed, the first error lines of `tsc` or the failing test names; empty when `passed` is true.

## Worked example

A wave whose `payments` slice built against the design's `Money` type while the `orders` slice, merged beside it, declared the type with another shape:

```json
{
  "passed": false,
  "failures": [
    "check 2 (tsc): src/payments/index.ts(14,22): error TS2345: Argument of type 'string' is not assignable to parameter of type 'Money'.",
    "check 3 (npm test): not run; the tree does not compile"
  ]
}
```

Report what the checks found, never what the tree should hold.
