# Omnia build — crate writer

Loaded by [../build.md](../build.md) phase 2. Reads `specs/<domain>/spec.md` + `design.md`, writes `$CRATE_PATH`. Sequenced after [guidance.md](../guidance.md) (idiom guidance already folded into spec + design by core synthesis).

## Authority hierarchy

The full Hard Rules + Authority Hierarchy live in [`../../references/hard-rules.md`](../../references/hard-rules.md). The summary below is a load-bearing extract; the full reference governs ties.

1. **Emery artifacts are ground truth.** `specs/<domain>/spec.md` and `design.md` outrank inferred behaviour. If artifacts conflict with source, trust the artifacts.
2. **Apply update categories in fixed order**: structural → subtractive → modifying → additive. Type renames propagate first, dead code is removed before any new code is added, additive code depends on the already-updated type system.
3. **Idempotency is non-negotiable.** If a section of an existing crate already matches the artifacts, do nothing.
4. **No `unwrap()` / `expect()` in production code.** Tests may unwrap.
5. **Provider trait selection follows [`guidance.md`](../guidance.md) and [`capabilities.md`](../../references/capabilities.md).** Every external I/O point in `design.md` resolves to a provider trait; see [`capability-mapping.md`](../../references/capability-mapping.md) for the artifact-to-trait mapping rules.
6. **WASM guardrails are absolute** — see [`guardrails.md`](../../references/guardrails.md) and [`wasm-constraints.md`](../../references/wasm-constraints.md). Forbidden crates and forbidden std APIs never appear in generated code.
7. **Never write tests in this step.** Tests belong to the [test writer](test.md) pass.
8. **Re-scan inventory after every structural change** before proceeding to subtractive / modifying / additive categories. See [`change-classification.md`](../../references/change-classification.md) for how to classify artifact-vs-code differences.

## Critical path

1. Read [guidance.md](../guidance.md) refresher and the slice's `specs/<domain>/spec.md` + `design.md` + `tasks.md`.
2. **Build the three cross-cutting matrices** per [`cross-cutting-matrices.md`](../../references/cross-cutting-matrices.md): Side-Effect, Outbound-Message, Transaction-Boundary. Every cell must land in code.
3. **Mode dispatch.** Inherited from the build prompt: create mode (no `Cargo.toml`) vs update mode.
4. Apply the per-mode process below; in update mode walk the four categories in fixed order.
5. Run the inventory re-scan after every structural change.
6. Continue with the build prompt's verify-repair loop.

## Create mode

1. Author the workspace `Cargo.toml` and the crate `Cargo.toml` per [`cargo-toml.md`](../../references/cargo-toml.md). Workspace dependencies pin `omnia-guest` plus the `omnia-wasi-*` adapters for the provider traits the design declares, at the Omnia version the exemplar checkout's `exemplar.yaml` declares — mirror its `[patch.crates-io]` block per [`exemplar.md`](../../references/exemplar.md).
2. Generate `$CRATE_PATH/src/lib.rs` (domain library — not the guest) with one module per operation. Module layout follows the exemplar convention (see `crates/tally-connector`, `crates/pulse-adapter`): handler/operation modules, types, errors as needed. Guest wiring is the workspace root package (`src/lib.rs`; create-mode guest writer).
3. For each use case, emit:
   - A zero-sized operation type implementing `Operation<P>` with a typed request DTO as `Input`, a plain domain value as `Output`, and the exact provider capability bounds from the design. The transport router, not the operation, deserializes bytes/path/query fields.
   - `Operation::call(input, CallContext)` with structural validation as its first step, followed by context loading and temporal/contextual validation, then delegation to a standalone business function where that improves readability.
   - No transport serialization behavior on domain types. HTTP status, headers, body encoding, and error envelopes belong in `http::Projector<O, P>` implementations at the guest boundary; messaging acknowledgement/retry policy belongs in messaging projectors.
