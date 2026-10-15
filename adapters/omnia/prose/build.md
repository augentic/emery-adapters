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

- **Your module.** `<stem>` is the first segment of your requirement ids (`orders` for `orders.create`), one word whatever its hyphens: `order-items` is the crate `crates/order-items/` (package `order-items`, path `order_items`) and never `crates/order/` with `items` as its part, and the adapter refuses a report over a crate laid under a stem's head. Your slice builds a part of each stem its requirements lead with, as `crates/<stem>/src/<part>.rs`, `<part>` your slice's name less the stem and its hyphen where the name leads with them, else the name whole, in snake_case (`reading` for `orders-reading`, `orders` for the slice `orders`). Everything your requirements need lives in that module: the handlers and the design types you own — less a type that is a client, a connection, a settings struct, or a lifecycle, which the [translation](references/translation.md) lays nothing for — `pub` where another module may use them, every `pub` item documented, as the workspace lints demand.
- **Your lines.** Each requirement that states a method and a path is one route; each that consumes a topic is one topic. A route is a constant at the end of `crates/common/src/routes/http.rs`, a statement before `router.with_state(..)` in `src/routes.rs`, and an entry before the `];` of `ROUTES` in `tests/routes.rs`; a topic the same three in `crates/common/src/routes/topic.rs`, `src/messaging.rs` before `tracing::debug!(..)`, and `tests/messaging.rs`; a topic you publish to and no requirement of yours consumes is its constant in the topic table alone, where no slice has declared it, with no statement and no rung entry, since the rung delivers to every topic it lists and nothing would consume yours; a setting you read is a constant after `ENV` in `crates/common/src/config.rs`, where the deployment supplies it — an endpoint, an identity, a bound the operator tunes — while a value the specification states, a code mapping, a format string, a timezone, a threshold a scenario spells, is a `const` of your module as the scenario states it and no constant of the catalog. Your stem is one `pub mod <part>;` line at the end of `crates/<stem>/src/lib.rs`, one `<stem> = { path = "crates/<stem>" }` line after `common = { path = "crates/common" }` in the root `Cargo.toml`, and one `<stem>.workspace = true` line at the end of the same file, the same lines whichever slice writes them. A crate your files use that the stem's manifest does not name is one line at the end of its `[dependencies]`. Add your lines where the layout's table says through `edit_files`, and change no other: a line you find in a list is not yours to rewrite, reorder, or fold your own into, whatever form the compiler would also accept, since the merge keeps every form it is given and the verify then has two. A list file is never written whole: the root's are in the tree before you start, and a stem's two you lay hold your lines alone.
- **Your wiring** is those lines and nothing else: no `mount`, no router of your own, no change to `src/lib.rs`. A slice none of whose requirements states a route or a topic adds nothing to the root lists: the behaviour is `pub fn`s your tests call directly, and no route is laid to give a description a surface. Never edit another stem's crate to be reached.
- **Your handlers**, as the [handlers](references/handlers.md) reference spells them: one `async fn(input, context) -> omnia_sdk::Result<output>` per route or topic, the design's types as `serde` structs, the host reached through `context.provider()` alone, a setting read under a key of `common::config`, a topic published under a constant of `common::routes::topic` qualified through `config::topic`, and every failure a classified error. The signatures of the route functions, the messaging router, the capability traits, and the test doubles, as the pinned crates spell them, are in [the omnia API](references/omnia-api.md): read them there rather than probing the crates through the shell. What a guest cannot do, what the host does for it and is never simulated, and the capability it reaches instead, is in the [guardrails](references/guardrails.md). A requirement, a scenario, or a design type that names the source's platform — a broker and its consumer, a vault, an agent, a timer, a server — is read for the behaviour behind it and lands as the [translation](references/translation.md) says: the capability that plays the role, the mechanism's own parts laid nowhere, and a requirement whose whole statement is the mechanism left out of `covered` rather than stubbed.
- **Your test file**, `crates/<stem>/tests/<part>.rs`: one `#[tokio::test]` per acceptance scenario, named by the scenario in snake_case, calling your handler through `Client::call` over the doubles of `omnia_test::guest`, as [testing](references/testing.md) spells it. A scenario the specification gives you is a test; invent none. A service the tree does not run is a double handed to the provider, never a live connection.
- **The seed**, every file outside `crates/<stem>/` beside `Cargo.lock`, is in the tree before you start: the adapter lays the exemplar's guest template where the tree lacks a file, byte for byte as the exemplar holds it, and your brief names the files laid for this slice, which count as your files. Read a seed file in the tree where you need its text; no document fences it. You edit the lists in it and nothing else: the root `Cargo.toml` pins every shared crate under `[workspace.dependencies]`, so a crate your module needs that it does not pin carries its version in your stem's manifest and is never added to the pins; `src/lib.rs`, `examples/runtime.rs`, the shared crate's shape, and the operator's tooling are laid once and never touched. The root `Cargo.toml` is held to the seed outside its two lists when you report: every line of it that is neither the seed's nor a stem's line in one of those lists is a finding on your report, with the lines named, and a check that passed through the change is no check at all.
- **Your stem's manifest**, `crates/<stem>/Cargo.toml`, as the layout spells it with your lines in its `[dependencies]`, when the tree holds none; your lines alone otherwise. A pinned crate is `<crate>.workspace = true`; one the pins lack carries its version; another stem is `<other>.workspace = true`.
- **The `start` stem**, where your slice is under it — the slice `start` itself, or one the plan cut from it, `start-prefetch`, `start-credentials`, `start-bootstrap`, each a part of `crates/start/` under the same rules: the seed is the program's start, and a `start` requirement it and the host satisfy — the component answers HTTP and deliveries, the routers reach every handler, the settings come from the host — is yours to cover, and no other slice's, whichever slice's brief named it laid; what is code, a health route a requirement states or a rule every handler calls, goes in `crates/start/` as in any stem. A rule every route obeys — authentication, a body limit, a request log — is a `pub fn` your module exports and the handlers it covers call at their top through your crate, never a route under another stem's prefix or a placeholder for one. Everything else the source did at start — read its settings, build and hand on its clients, connect, listen, prefetch a table, watch itself, catch its signals, keep a lifecycle type — translates by the table in [what `start` becomes](references/guardrails.md): a setting is a constant in the catalog read where it is used, a client is the calling stem's function over a capability, a table prefetched at start is cache-aside over `StateStore` in the stem whose handlers read it, under one key and one single-attempt fetch, never a loader under `crates/start/`, and the rest is nothing laid, each requirement covered or left uncovered as the table's last column says, never simulated. Most of a `start` slice's requirements are of that kind, so its report covers few of them and its module holds little or nothing: a `start` slice that lays a module for each of its requirements has simulated the process, and the adapter refuses the report with each simulation named — a function that starts, runs, listens, or stops, a lifecycle type, a `loop {` that retries or polls, a flag kept in the state store among them. What the clients did for the handlers — consume, publish, fetch, read a secret — is each handler stem's, by the [translation](references/translation.md).
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

