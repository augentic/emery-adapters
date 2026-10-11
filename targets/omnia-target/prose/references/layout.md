# Layout

The one shape every slice of an omnia build follows, so that slices built beside one another in a wave merge clean and the integrated workspace lints, tests, and builds for `wasm32-wasip2` as a whole.

## The tree

```
.gitignore
Cargo.toml               the root manifest: the workspace, its pins, the root crate `app`
Cargo.lock               written by cargo, never by hand
rust-toolchain.toml
src/
  lib.rs                 the application: the router list, the provider, the HTTP export
tests/
  routes.rs              the smoke test: every slice's routes mount together
examples/
  runtime.rs             the local host, behind the `host` feature
crates/
  <stem>/                a stem one slice builds whole
    Cargo.toml           the stem's manifest: the crates its files use
    src/
      lib.rs             the module: `mount`, the handlers, the types it owns
    tests/
      <stem>.rs          one test per acceptance scenario
  <stem>/                a stem several slices build
    Cargo.toml           the stem's manifest: the crates every part's files use
    src/
      lib.rs             a list alone: one `pub mod` line per part
      <part>.rs          a part's module: `mount`, the handlers, the types it owns
    tests/
      <part>.rs          one test per acceptance scenario of a part
```

- `<stem>` is the first segment of a requirement's dotted id: `orders` for `orders.create`, `start` for `start.listen`. It names the directory `crates/<stem>/` and the package `app-<stem>`, which Rust spells `app_<stem>` with every hyphen an underscore: `app_payment_methods` for `payment-methods`.
- A slice named by its stem — the slice `orders` holding `orders.*` — builds the stem whole and owns `crates/<stem>/`: its module is `src/lib.rs` and its test file `tests/<stem>.rs`, the stem spelled with underscores.
- Any other slice builds a part of each stem its requirements lead with and owns `crates/<stem>/src/<part>.rs` and `crates/<stem>/tests/<part>.rs`, where `<part>` is the slice's name less the stem and the hyphen after it where the name leads with them, else the name whole, spelled in snake_case: `reading` for the slice `orders-reading`, `application_bootstrap` for the slice of that name under `start`. A part whose name is a Rust keyword takes a trailing underscore: `match_`, `type_`. Under the stem, it writes those two files, its lines in the stem's two lists, and nothing else.
- Slice names are unique in the plan, so two slices never own one module, and at most one builds a stem whole.
- A stem's crate holds everything its requirements need: its modules, and its manifest naming the crates they use. A part is private to its stem until the stem's `src/lib.rs` lists it, and `pub` items of a part are reachable at `app_<stem>::<part>::Item`.
- A module uses another stem through its crate, named in its own stem's manifest (`app-<other> = { path = "../<other>" }`) and imported as `use app_<other>::..`; a part reaches a sibling part through `crate::<part>::..`. A crate never depends on the root crate `app`.
- A module another slice owns is not yours to change. Where the design has yours depend on it and the tree lacks it, implement against the design's types in your own module and leave the dependency for the slice that lands it.

## The lists

Four files hold a list slices add lines to. Each list is one block, every slice adds its line at the end of it, and nothing else in the file is edited: the engine keeps both sides' lines of such a file, so two slices adding a line each at once merge clean, and two slices adding the same line add it once. A line that does anything but what the list is for, or an edit outside the block, breaks that.

The root manifest's `[dependencies]`, the last table of `Cargo.toml`, one line per stem, the same line whichever slice writes it:

```toml
[dependencies]
axum.workspace = true
omnia-sdk.workspace = true
app-health = { path = "crates/health" }
app-orders = { path = "crates/orders" }
```

The router list in `src/lib.rs`, between `let router = axum::Router::new();` and `router.with_state(..)`, one line per slice: a stem built whole mounts its crate, a part mounts its module:

```rust
    let router = axum::Router::new();
    let router = app_health::mount(router);
    let router = app_orders::placing::mount(router);
    let router = app_orders::reading::mount(router);
    router.with_state(Client::new(OWNER, provider))
```

A stem built in parts has a list for its module, `crates/<stem>/src/lib.rs`, one line per part and nothing else:

```rust
pub mod placing;
pub mod reading;
```

