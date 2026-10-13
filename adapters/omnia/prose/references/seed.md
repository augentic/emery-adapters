# Seed

Every file the adapter lays in a tree that lacks it before the first slice's turn, byte for byte: the [omnia exemplar](https://github.com/augentic/omnia-exemplar)'s guest template (`templates/guest/manifest.yaml` at commit `31d89ab45c61c30f9df76ad6e11ca46982b3d239`) rendered for the package `guest`, with the two root lists, the two root rungs, the local host, and the `common` crate the [layout](layout.md) adds beside it. The verify holds every file outside the lists to these fences: a file that differs is put back as it reads here, through `write_files`, whatever the edit was for. A slice reads the layout for where its lines go and this document only to put a static file back.

Each fence below is one file, named by its path from the tree's root.

## `Cargo.toml`

The root package `guest` and the workspace: the pins, the lints, the `host` feature, and the two lists that close it.

```toml
[package]
name = "guest"
description = "Omnia WASI guest"
publish.workspace = true
edition.workspace = true
rust-version.workspace = true
version.workspace = true

[lints]
workspace = true

[lib]
# `rlib` lets the native rungs under `tests/` link the routers; `cdylib` is the WASI component.
crate-type = ["cdylib", "rlib"]

[features]
# The local host: `cargo run --features host --example runtime -- run target/wasm32-wasip2/release/guest.wasm`.
host = [
  "dep:omnia",
  "dep:omnia-wasi-blobstore",
  "dep:omnia-wasi-config",
  "dep:omnia-wasi-docstore",
  "dep:omnia-wasi-http",
  "dep:omnia-wasi-identity",
  "dep:omnia-wasi-keyvalue",
  "dep:omnia-wasi-messaging",
  "dep:omnia-wasi-otel",
  "dep:omnia-wasi-sql",
  "dep:omnia-wasi-websocket",
]

[[example]]
name = "runtime"
required-features = ["host"]

[target.'cfg(target_arch = "wasm32")'.dependencies]
omnia-wasi-messaging.workspace = true
wasip3.workspace = true

[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
omnia = { workspace = true, optional = true }
omnia-wasi-blobstore = { workspace = true, optional = true }
omnia-wasi-config = { workspace = true, optional = true }
omnia-wasi-docstore = { workspace = true, optional = true }
omnia-wasi-http = { workspace = true, optional = true }
omnia-wasi-identity = { workspace = true, optional = true }
omnia-wasi-keyvalue = { workspace = true, optional = true }
omnia-wasi-messaging = { workspace = true, optional = true }
omnia-wasi-otel = { workspace = true, optional = true }
omnia-wasi-sql = { workspace = true, optional = true }
omnia-wasi-websocket = { workspace = true, optional = true }

# Target-gated so the guest's wasm32 dependency graph never sees the doubles.
[target.'cfg(not(target_arch = "wasm32"))'.dev-dependencies]
omnia-test = { workspace = true, features = ["guest"] }
tokio.workspace = true
tower.workspace = true

[workspace]
members = ["crates/*"]
resolver = "3"

[workspace.package]
edition = "2024"
publish = false
rust-version = "1.99"
version = "0.1.0"

[workspace.lints.rust]
meta_variable_misuse = "warn"
missing_docs = "warn"
trivial_numeric_casts = "warn"
unsafe_code = "deny"
unsafe_op_in_unsafe_fn = "warn"
unused_extern_crates = "warn"

[workspace.lints.clippy]
all = { level = "warn", priority = -1 }
cargo = { level = "warn", priority = -1 }
nursery = { level = "warn", priority = -1 }
pedantic = { level = "warn", priority = -1 }

# cherry-picked `restriction`s
allow_attributes_without_reason = "warn"
assertions_on_result_states = "warn"
dbg_macro = "warn"
disallowed_script_idents = "warn"
empty_drop = "warn"
fn_to_numeric_cast_any = "warn"
map_err_ignore = "warn"
print_stderr = "warn"
print_stdout = "warn"
unused_result_ok = "warn"

[workspace.dependencies]
anyhow = "1.0.104"
axum = { version = "0.8.9", default-features = false, features = ["json"] }
bytes = "1.12.1"
chrono = { version = "0.4.45", features = ["serde"] }
http = "1.5.0"
http-body-util = "0.1.5"
serde = { version = "1.0.229", features = ["derive"] }
serde_json = "1.0.151"
thiserror = "2.0.20"
tokio = { version = "1.53.1", default-features = false, features = ["macros", "rt"] }
tower = { version = "0.5.3", default-features = false, features = ["util"] }
tracing = "0.1.44"
uuid = { version = "1.24.0", features = ["serde", "v4"] }
wasip3 = { version = "0.9.0", features = ["http-compat"] }

# Every `omnia-*` line below moves together: bump them to the same release in
# one change, and keep the shared pins above at the versions omnia itself pins
# so the guest links one copy of each.
omnia = "0.36.0"
omnia-sdk = "0.36.0"
omnia-test = "0.36.0"
omnia-wasi-blobstore = "0.36.0"
omnia-wasi-config = "0.36.0"
omnia-wasi-docstore = "0.36.0"
omnia-wasi-http = "0.36.0"
omnia-wasi-identity = { version = "0.36.0", default-features = false }
omnia-wasi-keyvalue = "0.36.0"
omnia-wasi-messaging = "0.36.0"
omnia-wasi-otel = "0.36.0"
omnia-wasi-sql = "0.36.0"
omnia-wasi-websocket = "0.36.0"

# The crates of this tree: `common`, then one line per stem, each added at the end.
common = { path = "crates/common" }

# What the guest wires: the four below, then one line per stem, each added at the end.
[dependencies]
axum.workspace = true
common.workspace = true
omnia-sdk.workspace = true
tracing.workspace = true
```

