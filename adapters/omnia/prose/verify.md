# target.verify

Verify the integrated project tree after a wave of slices has merged into it.
Run the checks. Repair what they find. Run them again. Answer with one JSON object, the verdict.

## Inputs

- `$WORKSPACE` — the integrated tree, lent writable with the shell. Read it through the workspace tools. Run the checks in it.
- The two tools. `edit_files` changes within a file: each edit's `find` must match one place, and `replace` is the text to put there.
  A repair is almost always an `edit_files` edit: a line added to a list, a type's shape put back to the design's.
  `write_files` writes a file whole, and removes a file through its `delete`.
- A refused `find` names the line of the file nearest its first line. Copy that line's text into a new `find` and try once more.

The shell runs the checks and nothing else. A file it writes is not a repair.
What you write, and what a check leaves behind, is sealed as the wave's own commit by the engine, never by you.
Nothing outside `$WORKSPACE` is reachable.

## The tree

The tree is an omnia guest in the [layout](references/layout.md):

- the root package `guest`: the entry in `src/lib.rs`, the route list in `src/routes.rs`, the topic list in `src/messaging.rs`, and the two rungs under `tests/`;
- `crates/common`: the capability bound, the two folds a stem's entry composes its parts with, the topic table, and the environment helpers;
- one crate per stem beside it, its entry composing its parts' routes and topics, each with its own manifest and tests;
- the root `Cargo.toml`, pinning every shared crate.

Five checks are yours to run. The layout's own rules the adapter holds for you when you answer, as [the rules held](#the-rules-held) says.
Do not read the tree for those rules.

## Checks

Mark the time first: `touch "$TMPDIR/before"`. Then run each check in `$WORKSPACE`, in this order, and read what it prints.

1. **It lints.** `cargo clippy --workspace --all-targets -- -D warnings` exits 0.
   Every crate and its tests, under the workspace's lints, the host boundary among them.
   A `disallowed` method, type, or macro names the capability to reach instead. The repair is that capability, never an `allow` or an `expect`.
   The merge may have kept one slice's `Cargo.lock` over another's. The first cargo run resolves the workspace again and rewrites it.
   What fails here, and its repair:
   - a statement naming a stem the root `Cargo.toml`'s `[dependencies]` lacks: add the stem's line at the end of the list;
   - a `pub mod` line naming no file: remove the line;
   - an entry of a stem's routes or topics array naming a part with no such function: remove the entry;
   - a module using a crate its stem's manifest does not name: add the line at the end of the stem's `[dependencies]`;
   - a part no entry lists, or a handler no `routes()` names, is dead code: add the missing entry or `.route(..)`.
2. **The tests pass.** `cargo test --workspace` exits 0, every test of every crate, and the two rungs with them.
   A panic in `tests/routes.rs` naming an overlapping route is two slices routing one path with one method.
   A panic in `tests/messaging.rs` naming a duplicate topic is two slices consuming one topic.
   The owner is whichever slice's requirement states it.
3. **The guest builds.** `cargo build --target wasm32-wasip2` exits 0 and `target/wasm32-wasip2/debug/guest.wasm` exists.
   The whole program compiles for the component, every crate a statement named included.
4. **It is formatted.** `cargo +nightly fmt --all -- --check` exits 0, where `rustup run nightly rustfmt --version` succeeds.
   Where it does not succeed, skip this check and name no failure for it.
   A file it names is reformatted as it reports, through `edit_files`, or `write_files` whole, a list file included.
   The formatter may wrap a list statement over lines. It never changes what the list holds.
5. **The checks wrote nothing else.** `find . -type f -not -path '*/target/*' -newer "$TMPDIR/before"` names `Cargo.lock` and what you repaired, at most.
   The pattern holds under any root, so the same command over `"$WORKSPACE"` excludes `target/` too.
   Any other path is something a check or a test wrote into the tree. Remove it through `delete`.
   No repository is reachable from `$WORKSPACE`, so `git` cannot answer this.

A check that fails ends the sequence where it is useful.
A tree that does not compile cannot run its tests or build the guest.
A lint failure alone leaves the tests, the guest build, and the formatter worth running. The fifth runs whatever came before.
A check that passed is run again only where a repair touched what it reads. The checks are slow, and the turn has one budget.
Run them as few times as the repairs demand. Answer once the last run is clean or its failure is yours to report.
Where every check passes at its first run and the fifth names `Cargo.lock` at most, there is nothing to repair: answer the passing verdict in your next reply, with no further command.
A name a reference's example uses, `ORDER_PLACED`, `place_order`, is the example's, not the tree's: the tree is not searched for it, and its absence is no failure.

## The rules held

When you answer, the adapter reads the tree against the layout's rules.
It refuses the verdict with every rule the tree breaks as a finding under `## Findings`, whatever the checks printed.
Each finding names the file, the line, and the repair. Make the repair through the two tools. Run the checks the repair touches again. Answer again.
The rules are:

