# Transport Routers

The guest does not define per-route business handlers. It assembles typed HTTP, messaging, WebSocket, or command routers over domain `Operation<P>` types.

## HTTP

Use `omnia_guest::api::http::Router` with `get`, `post`, `get_with`, and `post_with`. Input decoding is transport-owned; outputs are plain domain values. Default JSON projection is suitable only for 200 JSON responses. Add a named `Projector<O, P>` for status codes, headers, binary/text bodies, or custom error envelopes.

## Messaging

Use `omnia_guest::api::messaging::Router` and exact topic registrations with `consume::<O>()`. The default JSON decoder and acknowledgement projector reject malformed inputs and operation errors. Define custom decoder/projector policy explicitly when needed.

## WebSocket and command

WebSocket adapters construct typed invocations and project results at the boundary. WASI command guests use typed `omnia_guest::api::command` routes and a command projector; they do not parse and dispatch raw argv manually.

## Exports

Use explicit `wasip3::http::service::export!`, `omnia_wasi_messaging::export!`, `omnia_wasi_websocket::export!`, and `wasip3::cli::command::export!` declarations for the transports the component exposes.

See [guest-patterns.md](guest-patterns.md), [sdk-api.md](sdk-api.md), and [guest-wiring.md](guest-wiring.md).
