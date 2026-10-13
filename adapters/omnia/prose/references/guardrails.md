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

## What `start` becomes

The `start` stem is where a specification read from a process keeps what that process did at start-up: read its settings, build its clients, connect, listen, wire its monitoring, catch its signals. Each of those is one row below, and the row says what the `start` slice lays for it and how its report reads. The seed's entry, provider, and routers are the rest of `start`; the slice edits none of them, and it writes no module to stand for the program starting.

| The source's `start` did | The slice lays | The requirement is |
| --- | --- | --- |
| read its configuration — environment variables, a settings file, a config object handed to every component | one `pub const` per setting after `ENV` in `crates/common/src/config.rs`, named for what it is, read by the handler that uses it through `Config::get` at the moment of use; no settings struct, no load at start | covered by the catalog and the reads |
| held a credential or a secret retriever — a vault client, a key-vault reader, a token exchanged at start and refreshed | nothing of its own: the handler that needs the value reads it through `Config::get` or `Identity::access_token` where it uses it; no cache, no refresh, no watcher | covered where a handler reads it; left uncovered where the requirement states the retriever, the cache, or the refresh itself |
| constructed a client to an upstream and handed it on — an API client, a gateway, a store connection | nothing under `start`: the stem that calls the upstream owns the call, as a function over `HttpRequest::fetch` or the store capability, constructed nowhere | that stem's; `start` covers nothing of it |
| listened, bound a port, served, connected to a broker, subscribed | nothing: the host answers HTTP and deliveries through the two root lists | covered, by the seed and the host, as stated |
| initialised monitoring — an APM agent, a metrics exporter, a logger, a tracer | nothing: the host collects what `tracing` emits | covered, where the requirement states that the program is observed; left uncovered where it states the agent or the exporter |
| handled a signal, drained, stopped | nothing | left uncovered |
| ran a periodic job — a poller, a scheduler, a cron | nothing, unless a requirement states a route or a topic the host calls for it, which is that requirement's handler | left uncovered otherwise |
| declared a lifecycle type — a `Main`, an `App` with a `started` flag, a `Server` struct with `start` and `stop` | nothing: the type is not laid, whatever the design calls it | left uncovered |
| answered a health or readiness route a requirement states by method and path | that route, in `crates/start/`, as any stem's | covered |
| enforced a rule every route obeys — authentication, a body limit, a request log | a `pub fn` the module exports, called at the top of the handlers it guards through the `start` crate | covered, with the handlers' tests |

So the `start` slice writes `crates/start/src/<part>.rs` and `crates/start/tests/<part>.rs` for the last two rows, the constants of the first in the catalog, and its lines in the lists, and nothing for any other row: a `crates/start/` holding a client, a connection, a loop, a scheduler, or a struct that starts and stops is a module the verify refuses, however its tests pass. A `start` slice whose requirements are all of the kind the host satisfies writes its catalog constants and reports `covered` for the rows that say so, with no module at all.

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