- **The static files stand.** Every file of the seed outside the four root lists is held to the exemplar's guest template.
  Those are `src/lib.rs`, `tests/routes.rs`, `tests/messaging.rs`, `examples/runtime.rs`, `crates/common/Cargo.toml`, `crates/common/src/lib.rs`, `crates/common/src/config.rs`, `clippy.toml`,
  the toolchain and tooling files, `supply-chain/`, and `.github/` whole. The root `Cargo.toml` outside its two lists is held the same way.
  The finding is the `-` lines of the template to put back and the `+` lines of the tree to drop, or the template's body whole for a file the tree lacks.
  Nothing is put back before this turn. A drift a slice merged is yours to repair as the finding spells it, through `edit_files`, or `write_files` for a file written whole.
  So is a drift your own repair made.
  Every pin, the lints, and the component's entry are the template's.
  A check that fails for a crate the pins lack is repaired in the stem's manifest, where the crate carries its version, never in the root's.
  An entry of the root manifest's path list naming a crate the tree lacks, `start = { path = "crates/start" }` with no `crates/start/Cargo.toml`, is a finding at its line.
  The repair drops it and the stem's `<stem>.workspace = true` line through `edit_files`.
- **Every stem is merged.** Every stem's `router()` is merged by a `router.merge(<stem>::router())` statement of `src/routes.rs`.
  Every stem's `topics(..)` is registered by a `<stem>::topics(router, &env)` statement of `src/messaging.rs`.
  A stem the lists do not name compiles clean and serves nothing, which no check sees.
  The repair adds the stem's line at the end of its list through `edit_files` and rewrites no other.
  The static text around a list stands as the seed spells it, and every line of a list is one entry, once.
- **Every part is declared, and the entry holds nothing of its own.** Every `crates/<stem>/src/<part>.rs` is listed by a `pub mod <part>;` line of the stem's `src/lib.rs`.
  A module nothing declares is compiled by nothing and serves nothing, which the checks pass over.
  An entry that exposes `router()` or `topics(..)` holds its `pub mod` lines, its `use` lines, and the two folds, and no handler, type, constant, or inline module.
  One standing there is a part folded into the entry where its file was, as a `mv` through the shell leaves the tree.
  The repair adds the missing line among the entry's `pub mod` lines through `edit_files`.
  A folded item goes back into its part, `crates/<stem>/src/<part>.rs`, declared by its line and named in the entry's arrays, through `write_files` and `edit_files`; no file is moved through the shell.
- **Every topic is named.** Every constant of `crates/common/src/topic.rs` is named by a module, as `topic::<NAME>` through `config::topic` or `config::topic_for`.
  One nothing names is a topic nothing publishes or consumes.
  The repair names it from the part whose requirement publishes or consumes the topic, or removes the constant with its doc comment through `edit_files`.
- **No `crates/start/` crate is written.** The seed and the host are `start`, as the [guardrails](references/guardrails.md) spell out under what `start` becomes.
  The repair removes every file under `crates/start/` through `delete` and drops any `start` line from the root `Cargo.toml` through `edit_files`.
- **No module is the process.** Under `crates/*/src`, a function that starts, stops, runs, listens, serves, bootstraps, or shuts down is the process the source was.
  So is a `Main`, `App`, `Server`, or `Application` type. The host is the process, and the tree never simulates it.
  The repair is not a rename. The module is removed through `delete`, with its `pub mod` line and its test.
  The requirements it stood for are left to the report. What it did, the host does or nothing does.
  Where a removal leaves a crate with no module, the crate stays with its empty list.
- **No miss is answered as nothing.** A read of the store under a constant key, `StateStore::get(provider, KEY)`, must not answer its miss as an empty collection or a default.
  A function that returns an empty collection on `Ok(None)`, by a `return` or as its tail, or defaults the miss, has lost the table.
  A default on the hit's own arm decodes what the store held and is none. A read under a key built from the input is a lookup and passes.
  The repair writes the fetch into the reading function through `edit_files`.
  On `Ok(None)` it fetches the table from its upstream as the design spells the call, stores it under the same key with a TTL, and answers what it fetched.
- **No struct wraps an HTTP response in a single `data` field.** Under `crates/*/src`, a `Deserialize` struct whose body holds a single `data` field wraps the response in the source's client property.
  The wire never carries that. The repair removes the wrapper struct through `edit_files`.
  Deserialize `response.body()` directly into `Vec<T>` for a JSON array, or into a struct with the body's fields at its root.
- **Every stem inherits the lints.** Every `crates/*/Cargo.toml` carries `[lints]` with `workspace = true`.
  The workspace's lints are the host boundary and reach a crate through that table alone.
  The repair adds the table through `edit_files` and runs the first check again.
