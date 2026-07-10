# Operation and Router Repair Patterns

## Operation shape

Repair stateful or transport-coupled business code into a zero-sized `Operation<P>` with typed input, plain output, typed error, and static `call(input, CallContext)`. Preserve the narrow union of provider capability bounds used by the operation and its helpers.

At the start of `call`, run every structural check that depends only on input. Then load configuration, identity, time, or persisted state and run contextual validation. Never move contextual validation ahead of context loading.

## HTTP

- Replace raw Axum business handlers with `api::http::Router` registrations.
- Let GET decode path/query fields and POST decode JSON plus path fields into the operation input.
- Move body/status/header/error-envelope logic into a `Projector<O, P>`.
- Return plain operation outputs; do not add domain serialization traits.

## Messaging

- Replace topic `match` dispatch with exact `api::messaging::Router` routes.
- Use `consume::<O>()` for JSON plus acknowledgement.
- Add `decode_with` for another wire format and `project_with` for retry/rejection policy.
- Missing, duplicate, or unhandled topics must not succeed silently.

## Command and exports

- Use typed `omnia_guest::api::command` routes for command surfaces.
- Export HTTP, messaging, WebSocket, and command WIT interfaces explicitly.
- Keep guest implementations thin: construct provider/invoker/router and delegate.

## Tests

Invoke operations through `Invoker` for domain integration tests. Exercise HTTP and messaging routers for decoding, route inventory, projector, and unknown-route behavior. Test explicit guest exports in component-level checks.