And every stem's manifest ends with its `[dependencies]`, the four static lines, then one line per crate a module uses that the four do not cover, added by the slice whose files use it: `<crate>.workspace = true` where the root manifest's `[workspace.dependencies]` pins the crate, `<crate> = "<version>"` with its features where it does not, `app-<other> = { path = "../<other>" }` for another stem:

```toml
[dependencies]
axum.workspace = true
omnia-sdk.workspace = true
serde.workspace = true
serde_json.workspace = true
chrono.workspace = true
uuid.workspace = true
```

## The wiring

The application is `src/lib.rs`: the router, the production provider, and the HTTP export the host calls. The router mounts every module its list names over one `Client<P>` and one owner; `Capabilities` bounds `P` on every capability a module may ask for, so the root never changes for a new one. Nothing runs `main`: the host instantiates the component and hands it each request.

The wiring is one surface. A module the application reaches exports `mount`, which takes the router and returns it with the module's routes merged in, bounding `P` on the capabilities its handlers use and nothing more:

```rust
pub fn mount<P>(router: Router<Client<P>>) -> Router<Client<P>>
where
    P: StateStore + Send + Sync + 'static,
{
    router.merge(Router::new().route("/orders/{id}", get(get_order)))
}
```

`mount` merges a router of its own, so a layer applies to the module's routes alone and the order of the list never matters. Two modules may route one path with different methods; one method twice on one path is the panic `tests/routes.rs` catches. So a slice wires itself into the application by exporting `mount` and adding its line to the router list, and never edits `crates/start/` or another stem's crate to be reached.

Where the plan has a `start` stem, its slice owns `crates/start/` as any stem for what is code — a health route, a layer on its own routes — and the root files are the rest of it: the component the host runs, the provider, the router. A `start` requirement the root files and the host satisfy is covered by the slice that laid them; one nothing in the tree satisfies is left uncovered. A tree with no `start` stem is the same program with no module under that name.

## The tests

`crates/<stem>/tests/<stem>.rs`, or `crates/<stem>/tests/<part>.rs` for a part, over tokio's test macro and the doubles of `omnia_test::guest`:

```rust
use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use omnia_sdk::api::Client;
use omnia_test::guest::Provider;
use tower::ServiceExt as _;

fn app(provider: Provider) -> Router {
    app_orders::reading::mount(Router::new()).with_state(Client::new("test", provider))
}

#[tokio::test]
async fn an_order_is_returned_by_its_id() {
    let provider = Provider::default();
    provider.storage.insert_state(
        "orders/o-1",
        br#"{"id":"o-1","items":["widget"],"placed_at":"2026-10-12T00:00:00Z"}"#,
    );
    let request = Request::get("/orders/o-1").body(Body::empty()).expect("request");

    let response = app(provider).oneshot(request).await.expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.expect("body");
    let order: app_orders::reading::Order = serde_json::from_slice(&body).expect("json");
    assert_eq!(order.id, "o-1");
    assert_eq!(order.items, ["widget"]);
}
```

One test per acceptance scenario, named by the scenario in snake_case, its `WHEN` the arrangement and its `THEN` the assertion. A test mounts the module alone on a fresh router over the doubles and drives it in process with `oneshot`, as [testing](testing.md) spells out; it never listens on a port, and a service the tree does not run is a double handed to the provider, never a live connection.

The root's `tests/routes.rs` is the one test over the whole router, written once with the root files: it proves every module mounts together.

## The root files

Six root files are written byte for byte as below by any slice that finds them missing, so two slices laying them at once lay the same file, and no slice edits one after, save its lines in the two root lists. The root manifest pins every crate the tree shares under `[workspace.dependencies]`, which no slice edits: a stem's manifest names a pinned crate as `<crate>.workspace = true`, and a crate the pins lack with its version in that stem's manifest alone.

`Cargo.toml`, its `[dependencies]` the list above, holding the stem lines of whoever writes it:

