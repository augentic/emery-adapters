# Behaviour

What a `requirement` claim captures from a handler, and what commonly goes missing. Every behavioural fact a caller observes through the surface is one `requirement`: one present-tense sentence, its `path` at the span that exhibits it, backed by an `excerpt` of the span and a `call` for each thing the span reaches.

## Read each handler for

- **Inputs and validation.** Which fields are read, which are required, and what happens when one is missing or malformed: the check, the response or error, and whether the handler stops there. Each validation a caller can trip is a requirement (`user-registration.email-validation`). A security check is a validation like any other — path-traversal rejection, input sanitisation, an authorisation check on the caller — and is claimed the same way, however much it looks like plumbing.
- **Branches and early returns.** Every branch the caller can observe — a `return` inside an `if`, a `switch` arm, a ternary that changes the response — is its own requirement or is stated in the one it belongs to. A branch left out is a behaviour the specification does not have.
- **Errors.** What is caught and what propagates; what is returned, thrown, or logged; whether execution continues. `Retries up to 3 times with exponential backoff (100ms, 200ms, 400ms), then propagates` is behaviour; `handles errors` is not. The extent of a `try` block is behaviour too: a cache write inside it fails the way the fetch beside it fails. Granularity too: in a loop, whether one item's failure ends the whole operation or is recorded and the remaining items proceed.
- **Side effects.** Every write, publish, delete, and cache set, with its key or topic as the code constructs it and the condition under which it happens (`only when data.needsAudit is true`). A side effect that completes before the caller gets a response and one that completes after are different behaviours.
- **Sequencing.** Whether calls run in sequence or in parallel (`Promise.all`), and what the caller sees on partial failure.
- **Timing.** Delays and their placement — `sleep 5s, then publish all, repeated 2 times` is not `publish, sleep 5s, publish` — intervals, timeouts, and TTLs, with the exact values the source spells.
- **Loops.** Whether a count is fixed (`for (let i = 0; i < 2; i++)` publishes 2 times) or runtime-dependent (`for (const item of items)`), and whether the payload changes between iterations.
- **Transformations.** What a `map`, `filter`, or `reduce` keeps, drops, or computes, where the result is what the caller gets.
- **Configuration.** Every `process.env` key and constant the behaviour depends on, spelled exactly as the source spells it (`CC_STATIC_URL`, never a tidier name), with its default (`process.env.TIMEOUT ?? "5000"`) and whether the handler fails without it. When a constant is a lookup table filtered at runtime (`ACTIVE_CATEGORIES.includes(id)`), the requirement states the filter and the active subset, not the whole table.

## Data access

Name the store and the operation the way the source does: the entity or table and its key columns for a database or managed table store (`SELECT TripsChanges WHERE (trip_id, start_date, start_time)`), the key pattern and operation for a cache (`get data:{id}; on miss fetch and set with TTL 3600s`), the topic and payload for a publish. One `call` per access site, `callee` the client method; the requirement it backs states what the caller observes (`A cache miss fetches from the API and caches the result for 3600 seconds.`). A managed table store reached over HTTP by its SDK (`@azure/data-tables`, `TableClient`) is a data store, not an outbound API call — see [External services and publications](services.md).

## Shared modules

A repository, validator, or client that several surfaces reach is claimed for what this surface does with it, under this surface's noun: `orders.persistence`, not `repository.insert`. Two handlers calling one upstream API are two behaviours, each with its own request construction — format strings for generated ids, fields set to null, values that differ by branch — even where they look alike. So are two helpers with similar names (`checkOk`, `checkOkAccount`): each is read for its own error paths, and neither is claimed as behaving like the other.

## Stating a requirement

- One sentence, present tense, about the system as the caller sees it: `Registration rejects an email that is not RFC-5322 valid with a 400 response.`
- Precise values, as the source has them: the status code, the limit, the duration, the key.
- Nothing the code does not exhibit. A handler that does not enforce uniqueness has no uniqueness requirement; the engine renders the gap, you do not fill it.
- A `criterion` only for an explicit boundary the source encodes — a threshold constant, a schema constraint, a validation pattern — with an id that extends its requirement's.
