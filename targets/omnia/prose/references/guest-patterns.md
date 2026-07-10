# Operation Routers and Guest Exports

Domain crates define typed, stateless `Operation<P>` implementations. The guest owns provider construction, transport routers, projection policy, and explicit WIT exports.

## HTTP

```rust
#![cfg(target_arch = "wasm32")]

use domain::{CreateUser, GetUser};
use omnia_guest::api::http::{self, Router, get, post};
use omnia_guest::api::invoke::Invoker;
use omnia_guest::wasip3;

struct HttpGuest;
wasip3::http::service::export!(HttpGuest);

impl wasip3::exports::http::handler::Guest for HttpGuest {
    #[omnia_wasi_otel::instrument(name = "http_guest_handle")]
    async fn handle(
        request: wasip3::http::types::Request,
    ) -> Result<wasip3::http::types::Response, wasip3::http::types::ErrorCode> {
        let router = Router::new(Invoker::new("owner", Provider::new()))
            .route("/users/{user_id}", get::<GetUser, Provider>())
            .route("/users", post::<CreateUser, Provider>());
        http::serve(router, request).await
    }
}
```

Use Axum 0.8 `{param}` path syntax. GET path/query names and POST JSON/path names must match the typed input's serde field names. The default JSON projector returns status 200; define one `http::Projector<O, P>` at the transport boundary for custom status, headers, bytes, or error envelopes.

## Messaging

```rust
use domain::ProcessOrder;
use omnia_guest::api::invoke::Invoker;
use omnia_guest::api::messaging::{self, Router, consume};
use omnia_wasi_messaging::incoming_handler::Guest;
use omnia_wasi_messaging::types::{Error, Message};

struct MessagingGuest;
omnia_wasi_messaging::export!(MessagingGuest with_types_in omnia_wasi_messaging);

impl Guest for MessagingGuest {
    #[omnia_wasi_otel::instrument(name = "messaging_guest_handle")]
    async fn handle(message: Message) -> Result<(), Error> {
        let router = Router::new(Invoker::new("owner", Provider::new()))
            .route("orders.created.v1", consume::<ProcessOrder>());
        messaging::handle(&router, message).await
    }
}
```

Register exact topics. Use a custom decoder for XML/binary payloads and a custom messaging projector for retry/rejection policy. Never silently acknowledge missing, malformed, unhandled, or failed deliveries.

## WebSocket

WebSocket WIT exports remain explicit. Adapt the event into a typed `Invocation`, call the shared `Invoker`, and map the plain output or typed error to the WebSocket result. Keep event decoding and response projection at this boundary.

```rust
struct WebSocketGuest;
omnia_wasi_websocket::export!(WebSocketGuest);
```

## Command

For a WASI command surface, assemble an `omnia_guest::api::command::Router` over the same `Invoker`, register typed `run::<Args, Operation>()` routes, provide an output/error projector, and export `wasip3::cli::command::export!(CliGuest)`.

## Checklist

1. One zero-sized operation type per use case; typed input and plain output.
2. Exact provider bounds remain on each `Operation<P>` implementation.
3. Structural validation starts `call`; contextual validation follows context loading.
4. HTTP and messaging routing use the typed Omnia routers.
5. Serialization, status, acknowledgement, and retry policy live in projectors.
6. Every WIT transport has an explicit export and a thin delegation implementation.
7. Router inventory tests prove each route/topic points at the intended operation.
