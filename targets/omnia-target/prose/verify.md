# target.verify

Verify the integrated project tree after a wave of slices has merged into it, repair what the checks find, and answer a verdict.

## Inputs

- `$WORKSPACE` — the integrated tree, lent writable with the shell. Read it through the workspace tools, run the checks below in it, and repair what they find through this call's `write_files` tool, each file rewritten whole and a file removed through `delete`; then run the checks again. The shell runs the checks and nothing else: a file it writes is not a repair. What you write, and what a check leaves behind, is sealed as the wave's own commit by the engine, never by you.

Nothing outside `$WORKSPACE` is reachable.

## Checks

The tree is an omnia HTTP guest in the [layout](references/layout.md): one crate per stem under `crates/`, each with its own manifest and tests, the root crate `app` with its router list in `src/lib.rs`, its smoke test in `tests/routes.rs`, and its manifest pinning every shared crate. Run each check in `$WORKSPACE`, in this order, and read what it prints:

1. **It lints.** `cargo clippy --workspace --all-targets -- -D warnings` exits 0 over every crate and its tests. The merge may have kept one slice's `Cargo.lock` over another's; the first cargo run resolves the workspace again and rewrites it.
2. **The tests pass.** `cargo test --workspace` exits 0, every test of every crate passing, and `tests/routes.rs` with them: a panic there naming an overlapping route is two modules routing one path with one method.
3. **The guest builds.** `cargo build --target wasm32-wasip2` exits 0 and `target/wasm32-wasip2/debug/app.wasm` exists: the whole program compiles for the component, every crate a module named included.
4. **The checks wrote nothing else.** `git status --porcelain` in `$WORKSPACE` names `Cargo.lock` and what you repaired at most. Any other path is something a check or a test wrote into the tree.

A check that fails ends the sequence where it is useful: a tree that does not compile cannot run its tests or build the guest, a lint failure alone leaves the tests and the guest build worth running, and the fourth runs whatever came before.

## Repairs

A failure in the merged tree is the seam between two slices, and the repair is the smallest change that closes it: two slices typing one design type differently, one depending on a crate at a path the other did not lay, a list missing a line, a manifest missing a crate a file uses, two modules routing one path with one method, a test asserting what another slice's module no longer does. Repair it where the layout puts it — in the slice's own module, its lines, its stem's manifest, its test — and change nothing a passing check covers. Then run the checks again from the one that failed.

Leave what a repair would have to invent: a requirement no module implements, a test for a scenario the tree does not hold, a module the plan has a later wave land. A failure that remains after the repairs you can justify is reported, not patched over.

## Verdict

Answer with one JSON object:

- `passed` — true when every check passed on its last run, false otherwise.
- `failures` — one entry per check that still failed, naming the check and quoting the tail of what it printed, the first error lines of `cargo` or the failing test names; empty when `passed` is true.

## Worked example

A wave whose `payments` slice built against the design's `Money` type while the `orders` slice, merged beside it, declared the type with another shape; `orders`' declaration was repaired to the design's and clippy run again clean, and the tests then found a transition `payments` does not implement, left for the report:

```json
{
  "passed": false,
  "failures": [
    "check 2 (cargo test --workspace): crates/payments/tests/payments.rs — `a_cancelled_order_cannot_be_paid` fails: expected 409, got 200"
  ]
}
```

Report what the checks found on their last run, never what the tree should hold.
