# Omnia target — guidance prompt

> This document is **prose guidance only**, returned verbatim by the adapter's `guidance` operation. Core synthesis (the refine phase's pipeline) consumes the guidance below when authoring `proposal.md`, `specs/<domain>/spec.md`, and `design.md` for a slice whose `Slice.target == omnia`. It does **not** read sources or write artifacts — it carries idioms the synthesiser folds into the canonical artifacts. The same guidance applies whether the slice's evidence is pure intent, documentation, code, or any combination; per eval scenario #5h, both fixtures pick up identical `guidance`.

## Omnia domains

For Omnia targets, each `## Domains` entry in `proposal.md` maps to one spec file at `specs/<domain>/spec.md`. The domain slug normally equals the crate name for a single generated crate; for broader work (multi-crate service or migration), the domain is the service surface slug.

The workflow owns the `## Domains` section and the `specs/<domain>/spec.md` layout. This guidance defines what a domain *means* for Omnia but does not rename or relocate the core sections.

## How synthesis consumes this guidance

When the synthesising agent assembles a slice for an Omnia target it MUST:

1. Read this guidance first, ahead of any source-supplied `Evidence`.
2. Lift the deep references listed at the bottom into the synthesis context (they are normative, not optional).
3. Author `proposal.md`, `specs/<domain>/spec.md`, `design.md`, and `tasks.md` so the artifacts match the §Required artifact shapes below, regardless of which sources contributed evidence.
4. Carry tag-and-proceed posture on uncertainty: `[unknown]`, `[conflict]`, `[divergence]` are review signals (see [`../references/guardrails.md`](../references/guardrails.md) for the per-trait coverage matrix the spec lists each operation against). This guidance never asks synthesis to halt.

## Idiom: provider-based dependency injection

Omnia crates are stateless WASM components. All host-side I/O — config, HTTP, messaging, state, identity, blob storage, document storage, table storage, broadcast — flows through trait-bounded *providers*. The crate declares the traits it consumes as generic bounds on an operation; the guest supplies a `Provider` value; on wasm32 the capability traits use their WASI-backed default methods.

When synthesising `design.md` for an Omnia slice:

- The **Domain model** section MUST enumerate which Omnia provider traits each operation depends on, by name. The closed set is: `Config`, `HttpRequest`, `Publish`, `StateStore`, `Identity`, `TableStore`, `Broadcast`, `BlobStore`, `DocumentStore`.
- The **APIs / Integrations** section MUST list every external surface and bind it to a typed operation plus transport route/projector.
- The **Configuration** section MUST enumerate every `Config::get` key each operation reads, and reflect it in `.env.example`.
- The **Technical logic** section MUST describe a zero-sized `Operation<P>`, typed `Input`, plain `Output`, exact provider bounds, and static `call(input, CallContext)`. The guest owns deserialization and projection.

## Idiom: WASM-Preview-2 guardrails

All generated code targets `wasm32-wasip2`. The forbidden surface is normative — synthesised specs and `design.md` MUST NOT prescribe any APIs from the table below. Forbidden crates include `reqwest`, `tokio` (as runtime; dev-deps OK), `redis`, `sqlx`, `diesel`, `mongodb`, `hyper`, `dotenv` / `dotenvy`, `rand`, `uuid`, `std::process`, `lazy_static`. Forbidden std APIs include `std::env::var`, `std::fs::*`, `std::net::*`, `std::process::*`, `std::thread::spawn`. The replacements (provider traits, `Config::get`, `StateStore`, `BlobStore`, `DocumentStore`, `HttpRequest::fetch`) live in [`../references/guardrails.md`](../references/guardrails.md).

Statelessness: WASM components are fully stateless. Synthesised design MUST NOT prescribe `static mut`, `OnceCell::new`, or any mutable global. `std::sync::LazyLock` is allowed only for immutable compile-time lookup tables.

When sources surface behaviour that requires a forbidden API (e.g. legacy code reads files from disk), synthesis MUST translate it into the equivalent Omnia provider trait in `design.md`. Record the translation as inline commentary if the operator needs to verify the mapping.

## Idiom: error-variant conventions

Omnia errors converge on `omnia_guest::Error`, which has four variants — `BadRequest`, `NotFound`, `ServerError`, `BadGateway`. Synthesised specs and design MUST:

- Map every operator-facing error condition to one of these four variants.
- Use the macro form (`bad_request!`, `server_error!`, `bad_gateway!`) in `design.md` examples; method-style constructors like `Error::bad_request("...")` do **not** exist.
- For domain-internal errors, prescribe a `thiserror` enum that converts via `From<DomainError> for omnia_guest::Error`. The conversion MUST set both `code` (stable string identifier) and `description` (human-readable detail).
- Forbid `unwrap()` / `expect()` in production paths. Tests may unwrap. Use `anyhow::Context` for chained errors with downcast to `omnia_guest::Error`.

