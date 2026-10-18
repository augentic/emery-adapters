# Layout

The one shape every slice of an omnia build follows, so that slices built beside one another in a wave merge clean and the integrated tree lints, tests, and builds for `wasm32-wasip2` as a whole. It is the root-package guest the [omnia exemplar](https://github.com/augentic/omnia-exemplar) teaches, whose guest template the adapter compiles in and lays: the root package is the component, a `common` crate holds what every stem shares, and each stem is a crate under `crates/` that composes its parts' routes and topics into one router of each kind, which the root merges.

## The tree

```
Cargo.toml               the root package `guest` and the workspace: every shared pin, the lints, two lists
Cargo.lock               written by cargo, never by hand
src/
  lib.rs                 the guest: the provider and the HTTP and messaging exports
  routes.rs              the route list: one statement per stem, merging the stem's router
  messaging.rs           the topic list: one statement per stem, registering the stem's topics
tests/
  routes.rs              the route rung: the merged router builds, and an unknown path is not found
  messaging.rs           the messaging rung: the registered router builds, and an unknown topic is unhandled
examples/
  runtime.rs             the local host, behind the `host` feature
crates/
  common/                the crate every stem shares
    src/
      lib.rs             the `Capabilities` bound, and the two folds a stem's entry composes its parts with
      topic.rs           the topic table: one constant per topic
      config.rs          `ENV` and the helpers that qualify a topic for the environment
  <stem>/                one crate per stem, the package `<stem>`
    Cargo.toml           the stem's manifest: the crates its files use
    src/
      lib.rs             the entry: one `pub mod` line per part, `router()` over each part's routes, `topics(..)` over each part's topics
      <part>.rs          a slice's module: its paths and settings, its `routes()` and `topics(..)`, its handlers and the types it owns
    tests/
      <part>.rs          one test per acceptance scenario of a slice, through the stem's routers
Makefile  mise.toml  rust-toolchain.toml  rustfmt.toml  taplo.toml  clippy.toml  deny.toml  .gitignore
.vscode/  supply-chain/  .github/          the operator's tooling, laid once and never edited
```

- `<stem>` is the first segment of a requirement's dotted id: `orders` for `orders.create`, `payments` for `payments.charge`. It names the directory `crates/<stem>/` and the package `<stem>`, which Rust spells with every hyphen an underscore: `payment_methods::..` for `payment-methods`. A stem is one word whatever its hyphens, never split at one into a crate and a part: `payment-methods` is `crates/payment-methods/`, and `crates/payment/src/methods.rs` is a crate laid for a stem the plan does not hold, which the adapter refuses at the build with the directory named. A stem named as a crate the root manifest pins, or `common` or `guest`, takes the package name `<stem>-stem` so the names never collide; its directory stays `crates/<stem>/`.
- Every slice builds a part of each stem its requirements lead with and owns `crates/<stem>/src/<part>.rs` and `crates/<stem>/tests/<part>.rs`, where `<part>` is the slice's name less the stem and the hyphen after it where the name leads with them, else the name whole, spelled in snake_case: `reading` for the slice `orders-reading`, `payment_capture` for the slice of that name under `payments`, and `orders` for the slice `orders` itself, the one slice of a stem the plan did not cut. A part whose name is a Rust keyword takes a trailing underscore: `match_`, `type_`. Under the stem, a slice writes those two files, its lines in the lists, and nothing else.
- Slice names are unique in the plan, so two slices never own one module.
- A stem's crate holds everything its requirements need: its modules, and its manifest naming the crates they use. A part is private to its stem until the stem's entry lists it, and its `pub` items are reachable at `<stem>::<part>::Item`; its handlers, its routes, its paths, and its settings are `pub(crate)`, reached through the stem's `router()` and `topics(..)` alone, so a handler no `routes()` names, or a part the entry does not list, is dead code the first check refuses.
- A module uses another stem through its crate, named in its own stem's manifest as `<other>.workspace = true` once the root manifest's path list names it, and imported as `use <other>::<part>::..` for the `pub` types and functions the other stem exports; a part reaches a sibling part through `crate::<part>::..`. Every stem uses `common`. A crate never depends on the root package `guest`.
- Where a slice `depends-on` an earlier slice of its own stem, its module imports and calls `crate::<earlier_part>::..` for the types, deserialization, validation, and conversion the earlier slice built, never duplicating them in its own module, and repairs any defect integration exposes in `crates/<stem>/src/<earlier_part>.rs` while keeping its tests passing. A module another stem owns is not yours to change; where the design has yours depend on a stem the tree lacks, implement against the design's types in your own module and leave the dependency for the slice that lands it.

## The lists

Six files hold a list slices add lines to, and hold nothing a slice has any other reason to touch; the stem's entry holds three. Each list is one block; a slice adds its lines at the end of it, where the table below says, and edits nothing else in the file: the engine keeps both sides' lines of such a file, so two slices adding a line each at once merge clean, and two slices adding the same line add it once. A line that does anything but what the list is for, or an edit outside the block, breaks that. A root list file is never written whole, since it is in the tree before you start; a stem's entry and manifest are written whole by the first slice to lay the stem, in the one text below with that slice's lines in their lists, and every other line the same whichever slice writes it.

| List | File | One entry is | Added |
| --- | --- | --- | --- |
| the topic table | `crates/common/src/topic.rs` | `/// <doc>` and `pub const <NAME>: &str = "<suffix>";` | at the end of the file |
| the route list | `src/routes.rs` | `let router = router.merge(<stem>::router());` | before `router.with_state(..)` |
| the topic list | `src/messaging.rs` | `let router = <stem>::topics(router, &env);` | before `tracing::debug!(..)` |
| the workspace's path crates | `Cargo.toml`, `[workspace.dependencies]` | `<stem> = { path = "crates/<stem>" }` | after `common = { path = "crates/common" }` |
| the guest's crates | `Cargo.toml`, `[dependencies]` | `<stem>.workspace = true` | at the end of the file |
| a stem's parts | `crates/<stem>/src/lib.rs` | `pub mod <part>;` | after the last `pub mod` line |
| a stem's routes | `crates/<stem>/src/lib.rs`, the array in `router()` | `<part>::routes(),` | after `// one entry per part with routes, each added at the end` |
| a stem's topics | `crates/<stem>/src/lib.rs`, the array in `topics(..)` | `<part>::topics,` | after `// one entry per part with topics, each added at the end` |
| a stem's crates | `crates/<stem>/Cargo.toml`, `[dependencies]` | `<crate>.workspace = true` | at the end of the file |

`<stem>` in a root list is the stem's package as Rust spells it, `payment_methods::router()` for `payment-methods`. The two root lists import what a statement needs, so a statement is one line as the table spells it. The two root lists, with their lists empty:

```rust
//! The route list: every stem's routes merged onto one router, one statement per stem.

use common::Capabilities;
use omnia_sdk::api::Client;

use crate::OWNER;

/// Build the HTTP router over one provider-owning [`Client`].
///
/// Omnia creates one component instance per request, so the router and the
/// client are built inside each call. One statement per stem between
/// `Router::new()` and `with_state`, each added at the end of the list:
/// `let router = router.merge(<stem>::router());`. Merging panics where two
/// stems serve one path under one method, so the rung under `tests/` holds
/// the list to one route apiece.
pub fn router<P: Capabilities>(provider: P) -> axum::Router {
    let router = axum::Router::new();
    router.with_state(Client::new(OWNER, provider))
}
```

```rust
//! The topic list: every stem's topics registered on one messaging router, one statement per stem.

use common::{Capabilities, config};
use omnia_sdk::api::{Client, messaging};

use crate::OWNER;

/// Build the exact-topic messaging router over one provider-owning [`Client`].
///
/// Each stem registers its topics under their `{env}-` qualified names, so a
/// delivery from another environment is unhandled rather than consumed. One
/// statement per stem between `Router::new(..)` and the trace, each added at
/// the end of the list: `let router = <stem>::topics(router, &env);`.
/// Registering panics where two stems consume one topic, so the rung under
/// `tests/` holds the list to one consumer apiece.
pub async fn messaging_router<P: Capabilities>(provider: P) -> messaging::Router<P> {
    let env = config::env(&provider).await;
    let router = messaging::Router::new(Client::new(OWNER, provider));
    tracing::debug!(env, "messaging topics qualified");
    router
}
```

The two lists with one stem added to each:

```rust
    let router = axum::Router::new();
    let router = router.merge(orders::router());
    router.with_state(Client::new(OWNER, provider))
```

```rust
    let router = messaging::Router::new(Client::new(OWNER, provider));
    let router = orders::topics(router, &env);
    tracing::debug!(env, "messaging topics qualified");
```

Every stem adds both statements, whether or not it has a route or a topic yet: a stem's `router()` and `topics(..)` are always there, folding whatever parts have them, so the two root lists name every stem of the tree and a stem that gains a route in a later wave changes nothing at the root. A stem the lists do not name compiles clean and serves nothing, which the adapter refuses with the entry named.

The topic table of `common`, a file holding its `//!` and nothing else until a slice adds a constant; a constant is documented, as every `pub` item of the tree is:

```rust
//! Environment-agnostic topic suffixes: one constant per topic, each added at the end.
//!
//! A publisher and its consumer name one constant here, so the two agree on
//! the topic by construction. Prefix a suffix with `{env}-` through
//! [`crate::config::topic`] or [`crate::config::topic_for`] before publishing
//! or subscribing.
```

A topic suffix is a kebab-case noun phrase with a version, `orders-placed.v1`, and never carries the environment, which `config::topic` and `config::topic_for` prefix as `{env}-`. A constant is declared once, by the slice that owns the topic — the slice that consumes it, where another slice publishes to it — and named everywhere else as `common::topic::<NAME>`: the handler that publishes names the same constant its consumer is registered under, since the two slices may land in different waves and the table is the one place both reach. A constant no module names is a finding at the wave's verify, since nothing publishes or consumes it. An HTTP path is a stem's own, declared in the part that serves it; a setting is a stem's own, declared in the part that reads it; neither is a table of `common`.

The root manifest's two lists, with one stem added to each, the path list under `[workspace.dependencies]` and the guest's `[dependencies]` closing the file:

```toml
# The crates of this tree: `common`, then one line per stem, each added at the end.
common = { path = "crates/common" }
orders = { path = "crates/orders" }

# What the guest wires: the four below, then one line per stem, each added at the end.
[dependencies]
axum.workspace = true
common.workspace = true
omnia-sdk.workspace = true
tracing.workspace = true
orders.workspace = true
```

## The stem's entry

`crates/<stem>/src/lib.rs` is the crate's doc, its `pub mod` list, and the two functions the root lists compose, laid as one text by the first slice of the stem with that slice's lines in the three lists, and never otherwise written whole: a slice that finds it adds its `pub mod` line after the last, its `<part>::routes(),` entry at the end of the routes array where the part has a route, and its `<part>::topics,` entry at the end of the topics array where the part has a topic. The two arrays are the stem's lists, each under the comment that anchors it, and each may be empty: `common::routers` folds the parts' routers onto one, `common::topics(router, env).parts` registers the parts' topics in order, and a stem with no route yet is the fold over none. Every line outside the three lists is the same for every stem but the doc's name, so two slices laying one stem in one wave merge line by line. The entry of a stem of one part:

```rust
//! # orders
//!
//! The `orders` stem.

pub mod placing;

use common::Capabilities;
use omnia_sdk::api::{Client, messaging};

/// The stem's routes, merged onto the root router.
pub fn router<P: Capabilities>() -> axum::Router<Client<P>> {
    common::routers([
        // one entry per part with routes, each added at the end
        placing::routes(),
    ])
}

/// The stem's topics, registered on the root messaging router under `env`.
#[must_use]
pub fn topics<P: Capabilities>(router: messaging::Router<P>, env: &str) -> messaging::Router<P> {
    common::topics(router, env).parts([
        // one entry per part with topics, each added at the end
        placing::topics,
    ])
}
```

The `#[must_use]` sits on `topics` alone: `axum::Router` carries its own, and the lints refuse a second. The two functions' doc lines are the same for every stem. `axum` is pinned in the root manifest and named in the stem's manifest as every crate the entry uses is.

## The part

A part declares what it serves and reads as `pub(crate)` constants, registers its handlers through `pub(crate) fn routes()` and `pub(crate) fn topics(router, env)`, and holds its handlers and the design types it owns, as [handlers](handlers.md) spells them. A part with no route omits `routes()`, one with no topic omits `topics(..)`, and the entry's list for it stays as it was:

```rust
//! Placing an order within the deployment's line limit, and hearing it confirmed.

use common::{Capabilities, config, topic};
use omnia_sdk::api::http::post;
use omnia_sdk::api::{Client, Context, messaging};

// The settings the part reads, each key its own name.
pub(crate) const ORDERS_LIMIT: &str = "ORDERS_LIMIT";

// The paths the part serves.
pub(crate) const ORDERS: &str = "/orders";

// The state keys the part reads and writes.
pub(crate) const LINES_PLACED: &str = "orders/lines-placed";

pub(crate) fn routes<P: Capabilities>() -> axum::Router<Client<P>> {
    axum::Router::new().route(ORDERS, post(place))
}

pub(crate) fn topics<P: Capabilities>(
    router: messaging::Router<P>, env: &str,
) -> messaging::Router<P> {
    router.route(config::topic_for(env, topic::ORDER_PLACED), messaging::consume(on_placed))
}
```

A path is axum's, `/orders/{id}`, declared in the part that serves it. Two methods on one path are two `.route(..)` calls over one constant in one part, or one `.route(path, get(a).post(b))`. A setting is what the deployment supplies and may vary between deployments — an upstream's endpoint, an identity or a credential's key, a bound an operator tunes — declared as a `pub(crate) const` whose value is its name, read through `Config::get` at the moment of use, and documented by a `//` naming what the deployment provides under it; a value the specification states — a code mapping, a format string, a timezone or a locale, a threshold a scenario spells — is a `const` of the part as the scenario states it and no setting, and a setting that configured the source's broker, vault, or agent alone is the host's and gets none, as the [translation](translation.md) has it. Two parts that read one setting each declare it, under the same name. A state key joins the stem and what it holds, `orders/lines-placed`, `orders/<id>` through one `pub(crate) fn key`.

## The wiring

The guest is `src/lib.rs`: the production provider, the HTTP and messaging exports the host calls, and `OWNER`, the tenant, `guest`. `router` builds the HTTP router from the route list and `messaging_router` the messaging router from the topic list, each over one `Client<P>`; `common::Capabilities` bounds `P` on every capability a stem may ask for, so neither list's signature changes for a new one, and `src/lib.rs` never changes at all. Nothing runs `main`: the host instantiates the component and hands it each request or delivery, one instance apiece.

So a slice wires itself into the guest through its stem: a route is one `.route(..)` in its part's `routes()`, a topic one in its part's `topics(..)`, the part one entry in each of the entry's arrays it has a function for, and the stem one statement in each root list, and never a `mount` of its own, a router of its own, or a line in any file but the lists. A requirement that states a method and a path is one route: a path constant and a `.route(..)` in the part, and the handler. A requirement that consumes a topic is one topic: a constant in the topic table where no slice has declared it, a `.route(..)` in the part's `topics(..)`, and the handler. A requirement that publishes names the table's constant from its handler through `config::topic(provider, topic::<NAME>)`, and adds the constant where no slice has. A requirement states a topic when the design's `## APIs and integrations` lists an inbound stream whose bullet cites it or another requirement of its stem, or whose deliveries its scenario describes, since the registration itself is the host's and never a requirement. A slice none of whose requirements states a route or a topic adds nothing to the lists but its `pub mod` line and the stem's two root statements: its behaviour is the `pub fn`s its module exports, its tests call them directly, and no route is laid to give a description a surface, since a probe route answering a requirement that states none is a surface the specification does not hold. The host runs a stem through its `routes()` and `topics(..)` alone, so the slice that ends a stem's pipeline — the one that publishes, or that depends on the stem's other slices — registers the stream's consumer in its part's `topics(..)` over the earlier parts' functions where no part has, and a stem that publishes and is reached by no route, no topic, and no other stem's call is a finding on its report.

Two routes may share one path with different methods; one method twice on one path, or one topic twice, is the panic the root rungs catch, and the owner is whichever slice's requirement states it.

A rule every route obeys — authentication, a body limit, a request log — is a function, not a route. The stem that owns the rule exports it from its module as a `pub fn` (`auth::authentication::authenticate(&headers) -> Result<Claims, omnia_sdk::Error>`), and every handler the rule covers calls it at its top, through the owning stem's crate named in its own stem's manifest. The owning stem lays no route under another stem's prefix and no placeholder for one; a request the rule refuses is asserted in the tests of the stem whose route it is, where the call stands, as [testing](testing.md) spells it.

Where the plan has a `start` stem, the seed and the host are `start`, and no crate `crates/start/` is ever laid: the adapter refuses a report or a verdict over any file under `crates/start/`. A deployment setting a `start` requirement names is a `pub(crate) const` of the part that reads it; a `start` requirement the seed and the host satisfy — the component answers HTTP and deliveries, the routers reach every handler, the settings come from the host — is the `start` slice's to cover over the seed, laid before any slice; a stated health route lives in its own stem's crate (`crates/health/`), and a cross-cutting rule lives in the stem that owns it; one the host satisfies by not needing it — a process to start, a listener to bind, a client to connect, a secret retriever or table prefetch, a loop to keep alive, a graceful stop — is covered by nothing in the tree and left uncovered, never simulated, as the [guardrails](guardrails.md) spell out.

## The stem's crate

`crates/<stem>/Cargo.toml` is written by the first slice to lay the stem, its `[dependencies]` holding that slice's lines; a slice that finds it in the tree adds its lines there and changes nothing else. The `[target.'cfg(not(target_arch = "wasm32"))'.dev-dependencies]` block is the three lines below, the doubles, the runtime, and the service adapter every test uses, and is never edited or extended: `omnia_test::guest` stands in for every mocking crate, and a crate a test alone needs beyond the three — `bytes` or `http` for an arrangement — is one line in the stem's `[dependencies]` like any crate the stem's files use. So two slices creating one stem in one wave differ under `[dependencies]` alone, which the merge unions. A pinned crate is `<crate>.workspace = true`; one the root's pins lack carries its version in this manifest alone and never a `git` or `registry` source; another stem is `<other>.workspace = true` once the root manifest's path list names it.

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
tower.workspace = true

# One line per crate the stem's files use, each added at the end.
[dependencies]
axum.workspace = true
common.workspace = true
omnia-sdk.workspace = true
serde.workspace = true
serde_json.workspace = true
tracing.workspace = true
```

The crate lints under the workspace's `[workspace.lints]`, the exemplar's, through its `[lints]` table: every `pub` item carries a `///`, a module its `//!`, a fallible `pub fn` its `# Errors`, a `pub fn` answering a value and reaching nothing its `#[must_use]`, clippy's `pedantic` and `nursery` groups are warnings the checks deny, and the host boundary is the `disallowed` lists of `clippy.toml`, forbidden, as the [guardrails](guardrails.md) spell. A module is `crates/<stem>/src/<part>.rs`: its `//!`, its constants, its `routes()` and `topics(..)`, the handlers, and the design types the slice owns, `pub` where another module may use them, as [handlers](handlers.md) spells them. Its tests are `crates/<stem>/tests/<part>.rs`, one `#[tokio::test]` per acceptance scenario driving the stem's `router()` with a request, or its `topics(..)` with a delivery, over the doubles of `omnia_test::guest`, as [testing](testing.md) spells them; a test file opens with a `//!` and declares nothing `pub`.

## The static files

The seed is every file outside `crates/<stem>/` beside `Cargo.lock`, which cargo writes: the exemplar's guest template, with the two root lists, the two root rungs, the local host, and `common` beside it. The adapter lays it once, when the repository is initialised, before any slice builds, byte for byte as the exemplar holds it, so every slice finds it in place and lays, writes, and reports none of it: a slice that needs a seed file's text reads it in the tree. The four root lists above, in `Cargo.toml`, `src/routes.rs`, `src/messaging.rs`, and `crates/common/src/topic.rs`, are the only parts of it a slice edits, and the text around each list is static. Everything else is laid once and never edited, by a slice or a repair: at a wave's verify the adapter reads every such file against the template when the verdict is answered, and one that differs is a finding on the verdict, with the lines named, repaired before the verdict is accepted. The static files are:

- `src/lib.rs`, `tests/routes.rs`, `tests/messaging.rs`, `crates/common/Cargo.toml`, `crates/common/src/lib.rs`, and `crates/common/src/config.rs`: the program's entry, the two rungs, and the shared crate's shape. The rungs hold no table: `tests/routes.rs` builds the merged router and sends an unknown path, `tests/messaging.rs` builds the registered router and delivers an unknown topic, so two stems claiming one route or one topic fail there as they build and nothing of a slice's goes in.
- The root `Cargo.toml` outside its two lists: `[workspace.package]`, `[workspace.lints]`, the pins under `[workspace.dependencies]`, the `host` feature, and the target tables. A pinned version is the tree's; a crate the pins lack is named with its version in a stem's manifest, never added to the pins. The adapter holds this file itself, not through the merge: when a slice reports, and when a wave's verdict is answered, every line that is neither the seed's nor a stem's entry in one of the two lists is a finding, with the lines named, so a pin, a lint, or a feature changed there is put back before the report or the verdict is accepted and never reaches a commit.
- `examples/runtime.rs`, the local host behind the `host` feature, so no check compiles a runtime.
- `rust-toolchain.toml`, `rustfmt.toml`, `taplo.toml`, `clippy.toml`, `deny.toml`, `.gitignore`, `Makefile`, `mise.toml`, `.vscode/settings.json`, `supply-chain/`, and `.github/`: the operator's toolchain, formatter, lints, dependency policy, vet state, task runner, and workflows, which the checks read and the operator maintains. `clippy.toml` carries the host boundary and is edited by no slice.

The template departs from the exemplar's own root in stated ways, each so that slices built at once merge clean: `common::Capabilities` bounds the two routers in place of a bound listing each handler's capabilities, so the entry never changes; `common::routers` and `common::topics(..).parts` fold a stem's parts, so a stem's entry is one text with three lists; the root lists merge one stem apiece in place of the entry's own chain; `crates/common` is named `common` in place of an organisation's prefix; and the local host sits behind the `host` feature with the host crates optional, so neither `cargo test --workspace` nor the guest build compiles a runtime. Two more are for the tree a build lands in: `.gitignore` ignores `.emery/`, the directory the build's own engine keeps beside the tree, after the exemplar's rules; and `.github/workflows/publish.yaml` calls the toolkit at the one tag every other workflow and `mise.toml` pin, grants nothing by default and each job the permissions the toolkit workflow it calls needs, and reads what its deploy needs from the repository's own settings: the three Azure secrets by name, and the storage account, the resource group, and the app's name from the `STORAGE_ACCOUNT`, `RESOURCE_GROUP`, and `APP_NAME` variables, the repository's name standing in for the last. No file carries a value for the operator to fill.

## Merging

The engine merges each slice's commit into the integrated tree under these rules, in this order:

| Path | Rule | Why |
| --- | --- | --- |
| `Cargo.lock` | the integrated side kept | the next cargo run regenerates it |
| `Cargo.toml` | both sides' lines kept | its two lists |
| `src/routes.rs`, `src/messaging.rs` | both sides' lines kept | the route and topic lists |
| `crates/common/src/topic.rs` | both sides' lines kept | the topic table |
| `crates/*/Cargo.toml` | both sides' lines kept | a stem's `[dependencies]` list |
| `crates/*/src/lib.rs` | both sides' lines kept | a stem's entry: its `pub mod` list and its two arrays |
| everything else | the model combines both sides | the slice re-merges with the result |

A path one slice alone changed takes that slice's change under no rule, so two slices that keep to their own modules never conflict. `src/lib.rs` and the rest of the seed outside the lists are under no rule on purpose: no slice edits them once laid, so two slices changing one at once is handed to the model rather than merged line by line, and the adapter puts them back as the template spells them before a wave's checks and holds them after. The rules keep lines; they do not reconcile edits, and a rule is a file's, not a line's: the root `Cargo.toml` is kept as a union for its two lists, and what its union would also keep, a pin or a lint one slice changed, the adapter puts back itself, as the static files above say; a stem's entry is kept as a union for its three lists, and what two slices lay outside them is the one text above, the same on both sides, so the union keeps it once. A slice that changes a static line of a list file, or another slice's module, is the one conflict the layout leaves possible: the file is handed to a model turn to combine, and one it cannot combine ends the run, so it is the slice's to avoid.