```toml
[package]
name = "app"
version = "0.1.0"
edition = "2024"
publish = false

[lib]
crate-type = ["cdylib", "rlib"]

[features]
# The local host, `cargo run --features host --example runtime -- run <wasm>`.
host = [
    "dep:omnia",
    "dep:omnia-wasi-blobstore",
    "dep:omnia-wasi-config",
    "dep:omnia-wasi-docstore",
    "dep:omnia-wasi-http",
    "dep:omnia-wasi-keyvalue",
    "dep:omnia-wasi-messaging",
    "dep:omnia-wasi-otel",
]

[[example]]
name = "runtime"
required-features = ["host"]

[workspace]
members = ["crates/*"]
resolver = "3"

[workspace.dependencies]
axum = { version = "0.8.9", default-features = false, features = ["json"] }
chrono = { version = "0.4.45", default-features = false, features = ["clock", "serde"] }
omnia-sdk = "0.36.0"
omnia-test = { version = "0.36.0", features = ["guest"] }
serde = { version = "1.0.229", features = ["derive"] }
serde_json = "1.0.151"
thiserror = "2.0.20"
tokio = { version = "1.53.1", default-features = false, features = ["macros", "rt"] }
tower = { version = "0.5.3", default-features = false, features = ["util"] }
uuid = { version = "1.24.0", features = ["serde", "v4"] }

[profile.release]
lto = true
opt-level = "s"
strip = true

[target.'cfg(target_arch = "wasm32")'.dependencies]
wasip3 = { version = "0.9.0", features = ["http-compat"] }

[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
omnia = { version = "0.36.0", optional = true }
omnia-wasi-blobstore = { version = "0.36.0", optional = true }
omnia-wasi-config = { version = "0.36.0", optional = true }
omnia-wasi-docstore = { version = "0.36.0", optional = true }
omnia-wasi-http = { version = "0.36.0", optional = true }
omnia-wasi-keyvalue = { version = "0.36.0", optional = true }
omnia-wasi-messaging = { version = "0.36.0", optional = true }
omnia-wasi-otel = { version = "0.36.0", optional = true }

[target.'cfg(not(target_arch = "wasm32"))'.dev-dependencies]
omnia-test.workspace = true
tokio.workspace = true
tower.workspace = true

# One line per stem after the two below, each added at the end.
[dependencies]
axum.workspace = true
omnia-sdk.workspace = true
```

`src/lib.rs`, its router list holding the lines of whoever writes it:

```rust
//! The application: every stem's routes on one router over one provider.

use omnia_sdk::api::Client;
use omnia_sdk::{
    BlobStore, Broadcast, Config, DocumentStore, HttpRequest, Identity, Publish, StateStore,
    TableStore,
};

/// The tenant that owns this deployment.
pub const OWNER: &str = "app";

/// Every capability a stem may bound its provider on.
pub trait Capabilities:
    BlobStore
    + Broadcast
    + Config
    + DocumentStore
    + HttpRequest
    + Identity
    + Publish
    + StateStore
    + TableStore
    + Send
    + Sync
    + 'static
{
}

impl<P> Capabilities for P where
    P: BlobStore
        + Broadcast
        + Config
        + DocumentStore
        + HttpRequest
        + Identity
        + Publish
        + StateStore
        + TableStore
        + Send
        + Sync
        + 'static
{
}

/// The production provider: WASI's own implementation of every capability.
#[cfg(target_arch = "wasm32")]
pub struct Provider;

#[cfg(target_arch = "wasm32")]
impl BlobStore for Provider {}
#[cfg(target_arch = "wasm32")]
impl Broadcast for Provider {}
#[cfg(target_arch = "wasm32")]
impl Config for Provider {}
#[cfg(target_arch = "wasm32")]
impl DocumentStore for Provider {}
#[cfg(target_arch = "wasm32")]
impl HttpRequest for Provider {}
#[cfg(target_arch = "wasm32")]
impl Identity for Provider {}
#[cfg(target_arch = "wasm32")]
impl Publish for Provider {}
#[cfg(target_arch = "wasm32")]
impl StateStore for Provider {}
#[cfg(target_arch = "wasm32")]
impl TableStore for Provider {}

/// The WASI HTTP export.
#[cfg(target_arch = "wasm32")]
pub struct Http;

#[cfg(target_arch = "wasm32")]
wasip3::http::service::export!(Http);

#[cfg(target_arch = "wasm32")]
impl wasip3::exports::http::handler::Guest for Http {
    async fn handle(
        request: wasip3::http::types::Request,
    ) -> Result<wasip3::http::types::Response, wasip3::http::types::ErrorCode> {
        omnia_sdk::api::http::serve(router(Provider), request).await
    }
}

/// The router: every stem's routes over one [`Client`].
///
/// One line per slice after `Router::new()`, each added at the end.
pub fn router<P: Capabilities>(provider: P) -> axum::Router {
    let router = axum::Router::new();
    router.with_state(Client::new(OWNER, provider))
}
```

