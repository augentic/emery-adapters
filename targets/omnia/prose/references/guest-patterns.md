# Guest HTTP / messaging wiring

Domain crates define typed, stateless `Operation<P>` implementations. The guest owns provider construction, transport wiring, projection policy, and explicit WIT exports.

**Preferred compiling reference:** the exemplar checkout's `src/lib.rs` — hand-written Axum routes via `omnia_wasi_http::serve`, exact-topic messaging via `omnia_wasi_messaging::export!`, `#[omnia_wasi_otel::instrument]` on entry handlers, and a shared `Invoker` for each operation call. Navigation: [`exemplar.md`](exemplar.md). API semantics: [`sdk-api.md`](sdk-api.md). Wiring a new crate in: [`guest-wiring.md`](guest-wiring.md).

## HTTP (preferred — Axum)

Use Axum 0.8 `{param}` path syntax. Build an `axum::Router`, register handlers that decode the transport payload and call `invoker().invoke::<Op>(Invocation::new(…)).await`, then serve with `omnia_wasi_http::serve`. Export HTTP with `wasip3::http::service::export!`.

JSON handlers typically take `Json<T>` / return `HttpResult<Json<U>>`. Non-JSON ingress (bytes / XML) decodes in the handler before invoking the operation — match the exemplar's Pulse XML route.

## Messaging (preferred — exact topic)

Export messaging with `omnia_wasi_messaging::export!`. Match on the **exact** env-qualified topic (no substring matching). Decode the payload in the handler, invoke through the shared `Invoker`, and map failures to the messaging error type. Never silently acknowledge missing, malformed, unhandled, or failed deliveries.

## WebSocket

WebSocket WIT exports remain explicit (`omnia_wasi_websocket::export!`). Adapt the event into a typed `Invocation`, call the shared `Invoker`, and map the plain output or typed error to the WebSocket result. Keep event decoding and response projection at this boundary.

## Command

For a WASI command surface, assemble an `omnia_guest::api::command::RouterBuilder` over a `clap::Command` root and the same `Invoker`, register typed `run::<Args, Operation>()` routes, provide an output/error projector, and `build()` into the executable `command::Router`; export `wasip3::cli::command::export!(CliGuest)`.

## Fallback — typed routers

Use only when `design.md` explicitly requires the typed `omnia_guest::api` HTTP / messaging routers (or when updating a consumer that already uses them). The exemplar does **not** ship a compiling typed guest; do not invent a `guests/typed/` package to chase this style.

When falling back:

- `omnia_guest::api::http::Router` with `get::<Op, P>()` / `post::<Op, P>()`
- `omnia_guest::api::messaging::Router` with `consume::<Op>()`, plus `decode_with` for non-JSON payloads
- Custom HTTP status/body/error policy in `http::Projector<O, P>` at the transport boundary
- Non-JSON HTTP ingress may drop to `router.into_axum()` for a single hand-written route

## Checklist

1. One zero-sized operation type per use case; typed input and plain output.
2. Exact provider bounds remain on each `Operation<P>` implementation.
3. Structural validation starts `call`; contextual validation follows context loading.
4. HTTP and messaging wiring match the preferred Axum + exact-topic shape (or the typed-router fallback when design requires it).
5. Serialization, status, acknowledgement, and retry policy live at the guest boundary.
6. Every WIT transport has an explicit export and a thin delegation implementation.
7. Router / topic inventory stays aligned with a shared route catalog when the workspace has one.