## `src/lib.rs`

The guest: the provider, the HTTP and messaging exports, `OWNER`, and the `Capabilities` bound. No list; never edited.

```rust
//! # guest
//!
//! An Omnia WASI guest: every stem's HTTP routes on one router and every
//! stem's topics on one messaging router, over one provider. Only the WASI
//! exports are `wasm32`-gated: the routers are generic over their provider,
//! so `tests/routes.rs` and `tests/messaging.rs` drive the production
//! tables natively under `omnia_test::guest::Provider` doubles.

mod messaging;
mod routes;

use omnia_sdk::{
    BlobStore, Broadcast, Config, DocumentStore, HttpRequest, Identity, Publish, StateStore,
    TableStore,
};

pub use self::messaging::messaging_router;
pub use self::routes::router;

/// The tenant that owns this deployment.
pub const OWNER: &str = "guest";

/// Every capability a stem may bound its provider on.
///
/// Both routers take any provider under this bound, so a stem reaching a
/// capability no stem reached before changes neither router's signature.
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

/// Bare provider backed by the default WASI capability implementations.
///
/// Each capability trait ships its WASI-backed methods as defaults, so an
/// empty `impl` per capability is the whole production wiring.
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

/// WASI HTTP export.
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

/// WASI Messaging export.
#[cfg(target_arch = "wasm32")]
pub struct Messaging;
#[cfg(target_arch = "wasm32")]
omnia_wasi_messaging::export!(Messaging with_types_in omnia_wasi_messaging);

#[cfg(target_arch = "wasm32")]
impl omnia_wasi_messaging::incoming_handler::Guest for Messaging {
    async fn handle(
        message: omnia_wasi_messaging::types::Message,
    ) -> Result<(), omnia_wasi_messaging::types::Error> {
        let router = messaging_router(Provider).await;
        omnia_sdk::api::messaging::handle(&router, message).await
    }
}
```

## `src/routes.rs`

The route list.

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

## `src/messaging.rs`

