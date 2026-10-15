# target.verify

Verify the integrated project tree after a wave of slices has merged into it, repair what the checks find, and answer a verdict.

## Inputs

- `$WORKSPACE` — the integrated tree, lent writable with the shell. Read it through the workspace tools, run the checks below in it, and repair what they find through this call's two tools: `edit_files` for a change within a file, each edit the one place its `find` matches and the text to put there, which is what a repair almost always is — a line added to a list, a type's shape put back to the design's; `write_files` for a file laid whole and a file removed through its `delete`. Then run the checks again. The shell runs the checks and nothing else: a file it writes is not a repair. What you write, and what a check leaves behind, is sealed as the wave's own commit by the engine, never by you.

Nothing outside `$WORKSPACE` is reachable.

## Checks

The tree is an omnia guest in the [layout](references/layout.md): the root package `guest` holding the entry in `src/lib.rs`, the route list in `src/routes.rs`, the topic list in `src/messaging.rs`, and the two rungs under `tests/`; `crates/common` holding the path table, the topic table, and the configuration catalog; one crate per stem beside it, each with its own manifest and tests; and the root `Cargo.toml` pinning every shared crate. Five checks are yours to run; the layout's own rules the adapter holds for you when you answer, as [the rules held](#the-rules-held) says, so do not read the tree for them. Mark the time (`touch "$TMPDIR/before"`), then run each check in `$WORKSPACE`, in this order, and read what it prints:

1. **It lints.** `cargo clippy --workspace --all-targets -- -D warnings` exits 0 over every crate and its tests, under the workspace's lints. The merge may have kept one slice's `Cargo.lock` over another's; the first cargo run resolves the workspace again and rewrites it. A statement naming a stem the root `Cargo.toml`'s `[dependencies]` lacks, a `pub mod` line naming no file, or a module using a crate its stem's manifest does not name fails here, and the repair is the missing line, at the end of its list.
2. **The tests pass.** `cargo test --workspace` exits 0, every test of every crate passing, and the two rungs with them: a panic in `tests/routes.rs` naming an overlapping route is two slices routing one path with one method, in `tests/messaging.rs` naming a duplicate topic two slices consuming one; a rung's `dispatch` or `routed` naming an entry is a table entry whose list statement is missing, or the reverse.
3. **The guest builds.** `cargo build --target wasm32-wasip2` exits 0 and `target/wasm32-wasip2/debug/guest.wasm` exists: the whole program compiles for the component, every crate a statement named included.
4. **It is formatted.** `cargo +nightly fmt --all -- --check` exits 0, where `rustup run nightly rustfmt --version` succeeds; where it does not, skip this check and say so in no failure. A file it names is reformatted as it reports, through `edit_files`, or `write_files` whole, a list file included: the formatter may wrap a list statement over lines and never changes what the list holds.
5. **The checks wrote nothing else.** `find . -type f -not -path './target/*' -newer "$TMPDIR/before"` names `Cargo.lock` and what you repaired at most. Any other path is something a check or a test wrote into the tree. No repository is reachable from `$WORKSPACE`, so `git` cannot answer this.

A check that fails ends the sequence where it is useful: a tree that does not compile cannot run its tests or build the guest, a lint failure alone leaves the tests, the guest build, and the formatter worth running, and the fifth runs whatever came before. A check that passed is run again only where a repair touched what it reads: the checks are slow over a whole tree, and the turn has one budget, so run them as few times as the repairs demand and answer once the last run is clean or its failure is yours to report.

## The rules held

When you answer, the adapter reads the tree against the layout's rules and refuses the verdict with every rule it breaks as a finding under `## Findings`, whatever the checks printed. Each finding names the file, the line, and the repair; make it through the two tools, run the checks the repair touches again, and answer again. The rules are:

