# target.build

Build one slice of the plan into the lent project tree. The tree is an omnia guest in Rust.
The adapter has already scaffolded your slice's files. Your work is to fill their markers.
Then run the one check command, repair what it prints, and answer with one JSON object.

## Inputs

- `$WORKSPACE` — the project tree, lent writable. It sits on the commit the brief names.
- **The slice's plan entry** — its id, name, requirements, the design types it owns, and the slices it depends on.
- **The specification, cut to the slice** — the requirements to implement, each with its acceptance scenarios.
- **The design, cut to this slice** — the sections every slice shares, with the `Type:` blocks cut to yours: a type the adapter laid for you is one line pointing at the scaffold section, since its signature stands in your module's doc; a type another slice owns stands whole where your slice's text names it, and is left out otherwise, listed by key at the end; a `(mechanism)` type always stands whole.
- **What the adapter scaffolded for this slice before this turn** — per stem of the slice:
  - the files it wrote or found held, with their text;
  - the lines it added to the lists;
  - every marker to fill, in order, each quoted as the block one `edit_files` edit replaces;
  - the one check command, with your stem filled in;
  - the passages of the references the shapes it laid use.

Other slices of the wave build beside you from the same commit. Your files merge with theirs under the rules the [layout](references/layout.md) states. Keep to its shape.

## How to work

Work in this order. Do not skip ahead.

1. Read the brief whole. It carries the text of every file the adapter wrote for you. Do not read those files again.
2. Read a file the brief names as **held** once, in the tree, before you edit it. An earlier slice wrote it.
3. Fill the markers, in the brief's order. Each marker is one `edit_files` edit. Copy its `find` from the brief's block exactly.
4. Add the list lines the brief says are yours to add, through `edit_files`.
5. Run the check command the brief spells. Read what it prints. Repair it through `edit_files`. Run it again. Stop when it passes.
6. Answer with one JSON object: `covered` and `written`.

Write through the two tools alone.

- `edit_files` changes within a file. Each edit's `find` must match one place in the file. Several edits may go in one call.
- `write_files` writes a file whole. Use it only for a file of your own that the brief does not name. Its `delete` removes a file.
- A refused `find` names the line of the file nearest its first line. Copy that line's text into a new `find` and try once more. Do not guess.
- The shell runs the check command and nothing else. A file the shell writes, through a redirect, a heredoc, `cp`, or a script, is not yours and is a finding.
- Nothing outside `$WORKSPACE` is reachable. Do not read the registry sources under `.cargo`. Do not run `rg`, `find`, or `ls` over them.
- The signatures you need are in [the omnia API](references/omnia-api.md). The brief quotes the ones your shapes use.
- Read a reference through `read_doc` only when the brief's passages do not answer your question.

## What the adapter scaffolded

The [layout](references/layout.md) is the one shape every slice follows.
The root package is the guest. `common` holds what every stem shares.
Each stem is a crate under `crates/` whose entry composes its parts' routes and topics. The root lists merge the stems.

The adapter laid your slice's place in it before this turn, from the requirements and types the slice carries:

- The stem's crate, `crates/<stem>/`, with its manifest and entry where the tree lacked them.
- Your part, `crates/<stem>/src/<part>.rs`:
  - a path constant and a `.route(..)` per route your requirements carry;
  - a handler per route, over an input struct and an output struct;
  - a `topics(..)` and a consumer per message they carry, with its constant in the topic table;
  - a `pub fn` per requirement carrying neither;
  - each design type you own, laid from its signature, or an empty `{}` where the signature is not readable.
- A route function, `<name>_route`, over `handle_with`, for a `POST` handler and for a handler whose scenarios spell a status other than `200`.
  Its encoder spells `StatusCode::OK`; change that token for a `201` or a `503` on an `Ok`, or declare `<name>_route` as the marker says where the scaffold laid a verb.
  An error status, a `404`, is the error the handler returns.
- Your tests, `crates/<stem>/tests/<part>.rs`: the prelude every test shares and one ignored test per acceptance scenario.
  Each test already sends the request or the delivery its requirement carries.
- Your lines in every list: the stem's four at the root and the part's in the entry.