## Idiom: validation placement (edge vs core)

Synthesis MUST partition validation into two buckets in `design.md`:

- **Structural validation belongs in the start of `Operation::call()`.** Checks that depend only on the parsed shape: field presence, format (regex, email), range checks against constants, type conversions.
- **Temporal or contextual validation follows context loading inside `Operation::call()`.** Checks that compare against runtime state or time: timestamp freshness via `Utc::now()`, idempotency lookups against `StateStore`, business-rule validation that requires a provider call.

The synthesised design MUST NOT prescribe `Utc::now()` inside the start of `Operation::call()` — message replay tests rely on `shift_time` which cannot rewrite parse-time clocks.

## Idiom: code-quality and observability defaults

- No `println!`, `dbg!`, `eprintln!` in production paths — `tracing::debug!` / `tracing::info!`.
- No `unsafe` blocks.
- Functions stay below ~50 lines; if synthesis writes a operation specification implying a longer body, split into helper functions.
- Public types and functions carry doc comments (synthesis prescribes the doc shape in `design.md`; the writer enforces it at build time).
- Operational metrics use `tracing::info!(monotonic_counter.x = 1)` / `tracing::info!(gauge.y = …)` (OpenTelemetry-compatible names). The synthesised `design.md` MUST name the metrics each operation emits.

## Required artifact shapes (Omnia target)

### `specs/<domain>/spec.md`

Per-requirement provenance lines (`ID:`, `Sources:`, `Status:`) are core's responsibility (workflow §Requirement block contract). On top of that, an Omnia slice's spec files MUST cover:

- One requirement block per operation-observable behaviour (HTTP request → response shape, message topic → operation effect, WebSocket event → server-side reaction). The block names the trigger and the observable outcome; provider mechanics belong in `design.md`.
- Acceptance scenarios that name **inputs**, **provider state preconditions** (`StateStore` keys present / absent, `DocumentStore` rows, `Config` keys), and **observable outcomes** (response body, published events, state writes, status code).
- Error conditions enumerated per `omnia_guest::Error` variant the operation returns (`BadRequest` for validation, `NotFound` for missing lookups, `BadGateway` for upstream failure, `ServerError` for unexpected state).

### `design.md`

Synthesis writes the following headings, in order:

1. **Domain model** — types, IDs, enums. Newtypes for IDs; no raw primitives for domain concepts.
2. **Provider trait dependencies** — exhaustive list per operation, drawn from the §Idiom: provider-based DI rules above.
3. **Operation kernel** — for each use case: zero-sized operation type, typed input, plain output, exact provider bounds, and structural-before-contextual validation sequence.
4. **Transport surfaces** — typed HTTP route and projector, exact messaging topic and decoder/projector, WebSocket adapter, or typed command route; include explicit WIT exports.
5. **Configuration** — `Config::get` keys, defaults, identity OAuth keys when `Identity` is consumed (`IDENTITY_CLIENT_ID`, `IDENTITY_CLIENT_SECRET`, `IDENTITY_TOKEN_URL` — see [`../references/runtime.md`](../references/runtime.md)).
6. **Error mapping** — domain-error enum, `From<DomainError> for omnia_guest::Error`, per-variant code/description rules.
7. **Validation placement** — table of checks with the edge-vs-core column populated per §Idiom: validation placement.
8. **Observability** — operation-level metric names and tracing spans.

### `tasks.md`

Sequence:

1. Author / update crate per `design.md`.
2. Author / update tests per `specs/<domain>/spec.md` (scenarios) + `design.md` (side-effect assertions).
3. Author / update guest HTTP / messaging wiring, explicit WIT exports, and provider impls (Axum + exact topics; typed routers only when design requires the fallback).
4. Run code review.

The build prompt carries the detailed writer instructions; tasks.md should follow that ordering so the build walks the slice the same way every time.

## References

- [`../references/guardrails.md`](../references/guardrails.md) — Forbidden crates / std APIs, statelessness rules, serde idioms, timestamp semantics.
- [`../references/capabilities.md`](../references/capabilities.md) — Provider trait signatures and adapter triggers.
- [`../references/runtime.md`](../references/runtime.md) — `omnia::runtime!` macro, WASI host options, `.env.example` shape, identity env-var contract.
- [`../references/guest-patterns.md`](../references/guest-patterns.md) — HTTP / Messaging / WebSocket guest export patterns.
- [`../references/guest-wiring.md`](../references/guest-wiring.md) — Crate → guest injection contract.
- [`../references/providers/README.md`](../references/providers/README.md) — Per-trait selection notes, composition semantics, and the exemplar demonstration map.
- [`../rules/`](../rules/) — Stable Omnia rules (classified errors, provider-only host access, host-managed secrets, WASM runtime constraints).