- **Every dependency is used.** Every crate a stem's `[dependencies]` names beyond the layout's six fixed lines is spelled by a module or a test of the stem.
  One no file spells is compiled for nothing, and a slice that named it meant to use it.
  The repair removes the line through `edit_files`, or writes the use the slice's requirement states, and runs the first two checks again.
- **Every stem that publishes is reached.** A part spelling `Publish` in a stem whose entry registers no part's `routes()` or `topics`, and whose crate no other stem's module names, is never run.
  The host runs a stem through its two folds alone.
  The repair is the trigger the design's `## APIs and integrations` spells for the stem's inbound stream, through `edit_files`, then the first two checks again:
  - the constant at the end of the topic table, its value the topic less the environment prefix;
  - the `.route(config::topic_for(env, topic::<NAME>), messaging::consume_with(<handler>, <decode>))` in the part's `topics(..)`, the handler calling the stem's earlier parts;
  - the part's entry at the end of the stem's array.
- **No refusal is answered `Ok(())`.** Under `crates/*/src`, a `return Ok(())` or a tail `Ok(())` inside a catch answers a delivery or a call the code refused as processed.
  A catch is a `let Ok(..) = .. else` block, an `Err(..) =>` arm, an `if ..is_err()` branch, or a `let Some(..) = ...ok() else` block.
  The repair returns the error with `?`, through `edit_files`: `bad_request!` for a validation or timeliness rejection, `bad_gateway!` for an upstream that refused.
  `Ok(())` stays only where a valid message is filtered out by a rule. A test that expected the swallow is repaired to expect the error.
- **No setting names a format.** A `const NAME: &str = "NAME"` whose name spells `FORMAT` reads a parser's syntax from the deployment.
  The repair holds the syntax in the constant itself, `"%d/%m/%Y"` for a criterion's `DD/MM/YYYY`, through `edit_files`, and the module parses with it.

## Repairs

A failure in the merged tree is the seam between two slices. The repair is the smallest change that closes it.

- Two slices typing one design type differently, or each declaring a struct or a parser of its own for it:
  keep the struct, parser, and validation of the module whose slice owns the type key, and have the later module of the stem `use` and call `crate::<earlier_part>::*`.
  Every module of the stem then reads one shape and runs one pipeline.
- One slice depending on a crate the other did not write: the line in the stem's manifest.
- A list missing a line: add it at the end.
- An entry's array missing a part's entry, or holding one for a part with no such function: add or remove the entry.
- A manifest missing a crate a file uses: add the line.
- Two slices routing one path with one method, or consuming one topic: the owner is whichever slice's requirement states it.
- A topic constant declared twice under two names: keep one, name it from both sides.
- A test asserting what another slice's module no longer does: repair the test to the module.
- A cache-aside function answering an empty table where `StateStore::get` answered `None`: write the fetch and the store into it.

Repair it where the layout puts it: in the slice's own module, its lines, its stem's manifest, its test. Change nothing a passing check covers.
A static file is never a repair's to change. One that differs from the template is a finding on your verdict, put back as the finding spells it.
A manifest naming a source is put back to the pin. A list is repaired by adding the line it lacks, never by rewriting one it holds.
Then run the checks again from the one that failed.

Leave what a repair would have to invent: a requirement no module implements, a test for a scenario the tree does not hold, a module the plan has a later wave land.
A failure that remains after the repairs you can justify is reported, not patched over.

A repair is one `edit_files` call. Its `find` is the text as the file holds it. Its `replace` is the text to stand there:

```json
{
  "edits": [
    {
      "path": "src/routes.rs",
      "find": "    let router = router.merge(health::router());\n",
      "replace": "    let router = router.merge(health::router());\n    let router = router.merge(orders::router());\n"
    }
  ]
}
```

## Verdict

Answer with one JSON object and nothing else. No prose before it or after it.

- `passed` — `true` when every check passed on its last run, `false` otherwise.
- `failures` — one entry per check that still failed, naming the check and quoting the tail of what it printed: the first error lines of `cargo`, or the failing test names.
  Empty when `passed` is `true`.

The verdict is held to the tree before it is accepted, as [the rules held](#the-rules-held) says.
A finding under `## Findings` names a rule the tree breaks with what to put back or remove.
Repair it through the two tools. Run the checks the repair touches again. Answer again. The verdict is yours once the tree stands, passing or failing.

A passing verdict is this, exactly:

```json
{
  "passed": true,
  "failures": []
}
```

## Worked example

A wave whose `payments` slice built against the design's `Money` type while the `orders` slice, merged beside it, declared the type with another shape.
`orders`' declaration was repaired to the design's and clippy run again clean. The tests then found a transition `payments` does not implement, left for the report:

```json
{
  "passed": false,
  "failures": [
    "check 2 (cargo test --workspace): crates/payments/tests/payments.rs — `a_cancelled_order_cannot_be_paid` fails: expected `bad_request`, got Ok"
  ]
}
```

Report what the checks found on their last run, never what the tree should hold.

Answer with one JSON object.