Each place your code goes is a marker: `todo!("REQ-.. <subject>: ..")` in a body, or `{}` on a type.
The brief quotes every marker as the one block to replace, with the file it stands in.

A marker line reading `[conflict]` names a requirement the sources disagree over.
The specification's notes under its id say what each states. Fill it from the scenarios, as any other.

## The markers

Fill each kind as the shape below shows. Every `find` is the block the brief quotes, copied exactly.

### A design type

A design type the brief names as laid is finished: its fields and variants stand as the signature in its doc spells them.
Each field is documented, the wire's names are kept through `#[serde(rename = "..")]`, and `Eq` is dropped where a float reaches it.
Change a field only where a scenario's wire shape or the check requires it.
A field the brief names as `serde_json::Value` stands for a type another slice declares; leave it, and read the value's fields where the behaviour needs them.

A design type the brief lists among the markers, `pub struct <Type> {}` or `pub enum <Type> {}`, has a signature the scaffold could not lay.
It becomes the type its doc's signature spells, as the [handlers](references/handlers.md) reference declares a design type.

- Document every field or variant.
- Keep the wire's names through `#[serde(rename = "..")]`.
- Keep the scaffolded derives. Drop `Eq` where a field is a float.
- A type that is a client, a connection, a settings struct, or a lifecycle is a mechanism. The [translation](references/translation.md) writes nothing for it.
  Leave its struct empty and say nothing more of it.

```json
{
  "edits": [
    {
      "path": "crates/orders/src/reading.rs",
      "find": "pub struct OrderRef {}",
      "replace": "pub struct OrderRef {\n    /// The order id.\n    pub id: Option<String>,\n    /// The order code.\n    pub code: Option<String>,\n}"
    }
  ]
}
```

### A handler

Fill the handler as the [handlers](references/handlers.md) reference spells it.

- Fill `<Name>Input` with one field per path parameter, which the adapter scaffolded, query parameter, or body field.
- Fill `<Name>Output` with the response's fields.
- Replace the block the brief quotes, the `let _ = (&input, &context);` line and the `todo!`, with the body.
- A handler's `Ok` is answered `200` as JSON. Another status is changed in the route function, as said above.
- Reach the host through `context.provider()` alone.
- A setting is a `pub(crate) const` of your part whose value is its name. Read it through `Config::get` at use.
- A topic you publish is a constant of `common::topic`, qualified through `config::topic`.
- Every failure is a classified `omnia_sdk::Error`, propagated with `?`: a fetch that errors, a status that is not success, a body that does not parse.
  Never catch one and answer `Ok(())` or an empty collection.
- One handler serves every requirement its marker names. Two requirements at one method and path are one route.
- A requirement's route is its `Route:` line. The adapter scaffolded the constant, the `.route(..)`, and the handler from it.
  A method and a path a statement or a scenario spells in prose is no route. Add none for it.

```json
{
  "edits": [
    {
      "path": "crates/orders/src/reading.rs",
      "find": "    let _ = (&input, &context);\n    todo!(\"REQ-030 orders.get-id.return-order: the handler\")",
      "replace": "    let raw = StateStore::get(context.provider(), &input.id).await?\n        .ok_or_else(|| not_found!(\"order `{}`\", input.id))?;\n    Ok(serde_json::from_slice(&raw)?)"
    }
  ]
}
```

### A consumer

A requirement carrying a message, its `Message:` line, is a consumer the adapter scaffolded.
It is `<Name>Input` over the delivery and a `pub(crate) async fn(input, context) -> omnia_sdk::Result<()>`.
It is registered in your part's `topics(..)` under the table's constant for the topic.
The adapter declared the constant at the end of `crates/common/src/topic.rs` where no slice had, and added the `<part>::topics,` entry to the stem's entry.

- Fill the input with the delivery's fields.
- Write the body as a handler's. A rejected delivery is `Err(bad_request!(..))`, never `Ok(())`.
- The registration itself is never a requirement. The host registers and acknowledges.

```json
{
  "edits": [
    {
      "path": "crates/orders/src/fulfilment.rs",
      "find": "    let _ = (&input, &context);\n    todo!(\"REQ-032 orders.on-placed.count-lines: the consumer\")",
      "replace": "    let delta = i64::from(input.lines);\n    StateStore::increment(context.provider(), LINES_PLACED, delta).await?;\n    Ok(())"
    }
  ]
}
```

