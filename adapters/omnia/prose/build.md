# target.build

Build one slice of the plan into the lent project tree as an omnia guest in Rust, check it lints, its tests pass, and it compiles for `wasm32-wasip2`, and report what you covered and wrote.

## Inputs

- `$WORKSPACE` — the project tree, lent writable, sitting on the commit the brief names. Read it through the workspace tools; write it through this call's two tools alone: `write_files` lays a file whole, one or more per call, each created or replaced at a `/`-separated path relative to `$WORKSPACE`, and removes a file through the same call's `delete`; `edit_files` changes within a file the tree holds, each edit the one place its `find` matches and the text to put there. Lay your module, your test, and your stem's manifest in one `write_files` call and add your lines to the lists in one `edit_files` call; a fix the loop below asks for is an `edit_files` call where it is a change within a file, a `write_files` call where the file is rewritten whole. The shell runs the checks below in `$WORKSPACE` and nothing else: a file the shell writes, through a redirect, a heredoc, `cp`, or a script, is outside the tools and is not yours. Nothing outside `$WORKSPACE` is reachable.
- **The slice's plan entry** — its id, name, requirements, the design types it owns, and the slices it depends on.
- **The specification, cut to the slice** — the requirements to implement, each with its acceptance scenarios.
- **The design, whole** — the types and sections every slice shares.

Build this slice completely in one pass. Other slices of the wave are building beside you from the same commit; what you write merges with theirs under the rules the [layout](references/layout.md) states, so keep to its shape.

## What to write

The [layout](references/layout.md) is the one shape every slice follows: the root package is the guest, `common` holds the tables every stem shares, and each stem is a crate under `crates/`. In brief:

- **Your module.** `<stem>` is the first segment of your requirement ids (`orders` for `orders.create`). Your slice builds a part of each stem its requirements lead with, as `crates/<stem>/src/<part>.rs`, `<part>` your slice's name less the stem and its hyphen where the name leads with them, else the name whole, in snake_case (`reading` for `orders-reading`, `orders` for the slice `orders`). Everything your requirements need lives in that module: the handlers and the design types you own, `pub` where another module may use them, every `pub` item documented, as the workspace lints demand.
- **Your lines.** Each requirement that states a method and a path is one route; each that consumes a topic is one topic. A route is a constant at the end of `crates/common/src/routes/http.rs`, a statement before `router.with_state(..)` in `src/routes.rs`, and an entry before the `];` of `ROUTES` in `tests/routes.rs`; a topic the same three in `crates/common/src/routes/topic.rs`, `src/messaging.rs` before `tracing::debug!(..)`, and `tests/messaging.rs`; a setting you read is a constant after `ENV` in `crates/common/src/config.rs`. Your stem is one `pub mod <part>;` line at the end of `crates/<stem>/src/lib.rs`, one `<stem> = { path = "crates/<stem>" }` line after `common = { path = "crates/common" }` in the root `Cargo.toml`, and one `<stem>.workspace = true` line at the end of the same file, the same lines whichever slice writes them. A crate your files use that the stem's manifest does not name is one line at the end of its `[dependencies]`. Add your lines where the layout's table says through `edit_files`, and change no other: a line you find in a list is not yours to rewrite, reorder, or fold your own into, whatever form the compiler would also accept, since the merge keeps every form it is given and the verify then has two. A list file is never written whole: the root's are in the tree before you start, and a stem's two you lay hold your lines alone.
- **Your wiring** is those lines and nothing else: no `mount`, no router of your own, no change to `src/lib.rs`. A slice none of whose requirements states a route or a topic adds nothing to the root lists: the behaviour is `pub fn`s your tests call directly, and no route is laid to give a description a surface. Never edit another stem's crate to be reached.
- **Your handlers**, as the [handlers](references/handlers.md) reference spells them: one `async fn(input, context) -> omnia_sdk::Result<output>` per route or topic, the design's types as `serde` structs, the host reached through `context.provider()` alone, a setting read under a key of `common::config`, a topic published under a constant of `common::routes::topic` qualified through `config::topic`, and every failure a classified error. The signatures of the route functions, the messaging router, the capability traits, and the test doubles, as the pinned crates spell them, are in [the omnia API](references/omnia-api.md): read them there rather than probing the crates through the shell. What a guest cannot do, what the host does for it and is never simulated, and the capability it reaches instead, is in the [guardrails](references/guardrails.md).
- **Your test file**, `crates/<stem>/tests/<part>.rs`: one `#[tokio::test]` per acceptance scenario, named by the scenario in snake_case, calling your handler through `Client::call` over the doubles of `omnia_test::guest`, as [testing](references/testing.md) spells it. A scenario the specification gives you is a test; invent none. A service the tree does not run is a double handed to the provider, never a live connection.
- **The seed**, every file outside `crates/<stem>/` beside `Cargo.lock`, is in the tree before you start: the adapter lays the exemplar's guest template where the tree lacks a file, byte for byte as the exemplar holds it, and your brief names the files laid for this slice, which count as your files. Read a seed file in the tree where you need its text; no document fences it. You edit the lists in it and nothing else: the root `Cargo.toml` pins every shared crate under `[workspace.dependencies]`, so a crate your module needs that it does not pin carries its version in your stem's manifest and is never added to the pins; `src/lib.rs`, `examples/runtime.rs`, the shared crate's shape, and the operator's tooling are laid once and never touched. The root `Cargo.toml` is also held by the adapter once your turn ends: every line of it that is neither the seed's nor a stem's line in one of its two lists is put back as the seed spells it, so a pin, a lint, or a feature you change there does not reach your commit, and a check that passed through the change fails at the wave's verify.
- **Your stem's manifest**, `crates/<stem>/Cargo.toml`, as the layout spells it with your lines in its `[dependencies]`, when the tree holds none; your lines alone otherwise. A pinned crate is `<crate>.workspace = true`; one the pins lack carries its version; another stem is `<other>.workspace = true`.
- **The `start` stem**, where your slice holds it: the seed is the program's start, and a `start` requirement it and the host satisfy — the component answers HTTP and deliveries, the routers reach every handler, the settings come from the host — is yours to cover, and no other slice's, whichever slice's brief named it laid; what is code, a health route a requirement states or a rule every handler calls, goes in `crates/start/` as in any stem. A rule every route obeys — authentication, a body limit, a request log — is a `pub fn` your module exports and the handlers it covers call at their top through your crate, never a route under another stem's prefix or a placeholder for one. Everything else the source did at start — read its settings, build and hand on its clients, connect, listen, watch itself, catch its signals, keep a lifecycle type — translates by the table in [what `start` becomes](references/guardrails.md): a setting is a constant in the catalog read where it is used, a client is the calling stem's function over a capability, and the rest is nothing laid, each requirement covered or left uncovered as the table's last column says, never simulated.
- **Another slice's module is not yours to change, nor to lay.** Depend on its crate where the design has yours depend on it, as `<other>.workspace = true`; where it is not in the tree yet, implement against the design's types in your own module and leave the dependency for the slice that lands it. Lay no crate or module for a stem your requirements do not lead with, not as a stub: the slice that owns it lays it, and a stub it finds is a module it must write over.