The topic list.

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

## `tests/routes.rs`

The route rung: `ROUTES` is the list.

```rust
//! Route rung: the production HTTP routing table driven natively.
//!
//! `omnia_test::guest::Provider` implements every capability, and `oneshot`
//! exercises the same `axum::Router` the WASI export serves, so a route
//! regression surfaces here without a component build. `dispatch` checks
//! wiring, not semantics: every `(method, path)` the router registers must
//! reach its handler. Business logic is covered by each stem's own tests.

use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode};
use common::config;
use omnia_test::guest::{MapConfig, Provider, ScriptedTables};
use tower::ServiceExt as _;

/// Every `(method, path)` the router registers, one entry per route of
/// `src/routes.rs`, over the constants of `common::routes::http` spelled in full.
const ROUTES: &[(Method, &str)] = &[
    // one entry per route, each added at the end
];

/// Default doubles with the one config key every handler may read.
///
/// `ScriptedTables` panics on an unscripted statement, so SQL is answered
/// with no rows and no affected rows: table-backed routes reach their
/// handler and answer for themselves instead of aborting the request.
fn provider() -> Provider {
    Provider::default()
        .config(MapConfig::default().with([(config::ENV, "test")]))
        .tables(ScriptedTables::default().on_query(|_, _| true, Vec::new()).on_exec(|_, _| true, 0))
}

/// Replace each `{param}` segment of a route pattern with a concrete id.
fn concrete(path: &str) -> String {
    path.split('/')
        .map(|segment| if segment.starts_with('{') { "1" } else { segment })
        .collect::<Vec<_>>()
        .join("/")
}

/// Whether the router, rather than a handler, produced this response.
///
/// axum answers an unregistered path with an empty `404` and a registered
/// path with the wrong method with `405`. A handler's own `not_found!`
/// carries its message in the body, so it counts as dispatched.
fn router_miss(status: StatusCode, body: &[u8]) -> bool {
    status == StatusCode::METHOD_NOT_ALLOWED || (status == StatusCode::NOT_FOUND && body.is_empty())
}

#[tokio::test]
async fn dispatch() {
    let provider = provider();
    let mut misses = Vec::new();
    for (method, pattern) in ROUTES {
        let path = concrete(pattern);
        let request = Request::builder()
            .method(method.clone())
            .uri(&path)
            .body(Body::empty())
            .expect("request");
        let response = guest::router(provider.clone()).oneshot(request).await.expect("response");
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX).await.expect("body");
        if router_miss(status, &body) {
            misses.push(format!("{method} {path} -> {status}"));
        }
    }
    assert!(misses.is_empty(), "routes not dispatched:\n{}", misses.join("\n"));
}

#[tokio::test]
async fn an_unknown_path_is_not_found() {
    let request = Request::get("/nothing-here").body(Body::empty()).expect("request");

    let response = guest::router(provider()).oneshot(request).await.expect("response");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
```

## `tests/messaging.rs`

The messaging rung: `TOPICS` is the list.