### A function

A requirement carrying neither a route nor a message is a `pub fn` with a marker: a stage of a pipeline, a rule every route obeys, a behaviour another part calls.

- Give it the signature the behaviour needs. Change its parameters and its return type as the behaviour demands.
- Return `omnia_sdk::Result<T>`. Build an error with `bad_request!`, `not_found!`, `bad_gateway!`, or `server_error!`.
- Never a custom error enum. Never an `Option<T>` that swallows the reason.
- Add no route to give it a surface.

```json
{
  "edits": [
    {
      "path": "crates/orders/src/reading.rs",
      "find": "pub fn within_limit() -> omnia_sdk::Result<()> {\n    todo!(\"REQ-033 orders.within-limit: the behaviour\")\n}",
      "replace": "pub fn within_limit(n: usize, max: usize) -> omnia_sdk::Result<()> {\n    if n > max {\n        return Err(bad_request!(\"{n} > {max}\"));\n    }\n    Ok(())\n}"
    }
  ]
}
```

Where such a requirement consumes a topic that no `Message:` line carries, the function is that stream's handler instead.
The design's `## APIs and integrations` lists an inbound stream whose bullet cites it or another requirement of your stem.
Or the stream's deliveries are what its scenario's `when` describes.

- Make it a `pub(crate) async fn(input, context) -> omnia_sdk::Result<()>`.
- Add a `pub(crate) fn topics(router, env)` to your part: `router.route(config::topic_for(env, topic::<NAME>), messaging::consume(<handler>))`.
- Add the constant at the end of `crates/common/src/topic.rs` where no slice has declared it.
- Add one `<part>::topics,` entry at the end of the array in the entry's `topics(..)`.

A topic you publish to and no requirement of yours consumes is its constant in the topic table alone, named from your handler through `config::topic`.
The slice that ends a pipeline registers the consumer where no part of the stem has. That is the slice that publishes, or that `depends-on` the stem's other slices.
Its handler calls the earlier parts' functions in the pipeline's order.
A part that publishes in a stem no route or topic reaches, and no other stem calls, is a finding on your report. Nothing ever runs it.

### A test

Each scenario is a `#[tokio::test]` named by it, under `#[ignore = "<id> <scenario>: unfilled"]`.
The adapter laid its act from the requirement's route or message:
a request through `send(provider(), request)`, a delivery through `deliver(provider(), topic::<NAME>, payload)`, or `provider()` alone.
Replace the block the brief quotes with the filled test, as [testing](references/testing.md) spells it.

- The `#[ignore = ..]` line goes.
- The arrangement is over the doubles of `omnia_test::guest`. A service the tree does not run is a double handed to the provider, never a live connection.
- The act stays as laid, with the parameter, the body, or the payload the scenario states in place of the placeholders.
- The assertions are the scenario's: the status, the body, and what the doubles hold after.
- A scenario the specification gives you is a test. Invent none. Delete none.
- A scenario you cannot yet satisfy keeps its ignored test as scaffolded. Its requirement stays out of `covered`.

```json
{
  "edits": [
    {
      "path": "crates/orders/tests/reading.rs",
      "find": "#[ignore = \"REQ-033 over_limit: unfilled\"]\nasync fn over_limit() {\n    let _ = provider();\n    todo!(\"REQ-033 over_limit: arrange, act, assert\")\n}",
      "replace": "async fn over_limit() {\n    let err = within_limit(11, 10).expect_err(\"over limit\");\n    assert_eq!(err.code(), \"bad_request\");\n}"
    }
  ]
}
```

### A list line

The adapter added the stem's line in each root list, the part's `pub mod` line and its `routes()` and `topics,` entries in the entry, and the constant of each topic your requirements carry.
The brief says `added` or `already held` for each.
What the adapter does not derive is yours to add, at the end of its list, where the layout's table says:

- The constant of a topic you publish to.
- A `<part>::topics,` entry for a consumer no `Message:` line carried.
- One `<crate>.workspace = true` line at the end of your stem's `[dependencies]` for each crate your code uses beyond the six the manifest holds.
  A crate the root manifest does not pin carries its version instead.
