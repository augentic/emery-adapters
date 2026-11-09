# Layout

The one shape every slice of an omnia build follows, so that slices built beside one another in a wave merge clean and the integrated tree lints, tests, and builds for `wasm32-wasip2` as a whole. It is the root-package guest the [omnia exemplar](https://github.com/augentic/omnia-exemplar) teaches, whose guest template the adapter compiles in and scaffolds: the root package is the component, a `common` crate holds the topic table every stem shares, and each stem is a crate under `crates/` that folds its parts' handlers into one `Handlers`, which the root's router lists.

## The tree

```
Cargo.toml               the root package `guest` and the workspace: every shared pin, the lints, one list
Cargo.lock               written by cargo, never by hand
src/
  lib.rs                 the guest: `router()` over the list of every stem's handlers, and the two exports
tests/
  routes.rs              the route rung: the merged router builds, and an unknown path is not found
  messaging.rs           the messaging rung: the merged router builds, and an unknown topic is unhandled
examples/
  runtime.rs             the local host, behind the `host` feature
crates/
  common/                the crate every stem shares
    src/
      lib.rs             the crate's doc and `pub mod topic;`
      topic.rs           the topic table: one constant per topic
  <stem>/                one crate per stem, the package `<stem>`
    Cargo.toml           the stem's manifest: the crates its files use
    src/
      lib.rs             the entry: one `pub mod` line per part, `handlers()` over each part's handlers
      <part>.rs          a slice's module: its paths and settings, its `handlers()`, its handlers and the types it owns
    tests/
      <part>.rs          one test per acceptance scenario of a slice, through the stem's handlers
Makefile  mise.toml  rust-toolchain.toml  rustfmt.toml  taplo.toml  clippy.toml  deny.toml  .gitignore
.vscode/  supply-chain/  .github/          the operator's tooling, scaffolded once and never edited
```

- `<stem>` is the first segment of a requirement's dotted id: `orders` for `orders.create`, `payments` for `payments.charge`. It names the directory `crates/<stem>/` and the package `<stem>`, which Rust spells with every hyphen an underscore: `payment_methods::..` for `payment-methods`. A stem is one word whatever its hyphens, never split at one into a crate and a part: `payment-methods` is `crates/payment-methods/`, and `crates/payment/src/methods.rs` is a crate written for a stem the plan does not hold, which the adapter refuses at the build with the directory named. A stem named as a crate the root manifest pins, or `guest`, takes the package name `<stem>-stem` so the names never collide, its directory staying `crates/<stem>/`; one named `common` takes both `common-stem` and `crates/common-stem/`, since `crates/common/` is the shared crate. The adapter scaffolds the stem's crate from the slice's requirement ids before the slice's turn, so a slice writes no crate itself.
- Every slice builds a part of each stem its requirements lead with and owns `crates/<stem>/src/<part>.rs` and `crates/<stem>/tests/<part>.rs`, where `<part>` is the slice's name less the stem and the hyphen after it where the name leads with them, else the name whole, spelled in snake_case: `reading` for the slice `orders-reading`, `payment_capture` for the slice of that name under `payments`, and `orders` for the slice `orders` itself, the one slice of a stem the plan did not cut. A part whose name is a Rust keyword takes a trailing underscore: `match_`, `type_`. The adapter scaffolds both files before the slice's turn — the module with a path constant, a `.route(..)` in `handlers()`, and a handler over the route's extractors and an empty output struct per route the requirements state, a consumer and a `.topic(..)` per message, a `pub fn` per requirement stating neither, each body a `todo!("REQ-.. <subject>: ..")` marker, a `Settings` struct where the slice spells a setting, and each design type the slice owns laid from its signature as the [handlers](handlers.md) reference spells — its fields or variants, their renames and docs, `Eq` dropped where a float reaches it — or an empty `pub struct {}`, or `pub enum {}` where the signature is an enumeration, where the signature cannot be laid whole; the test file with the prelude its tests share and one `#[tokio::test]` per acceptance scenario under `#[ignore = "<id> <scenario>: unfilled"]` over the act the requirement's route or message lays and a marker — and the slice fills them. Under the stem, a slice edits those two files, adds what the adapter does not derive to the lists, and touches nothing else.
- Part names are unique in the plan, so two slices in the same wave never own one module. When a part is cut into several slices (`slice i of k` with `k > 1`), they run in serial waves under the part's name and share one module `crates/<stem>/src/<part>.rs` and test file `crates/<stem>/tests/<part>.rs`, never a numbered module: the first slice lays the part's owned design types and its initial handlers, functions, and tests, and each later slice (`index > 1`) finds the held module and test file already extended by the adapter with its new handlers, consumers, functions, registrations, and ignored tests, fills its new markers in place without rewriting what earlier slices of the part built, and uses the part's types directly or imports another part's as `crate::<part>::Order`.
- A stem's crate holds everything its requirements need: its modules, and its manifest naming the crates they use. A part is private to its stem until the stem's entry lists it, and its `pub` items are reachable at `<stem>::<part>::Item`; its handlers, its `handlers()`, its paths, and its settings are `pub(crate)`, reached through the stem's `handlers()` alone, so a handler no `handlers()` registers, or a part the entry does not list, is dead code the first check refuses.
- A module uses another stem through its crate, named in its own stem's manifest as `<other> = { path = "../<other>" }`, and imported as `use <other>::<part>::..` for the `pub` types and functions the other stem exports; a part reaches a sibling part through `crate::<part>::..`. A stem that names a topic uses `common`, for the table's constant. A crate never depends on the root package `guest`.
- Where a slice `depends-on` an earlier slice of its own stem, its module imports and calls `crate::<earlier_part>::..` for the types, deserialization, validation, and conversion the earlier slice built, never duplicating them in its own module, and repairs any defect integration exposes in `crates/<stem>/src/<earlier_part>.rs` while keeping its tests passing. A module another stem owns is not yours to change; where the design has yours depend on a stem the tree lacks, implement against the design's types in your own module and leave the dependency for the slice that lands it.

## The lists

Five files hold a list slices add lines to, and hold nothing a slice has any other reason to touch; the stem's entry holds two. Each list is one block; a line goes at the end of it, where the table below says, and nothing else in the file changes: the engine keeps both sides' lines of such a file, so two slices adding a line each at once merge clean, and two slices adding the same line add it once. A line that does anything but what the list is for, or an edit outside the block, breaks that. The adapter adds a slice's lines before its turn: the stem's `<stem> = { path = "crates/<stem>" }` line in the root manifest, its `<stem>::handlers()` entry in the root router's array, and the part's `pub mod` line and `<part>::handlers()` entry in the entry, each where the table says and once, a line an earlier slice added found rather than added again; the brief names each as added or held, and a list whose anchor the file lacks as not added, for the slice to add itself. What the adapter does not derive is the slice's to add through `edit_files`: a constant in the topic table for a topic it publishes, and a crate line in the stem's manifest. A root list file is never written whole, since it is in the tree before you start; a stem's entry and manifest are scaffolded whole by the adapter, in the one text below with the first slice's lines in their lists, and every other line the same whichever slice's turn edits it.

| List | File | One entry is | Added |
| --- | --- | --- | --- |
| the topic table | `crates/common/src/topic.rs` | `/// <doc>` and `pub const <NAME>: &str = "<topic>";` | at the end of the file |
| the root router | `src/lib.rs`, the array in `router()` | `<stem>::handlers(),` | one per line under the comment heading the `Handlers::from([..])` array, at the end |
| the guest's crates | `Cargo.toml`, `[dependencies]` | `<stem> = { path = "crates/<stem>" }` | after `omnia-sdk.workspace = true`, at the end of the file |
| a stem's parts | `crates/<stem>/src/lib.rs` | `pub mod <part>;` | among the `pub mod` lines, where the formatter sorts it |
| a stem's handlers | `crates/<stem>/src/lib.rs`, the array in `handlers()` | `<part>::handlers()` | in the `Handlers::from([..])` array, as the formatter lays it |
| a stem's crates | `crates/<stem>/Cargo.toml`, `[dependencies]` | `<crate>.workspace = true` | at the end of the file |

`<stem>` in the root router is the stem's package as Rust spells it, `payment_methods::handlers()` for `payment-methods`. The root's array is laid one entry per line from the seed on: the comment that heads it keeps the formatter from folding it onto one line however few entries it holds, so a stem's entry is one line added at the end and never a rewrite of the line before it, and two stems first listed in one wave each add their line; a stem whose handlers are all parts with none is still listed, so a stem that gains a route in a later wave changes nothing at the root. The root's `src/lib.rs`, with its list empty:

```rust
//! # guest
//!
//! An Omnia WASI guest: every stem's handlers merged onto one router over one
//! provider. Only the WASI exports are `wasm32`-gated: the router is generic
//! over its provider, so `tests/routes.rs` and `tests/messaging.rs` build the
//! production router natively under `omnia_test::guest::Provider` doubles.

use omnia_sdk::Capabilities;
use omnia_sdk::api::{Client, Handlers, Router, http, messaging};

/// Build the guest router over one provider-owning [`Client`].
///
/// Omnia creates one component instance per request or delivery, so the
/// router and the client are built inside each call. The list holds one
/// `<stem>::handlers(),` per stem, each added at the end, in the array
/// form a stem lists its parts in; the comment heading the array keeps it
/// one entry per line, so two stems added at once merge as two lines.
/// Merging panics where two stems serve one path under one method or
/// consume one topic, so the rungs under `tests/` hold the list to one
/// handler apiece.
pub fn router<P: Capabilities>(provider: P) -> Router<P> {
    Handlers::from([
        // one `<stem>::handlers(),` per stem, each added at the end
    ])
    .with_state(Client::new(provider))
}

http::export!(router);
messaging::export!(router);
```

The list with one stem added, and with three:

```rust
    Handlers::from([
        // one `<stem>::handlers(),` per stem, each added at the end
        orders::handlers(),
    ])
    .with_state(Client::new(provider))
```

```rust
    Handlers::from([
        // one `<stem>::handlers(),` per stem, each added at the end
        orders::handlers(),
        billing::handlers(),
        payments::handlers(),
    ])
    .with_state(Client::new(provider))
```

A stem the list does not name compiles clean and serves nothing, which the adapter refuses with the entry named.

The topic table of `common`, a file holding its `//!` and nothing else until a slice adds a constant; a constant is documented, as every `pub` item of the tree is:

```rust
//! Messaging topics: one constant per topic, each added at the end.
//!
//! A publisher and its consumer name one constant here, so the two agree on
//! the topic by construction.
```

A topic is a kebab-case noun phrase, `order-placed`, and carries no environment: the host maps the guest's topic names onto the broker's. A constant is declared once, by the slice that owns the topic — the slice that consumes it, where another slice publishes to it — and named everywhere else as `common::topic::<NAME>`: the handler that publishes names the same constant its consumer is registered under, since the two slices may land in different waves and the table is the one place both reach. A constant no module names is a finding at the wave's verify, since nothing publishes or consumes it. An HTTP path is a stem's own, declared in the part that serves it; a setting is a stem's own, declared in the part that reads it; neither is a table of `common`.

The root manifest's list, with one stem added, closing the file:

```toml
# What the guest wires: the two below, then one line per stem, each added at the end.
[dependencies]
common.workspace = true
omnia-sdk.workspace = true
orders = { path = "crates/orders" }
```

## The stem's entry

`crates/<stem>/src/lib.rs` is the crate's doc, its `pub mod` list, and the `handlers()` the root router lists, scaffolded as one text by the adapter before the stem's first slice builds, with that slice's lines in the lists, and never otherwise written whole: for a slice that finds it in the tree the adapter adds the `pub mod` line among the others, where the formatter sorts it, and the `<part>::handlers()` entry in the array where the part registers a route or a topic. The array is the stem's list and may be empty: `Handlers::from([..])` merges the parts' handlers in order, and a stem with no route or topic yet is the fold over none. Every line outside the two lists is the same for every stem but the doc's name, so two slices writing one stem in one wave merge line by line. The entry of a stem of one part:

```rust
//! # orders
//!
//! The `orders` stem.

pub mod placing;

use omnia_sdk::Capabilities;
use omnia_sdk::api::Handlers;

/// The stem's handlers, merged onto the root routers.
pub fn handlers<P: Capabilities>() -> Handlers<P> {
    Handlers::from([placing::handlers()])
}
```

The entry of the exemplar's fixture stem, whose four parts no longer fit the anchor's line, is the same text with the array laid vertically:

```rust
pub fn handlers<P: Capabilities>() -> Handlers<P> {
    Handlers::from([
        catalogue::handlers(),
        dispatch::handlers(),
        lookup::handlers(),
        placing::handlers(),
    ])
}
```

The doc line is the same for every stem. `Capabilities` is the SDK's own bound, every capability trait at once, so the entry's signature never changes for a new one.

## The part

A part declares what it serves as `pub(crate)` constants, registers its handlers through `pub(crate) fn handlers()`, declares the settings it reads as one `Settings` struct, and holds its handlers and the design types it owns, as [handlers](handlers.md) spells them. A part with neither a route nor a topic omits `handlers()`, and the entry's array has no entry for it. The fixture's `placing.rs`, less its handlers:

```rust
//! Placing an order within the deployment's line limit, and hearing it confirmed.

use axum::Json;
use axum::routing::post;
use common::topic;
use omnia_sdk::api::{Context, Handlers};
use omnia_sdk::{
    Capabilities, Config, Message, Publish, Result, Settings, StateStore, bad_request,
};
use serde::{Deserialize, Serialize};

// The paths the part serves.
pub(crate) const ORDERS: &str = "/orders";

// The state key of the running count of lines placed.
pub(crate) const LINES_PLACED: &str = "orders/lines-placed";

pub(crate) fn handlers<P: Capabilities>() -> Handlers<P> {
    Handlers::new().route(ORDERS, post(place)).topic(topic::ORDER_PLACED, on_placed)
}

// The settings the part reads: `ORDERS_LIMIT`.
#[derive(Debug, Settings)]
#[setting(prefix = "ORDERS_")]
pub(crate) struct PlacingSettings {
    limit: usize,
}
```

A path is axum's, `/orders/{id}`, declared in the part that serves it. Two methods on one path are two `.route(..)` calls over one constant in one part, or one `.route(path, get(a).post(b))`. A setting is what the deployment supplies and may vary between deployments — an upstream's endpoint, an identity or a credential's key, a bound an operator tunes — a field of the part's `Settings` struct, its key its name upper-cased behind the struct's prefix or spelled in its own `#[setting(key = "..")]`, typed as its value parses, loaded through `<Part>Settings::load(context.config()).await?` at the moment of use, and documented by the struct's `//` naming the keys; a value the specification states — a code mapping, a format string, a timezone or a locale, a threshold a scenario spells — is a `const` of the part as the scenario states it and no setting, and a setting that configured the source's broker, vault, or agent alone is the host's and gets none, as the [translation](translation.md) has it. Two parts that read one setting each declare it, under the same key. A state key joins the stem and what it holds, `orders/lines-placed`, `orders/order/<id>` through one `pub(crate) fn order_key`.

## The wiring

The guest is `src/lib.rs`: `router()` folds every stem's `handlers()` into one `Router<P>` over one `Client<P>`, and the two exports hand it each request and each delivery; `Capabilities` bounds `P` on every capability a stem may ask for, so the signature changes for no new one, and nothing of `src/lib.rs` but its list ever changes. Nothing runs `main`: the host instantiates the component and hands it each request or delivery, one instance apiece.

So a slice is wired into the guest through its stem: a route is one `.route(..)` in its part's `handlers()`, a topic one `.topic(..)` there, the part one entry in the entry's array, and the stem one entry in the root's, and never a `mount` of its own, a router of its own, or a line in any file but the lists. A requirement served at a route, its `Route:` line, is one route: a path constant and a `.route(..)` in the part, and the handler, all scaffolded by the adapter from the typed route the requirement carries, the handler's body a marker the slice fills; a method and a path spelled in prose under no `Route:` line is none. A requirement that consumes a topic is one topic: where its `Message:` line carries it, the adapter scaffolded the constant in the topic table where no slice had declared it, the `.topic(..)` in the part's `handlers()`, the `common` line in the stem's manifest, and the consumer; where the design alone names the stream, the slice wires those itself. A requirement that publishes names the table's constant from its handler through `context.publish().send(topic::<NAME>, &Message::json(&payload)?)`, and adds the constant where no slice has. A requirement states a topic when the design's `## APIs and integrations` lists an inbound stream whose bullet cites it or another requirement of its stem, or whose deliveries its scenario describes, since the subscription itself is the host's and never a requirement. A slice none of whose requirements states a route or a topic has nothing in the lists but its `pub mod` line and the stem's root entries: its behaviour is the `pub fn`s the adapter scaffolded a marker for and its module exports, its tests call them directly, and no route is added to give a description a surface, since a probe route answering a requirement that states none is a surface the specification does not hold. The host runs a stem through its `handlers()` alone, so the slice that ends a stem's pipeline — the one that publishes, or that depends on the stem's other slices — registers the stream's consumer in its part's `handlers()` over the earlier parts' functions where no part has, and a stem that publishes and is reached by no route, no topic, and no other stem's call is a finding on its report.

Two routes may share one path with different methods; one method twice on one path, or one topic twice, is the panic the root rungs catch, and the owner is whichever slice's requirement states it.

A rule every route obeys — authentication, a body limit, a request log — is a function, not a route. The stem that owns the rule exports it from its module as a `pub fn` (`auth::authentication::authenticate(&headers) -> Result<Claims, omnia_sdk::Error>`), and every handler the rule covers calls it at its top, through the owning stem's crate named in its own stem's manifest. The owning stem adds no route under another stem's prefix and no placeholder for one; a request the rule refuses is asserted in the tests of the stem whose route it is, where the call stands, as [testing](testing.md) spells it.

Where the plan has a `start` stem, the seed and the host are `start`, and no crate `crates/start/` is ever written: the adapter refuses a report or a verdict over any file under `crates/start/`. A deployment setting a `start` requirement names is a field of the `Settings` struct of the part that reads it; a `start` requirement the seed and the host satisfy — the component answers HTTP and deliveries, the router reaches every handler, the settings come from the host — is the `start` slice's to cover over the seed, scaffolded before any slice; a stated health route lives in its own stem's crate (`crates/health/`), and a cross-cutting rule lives in the stem that owns it; one the host satisfies by not needing it — a process to start, a listener to bind, a client to connect, a secret retriever or table prefetch, a loop to keep alive, a graceful stop — is covered by nothing in the tree and left uncovered, never simulated, as the [guardrails](guardrails.md) spell out.

## The stem's crate

`crates/<stem>/Cargo.toml` is scaffolded by the adapter before the stem's first slice builds, its `[dependencies]` the four fixed lines below, `common.workspace = true` sorted among them where the part names a topic; a slice adds one line there for each crate its files use beyond them and changes nothing else. The `[target.'cfg(not(target_arch = "wasm32"))'.dev-dependencies]` block is the two lines below, the doubles and the runtime every test uses, and is never edited or extended: `omnia_test::guest` stands in for every mocking crate, and a crate a test alone needs beyond the two — `bytes` or `http` for an arrangement — is one line in the stem's `[dependencies]` like any crate the stem's files use. So two slices creating one stem in one wave differ under `[dependencies]` alone, which the merge unions. A pinned crate is `<crate>.workspace = true`; one the root's pins lack carries its version in this manifest alone and never a `git` or `registry` source; another stem is `<other> = { path = "../<other>" }`.

```toml
[package]
name = "orders"
description = "The `orders` stem"
publish.workspace = true
edition.workspace = true
rust-version.workspace = true
version.workspace = true

[lints]
workspace = true

# Target-gated so the guest's wasm32 dependency graph never sees the doubles.
[target.'cfg(not(target_arch = "wasm32"))'.dev-dependencies]
omnia-test = { workspace = true, features = ["guest"] }
tokio.workspace = true

# One line per crate the stem's files use, each added at the end.
[dependencies]
axum.workspace = true
omnia-sdk.workspace = true
serde.workspace = true
serde_json.workspace = true
```

The crate lints under the workspace's `[workspace.lints]`, the exemplar's, through its `[lints]` table: every `pub` item carries a `///`, a module its `//!`, a fallible `pub fn` its `# Errors`, a `pub fn` answering a value and reaching nothing its `#[must_use]`, clippy's `pedantic` and `nursery` groups are warnings the checks deny, and the host boundary is the `disallowed` lists of `clippy.toml`, forbidden, as the [guardrails](guardrails.md) spell. A module is `crates/<stem>/src/<part>.rs`: its `//!`, its constants, its `handlers()`, its settings, the handlers, and the design types the slice owns, `pub` where another module may use them, as [handlers](handlers.md) spells them; the adapter scaffolds its shape and the slice fills the markers. Its tests are `crates/<stem>/tests/<part>.rs`, one `#[tokio::test]` per acceptance scenario driving the stem's `handlers()` with a request through `send`, or with a delivery through `deliver`, over the doubles of `omnia_test::guest`, as [testing](testing.md) spells them; a test file opens with a `//!` and declares nothing `pub`.

## The static files

The seed is every file outside `crates/<stem>/` beside `Cargo.lock`, which cargo writes: the exemplar's guest template, with the root router, the two root rungs, the local host, and `common` beside it. The adapter scaffolds it once, when the repository is initialised, before any slice builds, byte for byte as the exemplar holds it, so every slice finds it in place and scaffolds, writes, and reports none of it: a slice that needs a seed file's text reads it in the tree. The three root lists above, in `Cargo.toml`, `src/lib.rs`, and `crates/common/src/topic.rs`, are the only parts of it a slice edits, and the text around each list is static. Everything else is scaffolded once and never edited, by a slice or a repair: at a wave's verify the adapter reads every such file against the template when the verdict is answered, and one that differs is a finding on the verdict, with the lines named, repaired before the verdict is accepted. The static files are:

- `tests/routes.rs`, `tests/messaging.rs`, `crates/common/Cargo.toml`, and `crates/common/src/lib.rs`: the two rungs and the shared crate's shape. The rungs hold no table: `tests/routes.rs` builds the merged router and sends an unknown path, `tests/messaging.rs` builds it and delivers an unknown topic, so two stems claiming one route or one topic fail there as they build and nothing of a slice's goes in.
- `src/lib.rs` outside its list, and the root `Cargo.toml` outside its list: the two exports and `router()`'s signature; `[workspace.package]`, `[workspace.lints]`, the pins under `[workspace.dependencies]`, the `host` feature, and the target tables. A pinned version is the tree's; a crate the pins lack is named with its version in a stem's manifest, never added to the pins. The adapter holds the root manifest itself, not through the merge: when a slice reports, and when a wave's verdict is answered, every line that is neither the seed's nor a stem's entry in the list is a finding, with the lines named, so a pin, a lint, or a feature changed there is put back before the report or the verdict is accepted and never reaches a commit.
- `examples/runtime.rs`, the local host behind the `host` feature, so no check compiles a runtime.
- `rust-toolchain.toml`, `rustfmt.toml`, `taplo.toml`, `clippy.toml`, `deny.toml`, `.gitignore`, `Makefile`, `mise.toml`, `.vscode/settings.json`, `supply-chain/`, and `.github/`: the operator's toolchain, formatter, lints, dependency policy, vet state, task runner, and workflows, which the checks read and the operator maintains. `clippy.toml` carries the host boundary and is edited by no slice.

The template departs from the exemplar's own root in stated ways, each so that slices built at once merge clean: the root's `router()` lists one stem apiece in the array form a stem lists its parts in, so the list is one line per stem and nothing else of `src/lib.rs` changes; `crates/common` is named `common` in place of an organisation's prefix and holds the topic table alone; and the local host sits behind the `host` feature with the host crates optional, so neither `cargo test --workspace` nor the guest build compiles a runtime. Two more are for the tree a build lands in: `.gitignore` ignores `.emery/`, the directory the build's own engine keeps beside the tree, after the exemplar's rules; and `.github/workflows/publish.yaml` calls the toolkit at the one tag every other workflow and `mise.toml` pin, grants nothing by default and each job the permissions the toolkit workflow it calls needs, and reads what its deploy needs from the repository's own settings: the three Azure secrets by name, and the storage account, the resource group, and the app's name from the `STORAGE_ACCOUNT`, `RESOURCE_GROUP`, and `APP_NAME` variables, the repository's name standing in for the last. No file carries a value for the operator to fill.

## Merging

The engine merges each slice's commit into the integrated tree under these rules, in this order:

| Path | Rule | Why |
| --- | --- | --- |
| `Cargo.lock` | the integrated side kept | the next cargo run regenerates it |
| `Cargo.toml` | both sides' lines kept | its list |
| `src/lib.rs` | both sides' lines kept | the root router's array |
| `crates/common/src/topic.rs` | both sides' lines kept | the topic table |
| `crates/*/Cargo.toml` | both sides' lines kept | a stem's `[dependencies]` list |
| `crates/*/src/lib.rs` | both sides' lines kept | a stem's entry: its `pub mod` list and its array |
| everything else | the model combines both sides | the slice re-merges with the result |

A path one slice alone changed takes that slice's change under no rule, so two slices that keep to their own modules never conflict. The rest of the seed outside the lists is under no rule on purpose: no slice edits them once scaffolded, so two slices changing one at once is handed to the model rather than merged line by line, and the adapter puts them back as the template spells them before a wave's checks and holds them after. The rules keep lines; they do not reconcile edits, and a rule is a file's, not a line's: the root `Cargo.toml` is kept as a union for its list, and what its union would also keep, a pin or a lint one slice changed, the adapter puts back itself, as the static files above say; a stem's entry is kept as a union for its two lists, and what two slices lay outside them is the one text above, the same on both sides, so the union keeps it once. An array laid on one line is one line to the union, which is why the root's array is never one: two stems first listed in one wave each add their own line under the comment, and the union keeps both; a stem's own array is laid as the formatter lays it, since one stem's slices build in series and no two edit it at once. A slice that changes a static line of a list file, or another slice's module, is the one conflict the layout leaves possible: the file is handed to a model turn to combine, and one it cannot combine ends the run, so it is the slice's to avoid.