`tests/routes.rs`:

```rust
//! The whole router over the test doubles: every slice's routes mount together.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use omnia_test::guest::Provider;
use tower::ServiceExt as _;

#[tokio::test]
async fn an_unknown_path_is_not_found() {
    let request = Request::get("/nothing-here").body(Body::empty()).expect("request");

    let response = app::router(Provider::default()).oneshot(request).await.expect("response");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
```

`examples/runtime.rs`:

```rust
//! The local host: serves the built component over WASI's default backends.
//!
//! ```shell
//! cargo build --target wasm32-wasip2 --release
//! cargo run --features host --example runtime -- run target/wasm32-wasip2/release/app.wasm
//! ```

cfg_select! {
    not(target_arch = "wasm32") => {
        use omnia_wasi_blobstore::{BlobstoreDefault, WasiBlobstore};
        use omnia_wasi_config::{ConfigDefault, WasiConfig};
        use omnia_wasi_docstore::{DocStoreDefault, WasiDocStore};
        use omnia_wasi_http::{HttpDefault, WasiHttp};
        use omnia_wasi_keyvalue::{KeyValueDefault, WasiKeyValue};
        use omnia_wasi_messaging::{MessagingDefault, WasiMessaging};
        use omnia_wasi_otel::{OtelDefault, WasiOtel};

        omnia::runtime!({
            hosts: {
                WasiBlobstore: BlobstoreDefault,
                WasiConfig: ConfigDefault,
                WasiDocStore: DocStoreDefault,
                WasiHttp: HttpDefault,
                WasiKeyValue: KeyValueDefault,
                WasiMessaging: MessagingDefault,
                WasiOtel: OtelDefault,
            }
        });
    }
    _ => {
        fn main() {}
    }
}
```

`rust-toolchain.toml`:

```toml
[toolchain]
channel = "stable"
components = ["clippy"]
targets = ["wasm32-wasip2"]
```

`.gitignore`:

```
target/
.emery/
```

The `host` feature keeps the local host out of every check: the host crates are optional, native-only dependencies, and the example runs only with the feature on, so neither `cargo test --workspace` nor the guest build compiles a runtime. `Capabilities` names the nine capability traits the provider answers, every one of them WASI's own on `wasm32` through an empty `impl`, and `omnia_test::guest::Provider` answers all nine natively, so the root compiles and tests on both sides as it stands.

## The stem's manifest

`crates/<stem>/Cargo.toml` is written by the first slice to lay the stem, its `[dependencies]` the list above, holding that slice's lines; a slice that finds it in the tree adds its lines there and changes nothing else. The dev-dependencies are the doubles every test uses and are never edited: `omnia_test::guest` stands in for every mocking crate.

```toml
[package]
name = "app-orders"
version = "0.1.0"
edition = "2024"
publish = false

[target.'cfg(not(target_arch = "wasm32"))'.dev-dependencies]
omnia-test.workspace = true
serde_json.workspace = true
tokio.workspace = true
tower.workspace = true

# One line per crate the stem's files use after the four below, each added at the end.
[dependencies]
axum.workspace = true
omnia-sdk.workspace = true
serde.workspace = true
serde_json.workspace = true
```

## Merging

The engine merges each slice's commit into the integrated tree under these rules, in this order:

| Path | Rule | Why |
| --- | --- | --- |
| `Cargo.lock` | the integrated side kept | the next cargo run regenerates it |
| `Cargo.toml`, the root's and every stem's | both sides' lines kept | each ends in a list of independent lines |
| `src/lib.rs`, the root's | both sides' lines kept | its router list |
| `crates/*/src/lib.rs` | both sides' lines kept | a stem's `pub mod` list, or one slice's module |
| everything else | a conflict | the slice is rebuilt over the merged tree |

A path one slice alone changed takes that slice's change under no rule, so two slices that keep to their own modules never conflict. A stem built whole has its module at `crates/<stem>/src/lib.rs`, under the list rule, which is why only its one slice writes it. The rules keep lines; they do not reconcile edits: a slice that changes a pinned version, a static line of a root file, or another slice's module is the one conflict the layout leaves possible, and it is the slice's to avoid.