```rust
//! Messaging rung: the production topic table driven natively.
//!
//! `messaging_router` is a guest-side value, so one delivery per subscribed
//! topic goes straight to `Router::handle` under `omnia_test::guest::Provider`
//! doubles. `routed` checks wiring, not semantics: every topic of the table
//! is registered under the environment's prefix. Business logic is covered
//! by each stem's own tests.

use common::config;
use omnia_sdk::api::messaging::{Delivery, DeliveryError};
use omnia_test::guest::{MapConfig, Provider};

/// The environment the doubles answer `ENV` with.
const ENV: &str = "test";

/// Every topic suffix the messaging router registers, one entry per topic
/// of `src/messaging.rs`, over the constants of `common::routes::topic` spelled in full.
const TOPICS: &[&str] = &[
    // one entry per topic, each added at the end
];

fn provider() -> Provider {
    Provider::default().config(MapConfig::default().with([(config::ENV, ENV)]))
}

fn delivery(topic: String) -> Delivery {
    Delivery {
        topic: Some(topic),
        ..Delivery::default()
    }
}

#[tokio::test]
async fn routed() {
    let router = guest::messaging_router(provider()).await;
    let mut unrouted = Vec::new();
    for suffix in TOPICS {
        let topic = config::topic_for(ENV, suffix);
        let outcome = router.handle(delivery(topic.clone())).await;
        if matches!(outcome, Err(DeliveryError::UnhandledTopic(_))) {
            unrouted.push(topic);
        }
    }
    assert!(unrouted.is_empty(), "topics not routed:\n{}", unrouted.join("\n"));
}

#[tokio::test]
async fn another_environment_is_unhandled() {
    let router = guest::messaging_router(provider()).await;
    for suffix in TOPICS {
        let topic = config::topic_for("other", suffix);
        let outcome = router.handle(delivery(topic.clone())).await;
        assert!(matches!(outcome, Err(DeliveryError::UnhandledTopic(_))), "`{topic}` was consumed");
    }
}
```

## `examples/runtime.rs`

The local host, behind the `host` feature. Never edited.

````rust
//! Host runtime for the WASM guest: WASI's default backends, SQLite in
//! memory for tables and a fixed token for identity.
//!
//! Build the guest for `wasm32-wasip2` and run it with:
//!
//! ```shell
//! cargo build --target wasm32-wasip2 --release
//! cargo run --features host --example runtime -- run target/wasm32-wasip2/release/guest.wasm
//! ```

cfg_select! {
    not(target_arch = "wasm32") => {
        use omnia_wasi_blobstore::{BlobstoreDefault, WasiBlobstore};
        use omnia_wasi_config::{ConfigDefault, WasiConfig};
        use omnia_wasi_docstore::{DocStoreDefault, WasiDocStore};
        use omnia_wasi_http::{HttpDefault, WasiHttp};
        use omnia_wasi_identity::{IdentityStub, WasiIdentity};
        use omnia_wasi_keyvalue::{KeyValueDefault, WasiKeyValue};
        use omnia_wasi_messaging::{MessagingDefault, WasiMessaging};
        use omnia_wasi_otel::{OtelDefault, WasiOtel};
        use omnia_wasi_sql::{SqlDefault, WasiSql};
        use omnia_wasi_websocket::{WasiWebSocket, WebSocketDefault};

        omnia::runtime!({
            hosts: {
                WasiBlobstore: BlobstoreDefault,
                WasiConfig: ConfigDefault,
                WasiDocStore: DocStoreDefault,
                WasiHttp: HttpDefault,
                WasiIdentity: IdentityStub,
                WasiKeyValue: KeyValueDefault,
                WasiMessaging: MessagingDefault,
                WasiOtel: OtelDefault,
                WasiSql: SqlDefault,
                WasiWebSocket: WebSocketDefault,
            }
        });
    }
    _ => {
        fn main() {}
    }
}
````

## `crates/common/Cargo.toml`

The shared crate's manifest. Never edited.

```toml
[package]
name = "common"
description = "The route and topic tables and the configuration catalog every stem shares"
publish.workspace = true
edition.workspace = true
rust-version.workspace = true
version.workspace = true

[lints]
workspace = true

[dependencies]
omnia-sdk.workspace = true
tracing.workspace = true
```

## `crates/common/src/lib.rs`

The shared crate's root. Never edited.

```rust
//! # Common
//!
//! What every stem shares: the canonical route and topic tables, and the
//! catalog of configuration keys with the helpers that qualify a topic for
//! the deployment's environment.

pub mod config;
pub mod routes;
```

## `crates/common/src/routes.rs`

The tables' module. Never edited.

```rust
//! Canonical HTTP routes and messaging topics.
//!
//! The root-package guest builds its HTTP and messaging routers from these
//! tables, and a stem publishes to the same topic constant its consumer
//! subscribes to, so producers and consumers agree on the surface by
//! construction rather than by review.

pub mod http;
pub mod topic;
```

