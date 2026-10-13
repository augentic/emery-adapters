# Guardrails

What a module of the [layout](layout.md) never does, and what it does instead. The program is a WebAssembly component: it holds no process, no file system, no socket, and no clock of its own, and it lives one request or one delivery at a time. Every effect goes through a capability of the provider, as the [handlers](handlers.md) reach them.

## The provider is the host

| A module never | Because | It reaches instead |
| --- | --- | --- |
| reads or writes a file (`std::fs`, `include_str!` of data, a path from the request) | the component has no file system | `BlobStore` for objects, `DocumentStore` for JSON documents |
| opens a socket or builds an HTTP client (`std::net`, `reqwest`, `hyper`, `ureq`) | the component opens no connection of its own | `HttpRequest::fetch` with an `http::Request` |
| reads an environment variable or a `.env` file (`std::env`, `dotenvy`) | the deployment's settings are the host's to hand over | `Config::get` under a key of `common::config` |
| connects to a database (`sqlx`, `postgres`, `rusqlite`, `mongodb`) | the connection is the host's | `TableStore` for SQL, `DocumentStore` for documents |
| builds a cache or a map that outlives the request (`static`, `OnceLock`, `lazy_static`, `Mutex<HashMap>`) | each request may run in a fresh instance; nothing in memory survives it | `StateStore` for state, `cas` or `increment` where two requests race |
| speaks to a broker or a queue (`rdkafka`, `lapin`, `redis`), or subscribes to a topic itself | the broker is the host's, and a subscription is a statement in the topic list | `Publish::send` to a topic of `common::routes::topic`; a handler in the topic list to consume one |
| pushes to a WebSocket of its own | the sockets are the host's | `Broadcast::send` |
| exchanges credentials for a token (an OAuth client, a signing key) | the identity is the host's | `Identity::access_token` |
| spawns a thread or a task, or runs a timer (`std::thread`, `tokio::spawn`, `tokio::time`, `std::time::Instant` for a deadline) | a request runs to its answer and no further; `tokio` is a dev-dependency for the tests alone | nothing: a background job is a route or a topic the host calls |
| spawns a process or handles a signal | there is no process | nothing |
| tells the time through `std::time::SystemTime` | the design's timestamps are `chrono` types, serialised as RFC 3339 | `chrono::Utc::now()` |
| adds a crate for ids or randomness | the pinned crate reaches the host's entropy | `uuid::Uuid::new_v4()` for an id |
| logs to stdout or stderr (`println!`, `eprintln!`, `dbg!`) | the host collects traces, not a stream | `tracing::info!` and siblings, which `omnia_sdk` re-exports |

A crate compiles for `wasm32-wasip2` when it is pure Rust and reaches nothing in the left column: `serde`, `serde_json`, `chrono`, `uuid`, and `thiserror` are pinned in the root manifest for that reason. A crate with a C dependency (`openssl`, `native-tls`, `rusqlite`), one that reaches the system (`reqwest`, `tokio` as a runtime), or one that needs a build script to probe the host fails the guest check of the loop, and the module that named it is the one to change.

## The host is not simulated

The source a specification was read from may have been a process: a server that listened, a worker that polled a queue, a scheduler that ticked, a client that connected at start-up and reconnected on loss, a shutdown that drained. The component is none of these, and the host does each for it, so a requirement that states one is satisfied by the host or by nothing in the tree, never by code that imitates the mechanism:

- No `main`, no `start`, `run`, `listen`, `serve`, `bootstrap`, or `shutdown` function, and no struct holding a `started`, `running`, `connected`, or `ready` flag: there is no process to be in a state, and the host instantiates the component for each request.
- No loop that polls, no subscription set up in a handler, no reconnection, no heartbeat, no graceful stop: a topic is consumed through the topic list, and a connection is the host's to keep.
- No route laid to stand for a lifecycle — `GET /ready`, `POST /start`, a probe that reports a flag — unless a requirement states the route itself; a route answering a requirement that states none is a surface the specification does not hold.
- A requirement of this kind is covered where the host satisfies it as stated — a request is answered, a delivery is consumed — and left out of `covered` where the host satisfies it by needing nothing, never reported covered over a stub. The report says what the tree holds.

The `start` stem's slice lays what is code under it, a health route a requirement states or a rule every handler calls, and the seed's entry, provider, and routers are the rest of `start`; it edits none of them, and it writes no module to stand for the program starting.

## Errors

- A handler answers a classified error or `Ok`, never a panic: no `unwrap`, `expect`, `panic!`, `todo!`, `unreachable!`, slice or map indexing on request data, or arithmetic that can overflow on it. A panic is a trapped instance and a `500` the program did not choose.
- The class is the requirement's: a request the specification refuses is `bad_request!`, a thing it says does not exist is `not_found!`, an upstream that fails is `bad_gateway!`, and `server_error!` is what the program itself cannot do — a stored value that no longer parses, a serialisation that fails.
- A failure the handler cannot classify is carried with `?` as it is, never swallowed into a default, an empty list, or an `Ok`.
- `unsafe` has no place in a module, and a module declares no `build.rs`, no procedural macro, and no `#[no_mangle]`; the workspace denies `unsafe_code`.

## Secrets

- No credential is written into the tree: no key, token, password, connection string, or `.env` file, and no test fixture holding one. A value the program needs is a `Config::get` at the moment of use, under a key the configuration catalog declares and names for what it is (`RATES_URL`, `PAYMENTS_API_KEY`), and a test hands a placeholder through `MapConfig`.
- Nothing of what a handler reads from the host is logged or answered back unless the requirement asks for it: a token fetched to call an upstream goes into that request's header and nowhere else.

## The files a slice never edits

- The seed outside its lists, as the [layout](layout.md) states them: `src/lib.rs`, `examples/runtime.rs`, `crates/common/Cargo.toml`, `crates/common/src/lib.rs`, `crates/common/src/routes.rs`, the helpers of `crates/common/src/config.rs`, the toolchain and tooling files, `supply-chain/`, and `.github/`; the static text of the root `Cargo.toml`, the two root lists, the two root rungs, `clippy.toml`, and a stem's manifest around their lists, the pins, the `host` feature, the target tables, and the dev-dependencies among it. A pinned version is the tree's, and a crate the pins lack is named with its version in the stem's manifest alone, never as a `git` or `registry` source.
- `Cargo.lock`, which cargo writes.
- Another slice's module or test file, and anything under `.emery/` or `target/`.
