# Omnia Operation API Reference

The generated domain surface is the transport-neutral operation kernel from `omnia_guest::api`. Transport adapters decode wire input, invoke an operation, and project its plain output or typed error. Compiling usage lives in the exemplar checkout ([`exemplar.md`](exemplar.md)) — preferred guest wiring is hand-written Axum + exact-topic messaging in `src/lib.rs`; the typed HTTP/messaging routers below are the documented fallback. This document keeps only the contract semantics.

## Operation kernel

- One zero-sized type per use case implements `Operation<P>` (`omnia_guest::api::operation`) with `type Input` (typed DTO), `type Output` (plain domain value), and `type Error = omnia_guest::Error`.
- `Operation::call(input, context)` is static — never construct a stateful operation object.
- Preserve exact provider capability bounds on each implementation: the bound is the union of the traits `call` and its callees actually use.
- Structural validation is the first statement of `call`; checks requiring time, configuration, identity, or persisted state run only after that context is loaded.

## Invocation

`Invoker<P>` (`omnia_guest::api::invoke`) owns one provider and supplies the owner plus transport-neutral metadata; operations receive a `CallContext` carrying `owner`, `provider`, and `metadata`. Correlation IDs are transport metadata, not domain input fields.

## HTTP router and projectors (typed-router fallback)

Prefer Axum handlers in the root guest (see [`guest-patterns.md`](guest-patterns.md)). When `design.md` requires the typed-router fallback: `omnia_guest::api::http::Router` binds typed operations. `get` decodes path/query fields into `O::Input`; `post` merges path fields into a JSON object and deserializes it into `O::Input`. The default `Json` projector serializes plain outputs and maps operation errors through `HttpError` — suitable only for 200 JSON responses. Use `get_with` / `post_with` and implement `http::Projector<O, P>` when status, headers, non-JSON bytes, or a custom error envelope differ. Serialization belongs to the projector, never the domain output type.

## Messaging router and projectors (typed-router fallback)

Prefer exact-topic matching in the root guest's messaging export. When using the typed-router fallback: `omnia_guest::api::messaging::Router` routes exact topics to operations via `consume::<O>()`. The default decoder deserializes JSON; the default `Acknowledge` projector acknowledges outputs and rejects decode or operation errors. Use `decode_with` and `project_with` for another payload format or retry/rejection policy. Missing and unhandled topics are errors.

## Command router

When a component exports WASI command, assemble `omnia_guest::api::command::RouterBuilder::new(clap_command, invoker)` and `build()` it into the executable `command::Router`. Parse CLI args into the operation input, register `run::<Args, Operation>()`, and project `Outcome::{Output, Operation, Decode}` into stdout, stderr, and exit status. Do not hand-match argument vectors or call domain functions around the operation kernel.

## Explicit exports

Each exposed WIT transport gets its own export declaration (`wasip3::http::service::export!`, `omnia_wasi_messaging::export!`, `omnia_wasi_websocket::export!`, `wasip3::cli::command::export!`). Each implementation wires that transport (Axum + `Invoker` in the preferred shape, or a typed router in the fallback) and delegates to the transport adapter. There is no aggregate guest macro.
