# Operation and Router Repair Patterns

## Test-failure classification

When `cargo test` fails inside the build prompt's `## § Verify-repair loop`, classify each failure:

| Failure signal | Classification | Fix action |
|---|---|---|
| Error in `tests/` paths, `MockProvider`, or `provider.rs` | Test issue | Re-enter the test-writer prompt (`prompts/build/test.md`) |
| Error in `src/` paths, missing trait impls in production | Code issue | Re-enter the crate-writer prompt (`prompts/build/crate.md`) |
| Assertion mismatch where *actual* matches spec | Test issue | Test expectation is stale |
| Assertion mismatch where *expected* matches spec | Code issue | Handler returns the wrong result |
| MockProvider missing a trait impl the handler now requires | Test issue | Update MockProvider |
| Unresolved import or missing crate in `Cargo.toml` | Workspace issue | Fix `Cargo.toml` paths or workspace member list directly |

Group failures by classification and re-enter each writer prompt once with all same-class errors.

## Update-mode regression check

Before iteration 1 of the verify-repair loop in update mode, record the baseline: `cd $CRATE_PATH && cargo test 2>&1 | tee /tmp/${SLICE_NAME}-${CRATE_NAME}-baseline.txt`. After each iteration, for each test that passed before and now fails: if the spec explicitly changes the asserted behaviour → expected behavioural change, re-enter the test writer to align expectations; if the spec does not change the asserted behaviour → true regression, route the fix through the classification table above.

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