## `crates/common/src/routes/http.rs`

The path table: the list is the whole file after its `//!`.

```rust
//! HTTP ingress paths served by the guest: one constant per route, each added at the end.
```

## `crates/common/src/routes/topic.rs`

The topic table: the list is the whole file after its `//!`.

```rust
//! Environment-agnostic topic suffixes: one constant per topic, each added at the end.
//!
//! Prefix a suffix with `{env}-` through [`crate::config::topic`] or
//! [`crate::config::topic_for`] before publishing or subscribing.
```

## `crates/common/src/config.rs`

The configuration catalog: the list is the constants after `ENV`; the helpers below them are never edited.

```rust
//! Configuration keys and helpers.
//!
//! Every configuration key the guest reads is declared here, so there is one
//! catalog of what a deployment must provide, and every topic is qualified
//! for the deployment's environment through [`topic`] or [`topic_for`].

use omnia_sdk::Config;

/// Deployment environment (`dev`, `prod`). Prefixes every messaging topic;
/// see [`topic`]. One constant per key after this one, each added at the end
/// of the list, before [`env`].
pub const ENV: &str = "ENV";

/// Resolve the deployment environment.
///
/// Falls back to `dev`, with a warning, when [`ENV`] is not configured, so
/// the guest runs out of the box; a deployment sets `ENV` explicitly, and the
/// warning makes a missing key visible instead of silently mis-prefixing
/// every topic.
pub async fn env(provider: &impl Config) -> String {
    match Config::get(provider, ENV).await {
        Ok(env) => env,
        Err(error) => {
            tracing::warn!("`ENV` is not configured, defaulting to `dev`: {error}");
            "dev".to_string()
        }
    }
}

/// Build a fully-qualified messaging topic from an environment-agnostic
/// suffix (see [`crate::routes::topic`]): `realtime-orders.v1` becomes
/// `dev-realtime-orders.v1`.
pub async fn topic(provider: &impl Config, suffix: &str) -> String {
    topic_for(&env(provider).await, suffix)
}

/// Build a fully-qualified messaging topic from an already-resolved
/// environment. Prefer this over [`topic`] when qualifying several topics so
/// the environment is only resolved once.
#[must_use]
pub fn topic_for(env: &str, suffix: &str) -> String {
    format!("{env}-{suffix}")
}
```

## `rust-toolchain.toml`

The toolchain: stable, with clippy, rustfmt, and `rust-src`, for `wasm32-wasip2`.

```toml
[toolchain]
channel = "stable"
components = ["clippy", "rust-src", "rustfmt"]
targets = ["wasm32-wasip2"]
```

## `rustfmt.toml`

The formatter's configuration; its unstable options take nightly `rustfmt`.

```toml
# https://github.com/rust-lang/rustfmt/blob/master/Configurations.md
# https://rust-lang.github.io/rustfmt

max_width = 100
use_small_heuristics = "Max"

fn_params_layout = "Compressed"
format_code_in_doc_comments = true
format_macro_matchers = true
group_imports = "StdExternalCrate"
imports_granularity = "Module"
reorder_impl_items = true
struct_lit_width = 20
unstable_features = true
use_field_init_shorthand = true
# wrap_comments = true
# comment_width = 80
```

## `taplo.toml`

The TOML formatter's configuration.

```toml
[formatting]
allowed_blank_lines = 1
array_auto_collapse = false
array_auto_expand = false
column_width = 100
reorder_arrays = false
reorder_keys = false

[[rule]]
include = ["**/Cargo.toml", "Cargo.toml"]
keys = ["*-dependencies", "*.dependencies", "dependencies"]

[rule.formatting]
reorder_arrays = true
reorder_keys = true
```

## `clippy.toml`