- **The static files stand.** Every file of the seed outside the eleven lists the layout names — `src/lib.rs`, `examples/runtime.rs`, `crates/common/Cargo.toml`, `crates/common/src/lib.rs`, `crates/common/src/routes.rs`, the toolchain and tooling files, `supply-chain/`, and `.github/` whole — and the root `Cargo.toml` outside its two lists are held to the exemplar's guest template: the finding is the `-` lines of the template to put back and the `+` lines of the tree to drop, or the template's body whole for a file the tree lacks. Nothing is put back before this turn, so a drift a slice merged is yours to repair as the finding spells it, through `edit_files`, or `write_files` for a file laid whole, and so is one your own repair made. Every pin, the lints, and the component's entry are the template's, so a check that fails for a crate the pins lack is repaired in the stem's manifest, where the crate carries its version, never in the root's.
- **The tables are spelled into their lists.** Every constant of `crates/common/src/routes/http.rs` is named by one `.route(` statement of `src/routes.rs` and one entry of `ROUTES` in `tests/routes.rs`. A constant of `crates/common/src/routes/topic.rs` a statement of `src/messaging.rs` consumes is named by one entry of `TOPICS` in `tests/messaging.rs`; one no statement consumes is a topic a module publishes to, named by that module and by neither list, and is a finding only where no module names it or the rung lists it, since `routed` would then deliver to a topic nothing consumes. Every statement of a list names a constant of its table, never a literal. The repair adds the missing line at the end of its list, removes an entry nothing consumes or a constant nothing names, and rewrites no other line: the static text around a list stands as the seed spells it, and every line of a list is one entry, once.
- **No module is the process.** Under `crates/start/src`, a function that starts, stops, runs, listens, serves, bootstraps, or shuts down, or a `Main`, `App`, `Server`, or `Application` type is the process the source was, which the host is and the tree never simulates, as the [guardrails](references/guardrails.md) spell out under what `start` becomes. The repair is not a rename: the module is removed through `delete`, with its `pub mod` line and its test, and the requirements it stood for are left to the report, since what it did the host does or nothing does. A health route a requirement states and a `pub fn` every handler calls stay. Where a removal leaves a crate with no module, the crate stays with its empty list.
- **No module simulates the host.** Under every stem's `src/`, a task, a timer, a thread, a reach past the component to the environment, the file system, a socket, or a process, a `loop {` that polls or retries, a step-done key, a stop request, or an exit delay kept in the state store, or a crate-level `allow` is a mechanism the [translation](references/translation.md) lays nothing for. The repair removes the spelling from its module through `edit_files` — a loop's actions run once, back to back, a failure carried as the classified error; a flag's step read at the moment of use — and lays what the module did through it as the translation says, or leaves it out.
- **Every handler is reached.** Every `pub async fn` under `crates/*/src` whose signature takes a `Context` is handed to its route function or consumer by a `.route(` statement of `src/routes.rs` or `src/messaging.rs`, or to `handle_with` by a route function of its stem a statement calls. A handler no statement reaches is code the host never runs, however its tests pass: the repair adds the statement under the route or topic its requirement states, with the constant and the rung entry beside it, or, where the function is a helper and no handler, hands it the provider rather than the `Context`.

## Repairs

A failure in the merged tree is the seam between two slices, and the repair is the smallest change that closes it: two slices typing one design type differently, one depending on a crate the other did not lay, a list missing a line, a manifest missing a crate a file uses, two slices routing one path with one method or consuming one topic, a constant declared twice under two names, a test asserting what another slice's module no longer does. Repair it where the layout puts it — in the slice's own module, its lines, its stem's manifest, its test — and change nothing a passing check covers. A static file is never a repair's to change: one that differs from the template is a finding on your verdict, put back as the finding spells it; a manifest naming a source is put back to the pin; a list is repaired by adding the line it lacks, never by rewriting one it holds. Then run the checks again from the one that failed.

Leave what a repair would have to invent: a requirement no module implements, a test for a scenario the tree does not hold, a module the plan has a later wave land. A failure that remains after the repairs you can justify is reported, not patched over.

## Verdict

Answer with one JSON object:

- `passed` — true when every check passed on its last run, false otherwise.
- `failures` — one entry per check that still failed, naming the check and quoting the tail of what it printed, the first error lines of `cargo` or the failing test names; empty when `passed` is true.

The verdict is held to the tree before it is accepted, as [the rules held](#the-rules-held) says: a finding under `## Findings` names a rule the tree breaks with what to put back or remove. Repair it through the two tools, run the checks the repair touches again, and answer again; the verdict is yours once the tree stands, passing or failing.

## Worked example

A wave whose `payments` slice built against the design's `Money` type while the `orders` slice, merged beside it, declared the type with another shape; `orders`' declaration was repaired to the design's and clippy run again clean, and the tests then found a transition `payments` does not implement, left for the report:

```json
{
  "passed": false,
  "failures": [
    "check 2 (cargo test --workspace): crates/payments/tests/payments.rs — `a_cancelled_order_cannot_be_paid` fails: expected `bad_request`, got Ok"
  ]
}
```

Report what the checks found on their last run, never what the tree should hold.
