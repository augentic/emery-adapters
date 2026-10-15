# Layout

The one shape every slice of an omnia build follows, so that slices built beside one another in a wave merge clean and the integrated tree lints, tests, and builds for `wasm32-wasip2` as a whole. It is the root-package guest the [omnia exemplar](https://github.com/augentic/omnia-exemplar) teaches, whose guest template the adapter compiles in and lays: the root package is the component, a `common` crate holds the tables every stem shares, and each stem is a crate of handlers under `crates/`.

## The tree

```
Cargo.toml               the root package `guest` and the workspace: every shared pin, the lints, two lists
Cargo.lock               written by cargo, never by hand
src/
  lib.rs                 the guest: the provider, the HTTP and messaging exports, the capability bound
  routes.rs              the route list: one statement per HTTP route
  messaging.rs           the topic list: one statement per topic consumed
tests/
  routes.rs              the route rung: every route of the list reaches its handler
  messaging.rs           the messaging rung: every topic of the list is routed
examples/
  runtime.rs             the local host, behind the `host` feature
crates/
  common/                the crate every stem shares
    src/
      routes/http.rs     the path table: one constant per HTTP route
      routes/topic.rs    the topic table: one constant per topic
      config.rs          the configuration catalog: one constant per key, and the helpers
  <stem>/                one crate per stem, the package `<stem>`
    Cargo.toml           the stem's manifest: the crates its files use
    src/
      lib.rs             a list alone: one `pub mod` line per slice
      <part>.rs          a slice's module: the handlers and the types it owns
    tests/
      <part>.rs          one test per acceptance scenario of a slice
Makefile  mise.toml  rust-toolchain.toml  rustfmt.toml  taplo.toml  clippy.toml  deny.toml  .gitignore
.vscode/  supply-chain/  .github/          the operator's tooling, laid once and never edited
```

- `<stem>` is the first segment of a requirement's dotted id: `orders` for `orders.create`, `start` for `start.listen`. It names the directory `crates/<stem>/` and the package `<stem>`, which Rust spells with every hyphen an underscore: `payment_methods::..` for `payment-methods`. A stem is one word whatever its hyphens, never split at one into a crate and a part: `payment-methods` is `crates/payment-methods/`, and `crates/payment/src/methods.rs` is a crate laid for a stem the plan does not hold, which the adapter refuses at the build with the directory named. A stem named as a crate the root manifest pins, or `common` or `guest`, takes the package name `<stem>-stem` so the names never collide; its directory stays `crates/<stem>/`.
- Every slice builds a part of each stem its requirements lead with and owns `crates/<stem>/src/<part>.rs` and `crates/<stem>/tests/<part>.rs`, where `<part>` is the slice's name less the stem and the hyphen after it where the name leads with them, else the name whole, spelled in snake_case: `reading` for the slice `orders-reading`, `application_bootstrap` for the slice of that name under `start`, and `orders` for the slice `orders` itself, the one slice of a stem the plan did not cut. A part whose name is a Rust keyword takes a trailing underscore: `match_`, `type_`. Under the stem, a slice writes those two files, its lines in the lists, and nothing else.
- Slice names are unique in the plan, so two slices never own one module.
- A stem's crate holds everything its requirements need: its modules, and its manifest naming the crates they use. A part is private to its stem until the stem's `src/lib.rs` lists it, and `pub` items of a part are reachable at `<stem>::<part>::Item`.
- A module uses another stem through its crate, named in its own stem's manifest as `<other>.workspace = true` once the root manifest's path list names it, and imported as `use <other>::<part>::..`; a part reaches a sibling part through `crate::<part>::..`. Every stem uses `common`. A crate never depends on the root package `guest`.
- A module another slice owns is not yours to change. Where the design has yours depend on it and the tree lacks it, implement against the design's types in your own module and leave the dependency for the slice that lands it.

## The lists

Eleven files hold a list slices add lines to, and hold nothing a slice has any other reason to touch. Each list is one block; a slice adds its lines at the end of it, where the table below says, and edits nothing else in the file: the engine keeps both sides' lines of such a file, so two slices adding a line each at once merge clean, and two slices adding the same line add it once. A line that does anything but what the list is for, or an edit outside the block, breaks that. A list file is never written whole: the root's are in the tree before you start, and a stem's two you lay hold your lines alone.

| List | File | One entry is | Added |
| --- | --- | --- | --- |
| the path table | `crates/common/src/routes/http.rs` | `/// <doc>` and `pub const <NAME>: &str = "<path>";` | at the end of the file |
| the topic table | `crates/common/src/routes/topic.rs` | `/// <doc>` and `pub const <NAME>: &str = "<suffix>";` | at the end of the file |
| the configuration catalog | `crates/common/src/config.rs` | `/// <doc>` and `pub const <KEY>: &str = "<KEY>";` | after `ENV`, before `env` |
| the route list | `src/routes.rs` | `let router = router.route(<path>, <method>(<handler>));` | before `router.with_state(..)` |
| the topic list | `src/messaging.rs` | `let router = router.route(config::topic_for(&env, <suffix>), messaging::consume(<handler>));` | before `tracing::debug!(..)` |
| the route rung's table | `tests/routes.rs` | `(Method::<VERB>, <path>),` | before the table's `];` |
| the messaging rung's table | `tests/messaging.rs` | `<suffix>,` | before the table's `];` |
| the workspace's path crates | `Cargo.toml`, `[workspace.dependencies]` | `<stem> = { path = "crates/<stem>" }` | after `common = { path = "crates/common" }` |
| the guest's crates | `Cargo.toml`, `[dependencies]` | `<stem>.workspace = true` | at the end of the file |
| a stem's modules | `crates/<stem>/src/lib.rs` | `pub mod <part>;` | at the end of the file |
| a stem's crates | `crates/<stem>/Cargo.toml`, `[dependencies]` | `<crate>.workspace = true` | at the end of the file |

`<path>` and `<suffix>` are the constants of the two tables spelled in full, `common::routes::http::ORDERS` and `common::routes::topic::ORDER_PLACED`; `<method>` is a route function of `omnia_sdk::api::http` spelled in full, `omnia_sdk::api::http::post`; `<handler>` is the handler's path through its stem's crate, `orders::placing::place_order`. The two root lists import nothing a statement needs, so a statement names its paths whole, and the formatter may wrap one over several lines: it is still one entry, and it is added as one. The two root lists, with their lists empty:

```rust
//! The route list: every stem's HTTP routes on one router, one statement per route.

use omnia_sdk::api::Client;

use crate::{Capabilities, OWNER};

/// Build the HTTP router over one provider-owning [`Client`].
///
/// Omnia creates one component instance per request, so the router and the
/// client are built inside each call. One statement per route between
/// `Router::new()` and `with_state`, each added at the end of the list:
/// `let router = router.route(<path>, <method>(<handler>));` over a constant
/// of `common::routes::http` and a route function of `omnia_sdk::api::http`,
/// both spelled in full.
pub fn router<P: Capabilities>(provider: P) -> axum::Router {
    let router = axum::Router::new();
    router.with_state(Client::new(OWNER, provider))
}
```

```rust
//! The topic list: every stem's topics on one messaging router, one statement per topic.

use common::config;
use omnia_sdk::api::{Client, messaging};

use crate::{Capabilities, OWNER};

/// Build the exact-topic messaging router over one provider-owning [`Client`].
///
/// Each topic is registered under its `{env}-` qualified name, so a delivery
/// from another environment is unhandled rather than consumed. One statement
/// per topic between `Router::new(..)` and the trace, each added at the end
/// of the list: `let router = router.route(config::topic_for(&env, <suffix>),
/// messaging::consume(<handler>));` over a constant of `common::routes::topic`,
/// spelled in full.
pub async fn messaging_router<P: Capabilities>(provider: P) -> messaging::Router<P> {
    let env = config::env(&provider).await;
    let router = messaging::Router::new(Client::new(OWNER, provider));
    tracing::debug!(env, "messaging topics qualified");
    router
}
```

The route list with one route, and the topic list with one topic, as the formatter leaves them:

```rust
    let router = axum::Router::new();
    let router = router.route(
        common::routes::http::ORDERS,
        omnia_sdk::api::http::post(orders::placing::place_order),
    );
    router.with_state(Client::new(OWNER, provider))
```

```rust
    let router = messaging::Router::new(Client::new(OWNER, provider));
    let router = router.route(
        config::topic_for(&env, common::routes::topic::ORDER_PLACED),
        messaging::consume(orders::fulfilment::on_placed),
    );
    tracing::debug!(env, "messaging topics qualified");
```

A topic whose payload is not JSON is registered through `messaging::consume_with(<handler>, <decode>)`, the decoder a `fn(&Delivery) -> Result<I, DecodeError>` the stem exports beside the handler, as [handlers](handlers.md) spells it.

The two tables of `common`, with their lists empty, each a file holding its `//!` and nothing else until a slice adds a constant; a constant is documented, as every `pub` item of the tree is:

```rust
//! HTTP ingress paths served by the guest: one constant per route, each added at the end.
```

```rust
//! Environment-agnostic topic suffixes: one constant per topic, each added at the end.
//!
//! Prefix a suffix with `{env}-` through [`crate::config::topic`] or
//! [`crate::config::topic_for`] before publishing or subscribing.
```

A path is axum's, `/orders/{id}`; a topic suffix is a kebab-case noun phrase with a version, `orders-placed.v1`, and never carries the environment, which `config::topic` and `config::topic_for` prefix as `{env}-`. A constant is declared once, by the slice that owns the route or the topic — the slice that consumes a topic, where another slice publishes to it — and referenced everywhere else; the handler that publishes names the same constant its consumer is registered under. The configuration catalog opens with `ENV`, the one key the root reads, and holds every other key the tree reads after it, each spelled as its name and documented with what the deployment provides under it. A key is what the deployment supplies and may vary between deployments — an upstream's endpoint, an identity or a credential's key, a bound an operator tunes — and nothing the specification states as a value: a code mapping, a format string, a timezone or a locale, a threshold a scenario spells is a `const` of the module that reads it, and a setting that configured the source's broker, vault, or agent alone is the host's and gets no key:

```rust
/// Deployment environment (`dev`, `prod`). Prefixes every messaging topic;
/// see [`topic`]. One constant per key after this one, each added at the end
/// of the list, before [`env`].
pub const ENV: &str = "ENV";

/// Base URL of the rates upstream.
pub const RATES_URL: &str = "RATES_URL";
```

The two root tables are the rungs' lists. `tests/routes.rs` holds `ROUTES`, every `(method, path)` the route list registers, driven by its `dispatch` test, which sends each an empty request and fails on the ones the router answers itself with an empty `404` or a `405`; `tests/messaging.rs` holds `TOPICS`, every suffix the topic list registers, driven by `routed`, which delivers an empty payload to each under the test's environment and fails on the ones the router reports unhandled. Both check wiring, never semantics, so a handler validates its input before it reaches the host and answers an empty request itself:

```rust
const ROUTES: &[(Method, &str)] = &[
    // one entry per route, each added at the end
    (Method::POST, common::routes::http::ORDERS),
];
```

```rust
const TOPICS: &[&str] = &[
    // one entry per topic, each added at the end
    common::routes::topic::ORDER_PLACED,
];
```

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

A stem's module list, `crates/<stem>/src/lib.rs`, is the crate's one-line doc and one `pub mod` line per slice, so a stem of one slice lists one module; the doc is the same line whichever slice lays it:

```rust
//! The `orders` stem.

pub mod placing;
pub mod reading;
```

## The wiring

The guest is `src/lib.rs`: the production provider, the HTTP and messaging exports the host calls, the `Capabilities` bound, and `OWNER`, the tenant, `guest`. `router` builds the HTTP router from the route list and `messaging_router` the messaging router from the topic list, each over one `Client<P>`; `Capabilities` bounds `P` on every capability a stem may ask for, so neither list's signature changes for a new one, and `src/lib.rs` never changes at all. Nothing runs `main`: the host instantiates the component and hands it each request or delivery, one instance apiece.

So a slice wires itself into the guest by adding statements to the two root lists, one per route and one per topic, and never through a `mount` of its own, a router of its own, or a line in any file but the lists. A requirement that states a method and a path is one route: a constant in the path table, a statement in the route list, an entry in the route rung's table, and the handler in the slice's module. A requirement that consumes a topic is one topic: a constant in the topic table, a statement in the topic list, an entry in the messaging rung's table, and the handler. A requirement that publishes names the topic table's constant from its handler through `config::topic(provider, <suffix>)`, and adds the constant where no slice has. A slice none of whose requirements states a route or a topic adds nothing to the root lists: its behaviour is the `pub fn`s its module exports, its tests call them directly, and no route is laid to give a description a surface, since a probe route answering a requirement that states none is a surface the specification does not hold.

Two routes may share one path with different methods; one method twice on one path, or one topic twice, is the panic the root rungs catch, and the owner is whichever slice's requirement states it.

A rule every route obeys — authentication, a body limit, a request log — is a function, not a route. The stem that owns the rule exports it from its module as a `pub fn` (`start::authentication::authenticate(&headers) -> Result<Claims, omnia_sdk::Error>`), and every handler the rule covers calls it at its top, through the owning stem's crate named in its own stem's manifest. The owning stem lays no route under another stem's prefix and no placeholder for one; a request the rule refuses is asserted in the tests of the stem whose route it is, where the call stands, as [testing](testing.md) spells it.

Where the plan has a `start` stem, its slice owns `crates/start/` as any stem for what is code — a health route, a rule every route calls — and the seed is the rest of it: the component the host runs, the provider, the routers, the environment the topics are qualified for. A `start` requirement the seed and the host satisfy is the `start` slice's to cover, whichever slice's brief named the files laid; one the host satisfies by not needing it — a process to start, a listener to bind, a client to connect, a loop to keep alive, a graceful stop — is covered by nothing in the tree and left uncovered, never simulated, as the [guardrails](guardrails.md) spell out. A tree with no `start` stem is the same program with no module under that name.

## The stem's crate

`crates/<stem>/Cargo.toml` is written by the first slice to lay the stem, its `[dependencies]` holding that slice's lines; a slice that finds it in the tree adds its lines there and changes nothing else. The `[target.'cfg(not(target_arch = "wasm32"))'.dev-dependencies]` block is the three lines below, the doubles and the runtime every test uses, and is never edited or extended: `omnia_test::guest` stands in for every mocking crate, and a crate a test alone needs beyond the three — `bytes` or `http` for an arrangement — is one line in the stem's `[dependencies]` like any crate the stem's files use. So two slices creating one stem in one wave differ under `[dependencies]` alone, which the merge unions. A pinned crate is `<crate>.workspace = true`; one the root's pins lack carries its version in this manifest alone and never a `git` or `registry` source; another stem is `<other>.workspace = true` once the root manifest's path list names it.

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
serde_json.workspace = true
tokio.workspace = true

# One line per crate the stem's files use, each added at the end.
[dependencies]
common.workspace = true
omnia-sdk.workspace = true
serde.workspace = true
serde_json.workspace = true
```

The crate lints under the workspace's `[workspace.lints]`, the exemplar's: every `pub` item carries a `///`, a module its `//!`, a fallible `pub fn` its `# Errors`, a `pub fn` answering a value and reaching nothing its `#[must_use]`, and clippy's `pedantic` and `nursery` groups are warnings the checks deny. A module is `crates/<stem>/src/<part>.rs`: its `//!`, the handlers, and the design types the slice owns, `pub` where another module may use them, as [handlers](handlers.md) spells them. Its tests are `crates/<stem>/tests/<part>.rs`, one `#[tokio::test]` per acceptance scenario calling the handler through `Client::call` over the doubles of `omnia_test::guest`, as [testing](testing.md) spells them; a test file opens with a `//!` and declares nothing `pub`.

## The static files

The seed is every file outside `crates/<stem>/` beside `Cargo.lock`, which cargo writes: the exemplar's guest template, with the two root lists, the two root rungs, the local host, and `common` beside it. The adapter lays it in a tree that lacks it before any slice's turn, byte for byte as the exemplar holds it, so every slice finds it in place and writes none: a slice's brief names the files laid for it, and they count as its files, and a slice that needs a seed file's text reads it in the tree. The eleven lists above are the only parts of it a slice edits, and the text around each list is static. Everything else is laid once and never edited, by a slice or a repair: at a wave's verify the adapter reads every such file against the template when the verdict is answered, and one that differs is a finding on the verdict, with the lines named, repaired before the verdict is accepted. The static files are:

- `src/lib.rs`, `crates/common/Cargo.toml`, `crates/common/src/lib.rs`, `crates/common/src/routes.rs`, and the helpers of `crates/common/src/config.rs`: the program's entry and the shared crate's shape.
- The root `Cargo.toml` outside its two lists: `[workspace.package]`, `[workspace.lints]`, the pins under `[workspace.dependencies]`, the `host` feature, and the target tables. A pinned version is the tree's; a crate the pins lack is named with its version in a stem's manifest, never added to the pins. The adapter holds this file itself, not through the merge: when a slice reports, and when a wave's verdict is answered, every line that is neither the seed's nor a stem's entry in one of the two lists is a finding, with the lines named, so a pin, a lint, or a feature changed there is put back before the report or the verdict is accepted and never reaches a commit.
- `examples/runtime.rs`, the local host behind the `host` feature, so no check compiles a runtime.
- `rust-toolchain.toml`, `rustfmt.toml`, `taplo.toml`, `clippy.toml`, `deny.toml`, `.gitignore`, `Makefile`, `mise.toml`, `.vscode/settings.json`, `supply-chain/`, and `.github/`: the operator's toolchain, formatter, lints, dependency policy, vet state, task runner, and workflows, which the checks read and the operator maintains. `clippy.toml` holds two lists of its own, `doc-valid-idents` and `allowed-duplicate-crates`, which a slice extends by one line where a check names an identifier or a duplicate crate its files brought in, and changes no other way.

The template departs from the exemplar's own root in four stated ways, each so that slices built at once merge clean: `Capabilities` bounds the two routers in place of a bound listing each handler's capabilities, so the entry never changes; the `.route` statements live in the two root lists in place of the entry's own chain; `crates/common` is named `common` in place of an organisation's prefix; and the local host sits behind the `host` feature with the host crates optional, so neither `cargo test --workspace` nor the guest build compiles a runtime. Two more are for the tree a build lands in: `.gitignore` ignores `.emery/`, the directory the build's own engine keeps beside the tree, after the exemplar's rules; and `.github/workflows/publish.yaml` calls the toolkit at the one tag every other workflow and `mise.toml` pin, grants nothing by default and each job the permissions the toolkit workflow it calls needs, and reads what its deploy needs from the repository's own settings: the three Azure secrets by name, and the storage account, the resource group, and the app's name from the `STORAGE_ACCOUNT`, `RESOURCE_GROUP`, and `APP_NAME` variables, the repository's name standing in for the last. No file carries a value for the operator to fill.

## Merging

The engine merges each slice's commit into the integrated tree under these rules, in this order:

| Path | Rule | Why |
| --- | --- | --- |
| `Cargo.lock` | the integrated side kept | the next cargo run regenerates it |
| `Cargo.toml` | both sides' lines kept | its two lists |
| `clippy.toml` | both sides' lines kept | its two lists |
| `src/routes.rs`, `src/messaging.rs` | both sides' lines kept | the route and topic lists |
| `tests/routes.rs`, `tests/messaging.rs` | both sides' lines kept | the rungs' tables |
| `crates/common/src/routes/*.rs`, `crates/common/src/config.rs` | both sides' lines kept | the tables and the catalog |
| `crates/*/Cargo.toml` | both sides' lines kept | a stem's `[dependencies]` list |
| `crates/*/src/lib.rs` | both sides' lines kept | a stem's `pub mod` list |
| everything else | a conflict | the slice is rebuilt over the merged tree |

A path one slice alone changed takes that slice's change under no rule, so two slices that keep to their own modules never conflict. `src/lib.rs` and the rest of the seed outside the lists are under no rule on purpose: no slice edits them once laid, so two slices changing one at once is a conflict rather than a merge, and the entry is never merged line by line; the adapter puts them back as the template spells them before a wave's checks and holds them after. The rules keep lines; they do not reconcile edits, and a rule is a file's, not a line's: the root `Cargo.toml` is kept as a union for its two lists, and what its union would also keep, a pin or a lint one slice changed, the adapter puts back itself, as the static files above say. A slice that changes a static line of any other list file, or another slice's module, is the one conflict the layout leaves possible, and it is the slice's to avoid.