The lint configuration: two lists, `doc-valid-idents` and `allowed-duplicate-crates`, extended by a line where a check asks for it.

```toml
# https://doc.rust-lang.org/stable/clippy/index.html

doc-valid-idents = [
  # Add project-specific identifiers here (e.g. "GTFS", "OpenLR")
  "OpenTelemetry",
  "WebAssembly",
  "WebSocket",
  "WebSockets",
]

allowed-duplicate-crates = [
  # Populated by running `cargo clippy` and adding false positives.
  # This baseline covers a fresh Omnia guest workspace's transitive tree.
  "axum",
  "axum-core",
  "base64",
  "block-buffer",
  "const-oid",
  "core-foundation",
  "cpufeatures",
  "crypto-common",
  "darling",
  "darling_core",
  "darling_macro",
  "digest",
  "embedded-io",
  "foldhash",
  "getrandom",
  "hashbrown",
  "indexmap",
  "io-lifetimes",
  "itertools",
  "linux-raw-sys",
  "matchit",
  "quick-xml",
  "r-efi",
  "rand",
  "rand_chacha",
  "rand_core",
  "reqwest",
  "rustix",
  "rustls",
  "rustls-native-certs",
  "rustls-webpki",
  "sha2",
  "socket2",
  "syn",
  "thiserror",
  "thiserror-impl",
  "tokio-rustls",
  "wasi",
  "wasip3",
  "wasm-encoder",
  "wasm-metadata",
  "wasmparser",
  "webpki-roots",
  "windows-link",
  "windows-result",
  "windows-strings",
  "windows-sys",
  "windows-targets",
  "windows_aarch64_gnullvm",
  "windows_aarch64_msvc",
  "windows_i686_gnu",
  "windows_i686_gnullvm",
  "windows_i686_msvc",
  "windows_x86_64_gnu",
  "windows_x86_64_gnullvm",
  "windows_x86_64_msvc",
  "wit-bindgen",
  "wit-bindgen-core",
  "wit-bindgen-rust",
  "wit-bindgen-rust-macro",
  "wit-component",
  "wit-parser",
]
```

## `deny.toml`

The dependency policy `cargo deny` reads; the operator's.

```toml
# https://embarkstudios.github.io/cargo-deny

[graph]
all-features = true
targets = ["aarch64-apple-darwin", "wasm32-wasip2"]

[output]
feature-depth = 1

[advisories]
unmaintained = 'workspace'
ignore = [
]

[licenses]
allow = [
  "Apache-2.0 WITH LLVM-exception",
  "Apache-2.0",
  "BSD-2-Clause",
  "BSD-3-Clause",
  "BSL-1.0",
  "CC0-1.0",
  "CDLA-Permissive-2.0",
  "ISC",
  "MIT",
  "MIT-0",
  "OpenSSL",
  "Unicode-3.0",
  "Zlib",
]

[licenses.private]
ignore = true

[[licenses.clarify]]
name = "ring"
expression = "MIT AND ISC AND OpenSSL"
license-files = [{ path = "LICENSE", hash = 0xbd0eed23 }]

[sources]
allow-git = [
  "https://github.com/augentic/test.git",
]
private = [
  "sparse+https://pkgs.dev.azure.com/augentic/_packaging/augentic/Cargo/index/",
]

[bans]
deny = [
  { name = "tokio", deny-multiple-versions = true },
]
multiple-versions = "allow"
skip = []
skip-tree = []
wildcards = "allow"

[bans.workspace-dependencies]
duplicates = "deny"
include-path-dependencies = true
unused = "warn"
```

## `.gitignore`

The ignore rules.

```
target/
.env
**/*.rs.bk
rustc-ice-*
.DS_Store
```

## `Makefile`

The task runner's entry, forwarding to mise.