The report is held to the tree before it is accepted. Check your tree against this list before you report, since each rule it breaks comes back as a finding under `## Findings` with every line named, and a report that breaks none is accepted first time:

- The root `Cargo.toml` stands as the seed spells it outside its two stem lists; a finding gives the `-` lines to put back and the `+` lines to drop.
- No crate is laid under the head of a hyphenated stem: `crates/order/` for `order-items` is a finding naming the stem's own crate, whose files are removed through `delete` and laid under `crates/<stem>/` with the stem's lines in the root `Cargo.toml` in the head's place.
- No module under `crates/start/src` is the process: a function that starts, stops, runs, listens, serves, bootstraps, or shuts down, or a `Main`, `App`, `Server`, or `Application` type, is a finding at its line, and the module is removed with its `pub mod` line and its test rather than renamed.
- No module under `crates/start/src` loads for another stem: a write of a table to the store (`StateStore::set`, `provider.set(..)`) or a token warmed before the stem that sends it asks (`Identity::access_token`, `provider.access_token(..)`) is a finding at its line, and the module is removed the same way; the table is read cache-aside by the stem whose handlers read it, the token where it is sent.
- No read of the store under a constant key — `StateStore::get(provider, KEY)`, `config::KEY` — answers its miss as nothing anywhere in the function that reads: a `return Ok(Vec::new())`, an `.unwrap_or_default()`, an arm naming `None` whose body is an empty collection — `None => HashMap::new()`, `Ok(None) | Err(_) => Ok(HashMap::new())` — or an `Ok(Vec::new())` standing as the function's tail once the hit has returned above it, is a finding at the answering line, while a default on the hit's own arm — `Some(bytes) => decode(&bytes).unwrap_or_default()` — decodes what the store held and is none; on `Ok(None)` the function fetches the table from its upstream, stores it under the same key with a TTL, and answers what it fetched. A read under a key built from the input is a lookup and passes.
- No module simulates the host: a task, a timer, a thread, a reach to the environment, the file system, a socket, or a process, a `loop {`, a flag kept in the store (`_STARTED_KEY`, `STOP_REQUESTED`, `EXIT_DELAY`, a `b"1"` write), or a crate-level `#![allow(` is a finding at its line, and the spelling is removed from the module.
- Each table agrees with its lists: a route's constant is named by one statement of `src/routes.rs` and one entry of `ROUTES`, a consumed topic's by one statement of `src/messaging.rs` and one entry of `TOPICS`, a published topic's by its publishing module alone, and every statement names a constant rather than a literal.
- Every handler — a `pub async fn` taking a `Context` — is reached by a `.route(..)` statement of the two root lists, handed to its route function or consumer by the statement, or to `handle_with` by a route function of its stem the statement calls; a helper no route runs takes the provider, not the `Context`.
- Every setting you add to `crates/common/src/config.rs` is read by a module through `Config::get`; a key nothing reads is a finding at its line, and a setting that configured the source's broker, vault, or agent alone gets no key.