- A line the brief says was not added, because the file lacked its anchor.

Change no other line. A line you find in a list is not yours to rewrite, reorder, or fold your own into.
The merge keeps every form it is given, and the verify then has two.

```json
{
  "edits": [
    {
      "path": "crates/orders/Cargo.toml",
      "find": "tracing.workspace = true\n",
      "replace": "tracing.workspace = true\nchrono.workspace = true\n"
    },
    {
      "path": "crates/common/src/topic.rs",
      "find": "pub const ORDER_PLACED: &str = \"orders-placed.v1\";\n",
      "replace": "pub const ORDER_PLACED: &str = \"orders-placed.v1\";\n/// An order was read.\npub const ORDER_READ: &str = \"orders-read.v1\";\n"
    }
  ]
}
```

## What you never write

- No crate. Your stem's is scaffolded, one word whatever its hyphens: `crates/order-items/` for `order-items`.
  A crate written under any other name is a finding.
- No `crates/start/`. Never add `start` to `Cargo.toml`. The seed and the host are the program's start.
- No `mount`, no router of your own beyond your part's `routes()`, no change to `src/lib.rs`, and no edit to another stem's crate.
  The host runs a stem through its `routes()` and `topics(..)` alone.
- No seed file. Every file outside `crates/<stem>/` beside `Cargo.lock` is the exemplar's guest template, scaffolded when the repository was initialised.
  Read one where you need its text. Write or report none.
- No change to the root `Cargo.toml` outside its two stem lists. A pin, a lint, or a feature changed there is a finding with the lines named.
- No scaffolded file written whole. Edit it.
- No file left behind by a check or a test, beside your files and `Cargo.lock`. Remove such a stray through `delete`.

## The `start` stem

Where your slice carries requirements under `start` beside its own stem's, read them by the table in [what `start` becomes](references/guardrails.md).

- A `start` requirement carrying a route is laid under the crate of the stem its route's resource names: `GET /health/live` in `crates/health/`.
  The brief says so. Fill it as any handler.
- A `start` requirement carrying no route gets nothing scaffolded. The seed and the host satisfy what it states, or nothing does.
- Cover one the seed and the host satisfy: the component answers HTTP and deliveries, the routers reach every handler, the settings come from the host.
- A deployment setting a handler reads is a `pub(crate) const` of the part that reads it, written by that part's slice.
- Credential validation or rotation, client construction, table prefetch, a flag that decided whether a consumer ran, signals, periodic jobs, and lifecycle types:
  write nothing, and leave them out of `covered`.
- A slice of `start` requirements alone, none carrying a route, is never put to you.

## Reuse, and the platform's names

- Where your slice `depends-on` an earlier slice of your own stem, `crates/<stem>/src/<earlier_part>.rs`, import and call `crate::<earlier_part>::*`.
  Never duplicate its parser, types, or conversion.
- Where integrating your slice exposes a defect in the earlier part, an outer XML root wrapper, a missing wire field, a filter mismatch, fix that part through `edit_files`.
  Keep every test under `crates/<stem>/tests/` passing.
- A test of yours that compensates for the earlier part has found that defect. Repair the part. Let the test state the wire as the fixture spells it.
- Where your slice depends on another stem, depend on its crate as `<other>.workspace = true` if the tree holds it.
  Else implement against the design's types in your own module.
- A requirement, a scenario, or a design type that names the source's platform is read for the behaviour behind it: a broker and its consumer, a vault, an agent, a timer, a server.
  It lands as the [translation](references/translation.md) says: the capability that plays the role, the mechanism's own parts written nowhere.
- A requirement whose whole statement is the mechanism is left out of `covered`, its marker standing. Do not stub it.
- What a guest cannot do, what the host does for it, and the capability it reaches instead are in the [guardrails](references/guardrails.md).

## The check

The brief spells one command, with your stem filled in:
`cargo clippy -p <stem> --all-targets -- -D warnings && cargo test -p <stem> && cargo check --target wasm32-wasip2 && cargo +nightly fmt --all -- --check`.
Once your edits are in, run it in `$WORKSPACE` as the brief spells it. It stops at the first step that fails. What it prints is the one thing to repair.