Read `crates/`, the two tables, and the two root lists before writing: a slice built before this one, or a resumed build, may already hold a crate, a constant, or a line.

## The loop

Once the tree holds your files, mark the time (`touch "$TMPDIR/before"`), then run each check in `$WORKSPACE` and read what it prints, in this order:

1. `cargo clippy -p <stem> --all-targets -- -D warnings` — your crate and its tests, under the workspace's lints: `pedantic` and `nursery` among them, every `pub` item documented, no `unsafe`. The first run resolves the workspace, which writes `Cargo.lock`, which stays, and `target/`, which `.gitignore` keeps out of the tree. A failure in a crate not yours is a type the design names and your module misuses, or a crate that is not there yet, which you work around in yours.
2. `cargo test -p <stem>` — your crate's tests; they fail only for what your module does not yet do.
3. `cargo check --target wasm32-wasip2` — every crate for the guest, so your lines resolve and your module compiles for the target; a crate that reaches past the component fails here, and the module that named it is the one to change.
4. `cargo +nightly fmt --all -- --check`, where `rustup run nightly rustfmt --version` succeeds; otherwise skip it — the formatter's configuration takes nightly `rustfmt`. A file it names is one you reformat through `edit_files`, or `write_files` whole, as it reports; the formatter may wrap a list statement over several lines, which is still one entry.
5. `find . -type f -not -path './target/*' -newer "$TMPDIR/before"` — once the checks pass, this names the files you wrote and `Cargo.lock` alone. Any other path is something a check or a test left in the tree, and the whole change set of this copy is what gets sealed as your slice's commit: remove each such path through `delete` before you report.

Fix what a check reports, through `edit_files` within a file or `write_files` for a file whole, and run it again until the checks pass. Report only once they pass and the last names nothing but yours, or once a check fails for a reason outside your slice, which the report then leaves uncovered.

## Report

Answer with one JSON object:

- `covered` — each requirement id the specification above holds that your module now implements and its test confirms, once each. Leave an id out rather than claim what the tree does not hold or a test does not pass.
- `written` — the files the two tools wrote, and the seed files your brief names as laid for this slice, once each, as a `/`-separated path relative to `$WORKSPACE`. A file a check wrote, `Cargo.lock`, is the tree's whether or not you list it.

## Worked example

A slice `orders-reading` (`SLICE-006`) implementing `REQ-030` and `REQ-031`, a part of the `orders` stem another slice also builds this wave, in a later wave whose brief named no seed file laid, its module using `chrono` for the order's timestamp; the root `Cargo.toml`, the route list, the route rung, and the path table are written because its lines went into them. `REQ-031` depends on a `customers` module another slice is building this wave, so its test cannot pass yet and it is left uncovered:

```json
{
  "covered": ["REQ-030"],
  "written": [
    "crates/orders/Cargo.toml",
    "crates/orders/src/lib.rs",
    "crates/orders/src/reading.rs",
    "crates/orders/tests/reading.rs",
    "crates/common/src/routes/http.rs",
    "Cargo.toml",
    "src/routes.rs",
    "tests/routes.rs"
  ]
}
```