Repair a finding through `edit_files` where it is a change within a file, `delete` where a module goes, then answer again; the report is yours once the tree stands.

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

## Worked example: a `start` slice

The slice `start` (`SLICE-001`) holds eleven requirements read from the process the source was: `REQ-001` and `REQ-002` state the settings it refused to start without, `REQ-003` the vault client it built, `REQ-004` the token it exchanged at start and refreshed hourly, `REQ-005` the brokers it connected to, `REQ-006` the consumer group it joined, `REQ-007` the APM agent it initialised, `REQ-008` the health route `GET /health` it answered, `REQ-009` the signal handler that drained it, `REQ-010` the `Main` class that ran it all, and `REQ-011` the job it scheduled to reload a table every ten minutes. By the table in [what `start` becomes](references/guardrails.md) the slice writes two constants after `ENV` in the catalog, the health route in `crates/start/src/start.rs` with its three lines and its test, and nothing else: no module for the vault, the token, the brokers, the agent, the signals, the `Main`, or the job, and no key in the state store recording that any of them ran. Its report covers the settings, since the catalog and the reads hold them, the brokers, since the host answers deliveries through the topic list as stated, and the health route, and leaves the other seven out:

```json
{
  "covered": ["REQ-001", "REQ-002", "REQ-005", "REQ-008"],
  "written": [
    "crates/start/Cargo.toml",
    "crates/start/src/lib.rs",
    "crates/start/src/start.rs",
    "crates/start/tests/start.rs",
    "crates/common/src/config.rs",
    "crates/common/src/routes/http.rs",
    "Cargo.toml",
    "src/routes.rs",
    "tests/routes.rs"
  ]
}
```