```make
# Convenience pass-through to mise: `make <task> [args]` runs `mise run <task> -- [args]`.
# mise is never installed implicitly; see https://mise.jdx.dev/getting-started.html
MISE := $(shell command -v mise 2>/dev/null)

.PHONY: %
%:
	@if [ "$@" = "$(firstword $(MAKECMDGOALS))" ]; then \
		if [ -z "$(MISE)" ]; then \
			echo "mise not found on PATH; install it first: https://mise.jdx.dev/getting-started.html" >&2; \
			exit 1; \
		fi; \
		"$(MISE)" run "$@" -- $(wordlist 2,$(words $(MAKECMDGOALS)),$(MAKECMDGOALS)); \
	fi
```

## `mise.toml`

The tasks, included from the toolkit at its pinned tag.

```toml
[task_config]
includes = ["git::https://github.com/augentic/toolkit.git//mise/rust.toml?ref=v0.6.0"]

# No `[env]` knobs: every workspace member is dual-target — the root guest
# lib, the `crates/*` handler libs, the std-only `template-check` gate, and
# the `cfg_select!`-gated `examples/runtime.rs` host (an empty `main` on wasm32) —
# so the shared `lint` covers the whole workspace, and `outdated`
# ignores nothing. Integration tests are host-only and never in that scope.
#
# This file is an `exact` entry of the guest template contract
# (`templates/guest/manifest.yaml`): keep it token-free and generic to any
# Omnia guest workspace; exemplar-only tasks do not belong here.

# -------------------------------------
# Guest (project-specific; the generic Rust tasks live in the include)
# -------------------------------------

# Release build of the guest component (and every other workspace lib/bin);
# the shared `lint` already clippies the same scope for wasm32-wasip2 in CI.
[tasks.wasm]
description = "Release-build the workspace for wasm32-wasip2"
run = "cargo build --locked --workspace --all-features --target wasm32-wasip2 --release"
```

## `.vscode/settings.json`

The editor's settings.

```json
{
    "rust-analyzer.linkedProjects": [
        "Cargo.toml"
    ],
    "rust-analyzer.check.command": "clippy",
    "rust-analyzer.cargo.cfgs": [
        "!miri"
    ],
    "rust-analyzer.cargo.target": "wasm32-wasip2"
}
```

## `supply-chain/README.md`

How the operator regenerates the vet state.

````markdown
# Cargo Vet

Following a Cargo dependency update, run:

```bash
cargo vet regenerate imports
cargo vet regenerate exemptions
cargo vet regenerate unpublished
```

to update the vetted dependencies based on trusted authors.

See the [Cargo Vet book](https://mozilla.github.io/cargo-vet/commands.html) for
more information.
````

## `supply-chain/config.toml`

The `cargo vet` configuration: the imports, with the exemptions left for the operator to regenerate.

```toml
# cargo-vet config file

[cargo-vet]
version = "0.10"

[imports.bytecode-alliance]
url = "https://raw.githubusercontent.com/bytecodealliance/wasmtime/main/supply-chain/audits.toml"

[imports.embark-studios]
url = "https://raw.githubusercontent.com/EmbarkStudios/rust-ecosystem/main/audits.toml"

[imports.google]
url = "https://raw.githubusercontent.com/google/supply-chain/main/audits.toml"

[imports.isrg]
url = "https://raw.githubusercontent.com/divviup/libprio-rs/main/supply-chain/audits.toml"

[imports.mozilla]
url = "https://raw.githubusercontent.com/mozilla/supply-chain/main/audits.toml"

[imports.zcash]
url = "https://raw.githubusercontent.com/zcash/rust-ecosystem/main/supply-chain/audits.toml"

# [policy.<crate-name>] and [[exemptions.<crate-name>]] entries are
# workspace-specific. They are populated by running:
#
#   cargo vet regenerate exemptions
#
# after workspace dependencies are finalized.
```

## `supply-chain/audits.toml`

The `cargo vet` audits, empty at the start.

```toml
# cargo-vet audits file

[audits]
```

## `.github/dependabot.yml`

Dependabot over the actions.

```yaml
version: 2
updates:
  - package-ecosystem: github-actions
    directory: /
    schedule:
      interval: weekly
    groups:
      actions:
        patterns:
          - "*"
    ignore:
      # The toolkit pin is Renovate's: one pull request over every `uses:`
      # and the mise `?ref=` together (renovate.json).
      - dependency-name: "augentic/*"
```

## `.github/workflows/audit.yaml`

The daily advisory audit.

```yaml
name: Audit

# Daily advisory-database audit and outdated-dependency check of the
# workspace, apart from the per-push CI gate so a fresh upstream advisory
# never blocks an unrelated change.

on:
  schedule:
    - cron: "0 0 * * *"
  workflow_dispatch:

concurrency:
  group: ${{ github.workflow }}
  cancel-in-progress: true

permissions:
  contents: read
  issues: write
  checks: write

jobs:
  audit:
    uses: augentic/toolkit/.github/workflows/audit.yaml@v0.6.0
```

## `.github/workflows/ci.yaml`

CI over every push, through the toolkit.

```yaml
name: CI

on:
  push:

concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: ${{ github.ref != 'refs/heads/main' }}

permissions:
  contents: read

jobs:
  ci:
    uses: augentic/toolkit/.github/workflows/ci.yaml@v0.6.0
    secrets: inherit
    with:
      targets: wasm32-wasip2
```

## `.github/workflows/patch.yaml`

The patch-release workflow.

```yaml
name: Create Patch

# Bump the patch version on a durable release-X.Y.Z line (Cargo version and
# RELEASES.md). Dispatch from the release-* branch carrying the fix; Publish
# Release, dispatched from the same branch, tags and releases it.

on:
  workflow_dispatch:

concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: false

permissions:
  contents: write
  copilot-requests: write

jobs:
  patch:
    uses: augentic/toolkit/.github/workflows/patch.yaml@v0.6.0
    secrets: inherit
```

## `.github/workflows/release.yaml`

The release-cut workflow.

```yaml
name: Create Release

# Cut a durable release-X.Y.Z line from main: push release-<version> at the
# current tip, then open a pull request that bumps main to the next unreleased
# version and resets RELEASES.md.

on:
  workflow_dispatch:
    inputs:
      increment:
        description: The version bump for main after the cut; while on 0.x, minor marks a breaking change
        type: choice
        options: [minor, patch, major]
        default: minor

concurrency:
  group: ${{ github.workflow }}
  cancel-in-progress: false

permissions:
  contents: write
  pull-requests: write
  copilot-requests: write

jobs:
  release:
    uses: augentic/toolkit/.github/workflows/release.yaml@v0.6.0
    with:
      increment: ${{ inputs.increment }}
    secrets: inherit
```

## `.github/workflows/publish.yaml`

The publish-and-deploy workflow: `<STORAGE_ACCOUNT>` and `<RESOURCE_GROUP>` stay for the operator to fill.

```yaml
name: Publish Release

on:
  workflow_dispatch:

jobs:
  skip:
    name: Skipped
    if: ${{ !startsWith(github.ref_name, 'release-') }}
    runs-on: ubuntu-latest
    steps:
      - run: |
          echo "This workflow only runs on the 'release-*' branches"
          exit 1

  ci:
    name: CI
    if: ${{ startsWith(github.ref_name, 'release-') }}
    uses: augentic/toolkit/.github/workflows/ci.yaml@v0.5.0
    secrets: inherit
    with:
      targets: wasm32-wasip2

  publish:
    name: Publish
    needs: ci
    uses: augentic/toolkit/.github/workflows/publish.yaml@v0.5.0

  deploy:
    name: Deploy
    needs: publish
    uses: augentic/toolkit/.github/workflows/wasm.yaml@v0.5.0
    secrets: inherit
    with:
      package: guest
      storage-account: <STORAGE_ACCOUNT>
      resource-group: <RESOURCE_GROUP>
```
