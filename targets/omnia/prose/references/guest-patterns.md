# Operation Routers and Guest Exports

Domain crates define typed, stateless `Operation<P>` implementations. The guest owns provider construction, transport routers, projection policy, and explicit WIT exports.

Compiling reference: the exemplar checkout's `guests/typed/src/lib.rs` shows every pattern below — HTTP and messaging guest structs, `#[omnia_wasi_otel::instrument]` on the handle functions, router assembly over one shared `Invoker`, and the per-transport export macros. Navigation: [`exemplar.md`](exemplar.md). API semantics: [`sdk-api.md`](sdk-api.md). Wiring a new crate in: [`guest-wiring.md`](guest-wiring.md).

## HTTP

Use Axum 0.8 `{param}` path syntax. GET path/query names and POST JSON/path names must match the typed input's serde field names. The default JSON projector returns status 200; define one `http::Projector<O, P>` at the transport boundary for custom status, headers, bytes, or error envelopes.

## Messaging

Register exact topics (no substring matching). Use a custom decoder for XML/binary payloads and a custom messaging projector for retry/rejection policy. Never silently acknowledge missing, malformed, unhandled, or failed deliveries.

## WebSocket

WebSocket WIT exports remain explicit (`omnia_wasi_websocket::export!`). Adapt the event into a typed `Invocation`, call the shared `Invoker`, and map the plain output or typed error to the WebSocket result. Keep event decoding and response projection at this boundary.

## Command

For a WASI command surface, assemble an `omnia_guest::api::command::RouterBuilder` over a `clap::Command` root and the same `Invoker`, register typed `run::<Args, Operation>()` routes, provide an output/error projector, and `build()` into the executable `command::Router`; export `wasip3::cli::command::export!(CliGuest)`.

## Checklist

1. One zero-sized operation type per use case; typed input and plain output.
2. Exact provider bounds remain on each `Operation<P>` implementation.
3. Structural validation starts `call`; contextual validation follows context loading.
4. HTTP and messaging routing use the typed Omnia routers.
5. Serialization, status, acknowledgement, and retry policy live in projectors.
6. Every WIT transport has an explicit export and a thin delegation implementation.
7. Router inventory tests prove each route/topic points at the intended operation.