- `clippy` is your crate and its tests under the workspace's lints: `pedantic` and `nursery`, every `pub` item documented, no `unsafe`, and the host boundary.
  The [guardrails](references/guardrails.md) spell the boundary. A `disallowed` method, type, or macro names the capability to reach instead. The repair is that capability.
  No `allow` or `expect` lifts a lint. A handler `routes()` does not name, or a part the entry does not list, is dead code here.
- The first run resolves the workspace and writes `Cargo.lock`, which stays, and `target/`, which `.gitignore` keeps out.
- A failure in a crate not yours is a type the design names and your module misuses, or a crate that is not there yet. Work around it in yours.
- `test` is your crate's tests. They fail only for what your module does not yet do. A test still ignored is one you have not filled.
- `check` is every crate for the guest. A crate that reaches past the component fails here, and the module that named it is the one to change.
- `fmt` names a file to reformat through `edit_files`, as it reports. The formatter may wrap a list statement over several lines, which is still one entry.
- Where `fmt` alone fails because no nightly toolchain is installed, the steps before it passed and the check is done.

Repair what it reports, through `edit_files` within a file, or `write_files` for a file of your own. Run it again until it passes.
Report once it passes, or once a step fails for a reason outside your slice, which the report then leaves uncovered.

## Report

Answer with one JSON object and nothing else. No prose before it or after it.

- `covered` — each requirement id of the specification above that your module now implements and its test confirms, once each.
  Include a requirement whose scenario `THEN` was rendered as `[unknown]` when your test asserts the outcome its statement states.
  Leave an id out rather than claim what the tree does not hold or a test does not pass.
- `written` — the files the two tools wrote, once each, as a `/`-separated path relative to `$WORKSPACE`: the scaffolded files you edited, and the lists you added a line to.

The report is held to the tree before it is accepted. Each rule the tree breaks comes back as a finding under `## Findings`, every line named with the repair to make.
The rules are the layout's. The [handlers](references/handlers.md), [testing](references/testing.md), and [translation](references/translation.md) references spell the shapes that keep them.

- A marker of a covered requirement stands, or a scenario of one is still ignored, still a `todo!`, or missing. Fill it, or leave the id out of `covered`.
- A file stands beside your stems' crates and `Cargo.lock` that the tree did not hold before your turn. Remove it through `delete`.
- A crate is written for a stem your requirements do not lead with, or for a stem split at a hyphen.
- A module is the process: a function that starts, runs, listens, or shuts down, or a `Main`, `App`, or `Server` type. A file under `crates/start/`.
- The root `Cargo.toml` differs from the seed outside its two stem lists, or an entry of its path list names a crate the tree lacks.
- A crate your stem's manifest names that no file of the stem spells. A constant of the topic table nothing names. A stem that publishes and nothing reaches.
- A store read under a constant key that answers its miss as an empty collection or a default. A `Deserialize` struct wrapping a response in one `data` field.
- A refusal answered `Ok(())` from inside a catch. A setting whose name spells `FORMAT`.

Repair a finding through `edit_files` where it is a change within a file, `delete` where a file goes. Then answer again. The report is yours once the tree stands.

## Worked example

A slice `orders-reading` (`SLICE-006`) implementing `REQ-030` and `REQ-031`, the first part of the `orders` stem, which another slice also builds this wave.
The adapter scaffolded the stem's manifest and entry, the part with a handler per route and an ignored test per scenario, and the stem's lines in the root lists.
The turn filled the handlers and the tests. It added a `chrono` line to the stem's manifest for the order's timestamp.
It declared the constant of the topic it publishes to in the topic table.
`REQ-031` depends on a `customers` module another slice is building this wave, so its test cannot pass yet.
Its marker and its ignored test stand as scaffolded, and it is left uncovered:

```json
{
  "covered": ["REQ-030"],
  "written": [
    "crates/orders/Cargo.toml",
    "crates/orders/src/reading.rs",
    "crates/orders/tests/reading.rs",
    "crates/common/src/topic.rs"
  ]
}
```

Answer with one JSON object.