4. Emit a domain error enum via `thiserror`, plus `impl From<DomainError> for omnia_guest::Error` mapping each variant to the right `BadRequest` / `NotFound` / `ServerError` / `BadGateway` constructor with stable `code` strings. See [`error-handling.md`](../../references/error-handling.md) for the macros, domain enums, context patterns, and troubleshooting.
5. Author the provider trait bundle: an `AppProvider` trait that composes the per-operation trait bounds, plus a `Provider` type in the guest wrapper; on wasm32 the capability traits use their WASI-backed default methods.
6. Apply [`guardrails.md`](../../references/guardrails.md) serde, timestamp, and DST rules verbatim: `#[serde(rename_all = "camelCase")]` on output types, `#[serde(skip_serializing_if = "Option::is_none")]` on optional fields, `#[serde(default)]` + `#[serde(rename(deserialize = …))]` on input-only types, `.earliest()` (not `.single()`) for DST-safe local-time conversion, `received_at = Utc::now()` semantics.
7. Honour TODO markers, adapter overrides, and cache-aside patterns per [`todo-markers.md`](../../references/todo-markers.md).
8. Emit accompanying output documents (`Migration.md`, `Architecture.md`, `CHANGELOG.md`, `.env.example`) per [`output-documents.md`](../../references/output-documents.md).

Worked code: the exemplar checkout is the primary reference for compiling current-SDK crate shapes — `crates/tally-connector` (minimal), `crates/pulse-adapter` (compact adapter), `crates/gtfs-adapter` (full-size, stateful); navigation map in [`exemplar.md`](../../references/exemplar.md); capability operations and mocks in `crates/capability-examples/`. Retained explanatory walkthrough: [`examples/crates/anti-patterns.md`](../../references/examples/crates/anti-patterns.md) for shapes to avoid.

## Update mode

Walk the four categories in fixed order; re-scan after structural before proceeding. The strategy library per category lives at [`update-patterns.md`](../../references/update-patterns.md); diff classification rules live at [`change-classification.md`](../../references/change-classification.md).

1. **Structural** — type renames, file moves, module reshuffles. Apply via small, semantics-preserving rewrites. Re-run `cargo check` before moving on. Worked example: [`examples/crates/updates/structural.md`](../../references/examples/crates/updates/structural.md).
2. **Subtractive** — delete operations / fields / types the new artifacts no longer name and remove their HTTP route or exact messaging-topic registration from the guest. Worked example: [`examples/crates/updates/subtractive.md`](../../references/examples/crates/updates/subtractive.md).
3. **Modifying** — change an operation's behaviour, output shape, validation rules, provider dependencies, or guest boundary mapping in place. Update the matching `Cargo.toml` adapter dependency if a new provider trait is consumed. Worked example: [`examples/crates/updates/modifying.md`](../../references/examples/crates/updates/modifying.md).
4. **Additive** — add new operations, guest routes/topics, types, and variants. Additive code MUST compile against the already-updated structural layer. Worked example: [`examples/crates/updates/additive.md`](../../references/examples/crates/updates/additive.md).

When a `cargo` failure surfaces during this pass, apply minimum-change repair via [`repair-patterns.md`](../../references/repair-patterns.md) before re-entering the parent verify-repair loop.

## Outputs and quality checklist

The full checklist lives at [`checklists.md`](../../references/checklists.md). Highlights:

- Every operation in `design.md` has a matching zero-sized `Operation<P>` implementation in `$CRATE_PATH`.
- Every external surface is registered in the root guest (`src/lib.rs` — Axum routes and/or exact messaging topics) and exported through its WIT interface.
- Every provider trait bound on an operation appears in the `AppProvider` composition.
- Every `Config::get` key in `design.md` has a matching read in the operation (or in `Provider::new`).
- Every `omnia_guest::Error` mapping in `design.md` has a matching arm in `impl From<DomainError>`.
- No forbidden crate or forbidden std API per [`guardrails.md`](../../references/guardrails.md).
- `cargo fmt`, `cargo check`, `cargo clippy -- -D warnings` all pass before entering the build prompt's verify-repair loop.
