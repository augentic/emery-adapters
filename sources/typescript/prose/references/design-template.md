# Design coverage for extraction

The typescript source adapter emits **Evidence claims only** — it never writes `spec.md` or `design.md`. Those artifacts are authored downstream by the engine's synthesis from every bound source's Evidence. This reference lists the design surface that synthesis will need to fill, so an extraction pass knows what its claims must cover for reconstruction-grade generation: every section below that the source exhibits should be backed by claims (`type`, `contract`, `call`, `region`, `container`, `requirement`, …) with verbatim payloads or path anchors.

## THINK: coverage self-check before emitting Evidence

Before answering with the Evidence document, verify the claims cover:

1. ALL entry points and handlers
2. ALL external API calls with complete request/response shapes
3. ALL config keys and constants exactly as written
4. ALL business logic, tagged appropriately
5. ALL type definitions with complete nested structures
6. ALL optional fields, wire-format names, and custom converters
7. ALL error handling patterns
8. ALL metrics, message metadata, and timing patterns
9. Any `[unknown]` areas worth flagging in claim synopses
10. Enough detail for reconstruction-grade code generation downstream
11. Guest/entry-point layer behaviors
12. Dependency versions from the lock file

### Check for common omissions

- [ ] Config keys captured verbatim (not renamed for clarity)
- [ ] Active subsets identified for filtered lookup tables
- [ ] Wire-format field names for all serialized types
- [ ] Custom converter behavior documented
- [ ] Field optionality marked for all input types
- [ ] Complete output schemas (including fields not populated by this component)
- [ ] Message partition keys and custom headers
- [ ] Metrics with emission points and labels
- [ ] Retry patterns and timeout values
- [ ] Concurrent vs sequential operation patterns
- [ ] Keyword-collision renames and deserialization aliases
- [ ] Unconditional vs conditional serialization skips
- [ ] Guest/entry-point behaviors (middleware, error mapping, body injection)
- [ ] Response type deduplication table
- [ ] Dependency versions from lock file

## Design surface the claims feed

Downstream synthesis assembles `design.md` from Evidence across these areas. Emit claims so each area the source exhibits is answerable from the Evidence alone:

1. **Context** — source component path, purpose, source files analyzed
2. **Domain Model** — full nested type definitions with wire-format annotations; entities with attributes, relationships, and business rules; field-level renames, aliases, unconditional skips, and conditional skips; deduplication of shared response types
3. **Structures** — source code structure inventory (imports, exports, classes, functions, external dependencies)
4. **API Contracts** — inbound endpoints with request/response schemas; outbound API calls with complete request/response shapes traced from actual deserialization
5. **External Services** — each service with type (database, managed table store, message broker, cache, identity provider, API, WebSocket), technology, operations, connection details, authentication
6. **Constants & Configuration** — every constant with source (hardcoded/env var), literal value, semantics, required flag, default
7. **Business Logic** — tagged pseudocode algorithm for every handler/function. **Every controller endpoint** that delegates to a service method needs an algorithm claim, including simple list endpoints — otherwise downstream generation has no algorithm to implement. See [Context Gaps §14](context-gaps.md#14-simple-list-endpoints-missing-business-logic-blocks). Include: execution mode, input/output types, error handling, state mutations, preconditions, postconditions, edge cases, errors raised, unknowns
8. **Publication & Timing Patterns** — topic/queue names, construction patterns, message counts, timing, payload structures, partition keys, custom headers
9. **Output Event Structures** — full nested output type schemas
10. **Implementation Constraints** — factual `[runtime]` constraints describing source behavior (never prescribe target-specific solutions), e.g. in-memory cache with startup/background loading, `setTimeout`/`setInterval` refresh, circuit breakers, in-process OAuth token caching. When API response parity matters, cover serialization fidelity (optional fields, DateTime format, field naming, concurrency)
11. **Source Capabilities Summary** — generic adapter categories the source exercises (Configuration, Outbound HTTP, Message publishing, Key-value state, Authentication/Identity, Table/database access, Real-time messaging, Blob storage, Document storage)
12. **Dependencies** — external packages with manifest version specifier and lock-file resolved version, plus enabled feature flags
13. **Risks / Open Questions** — unknowns, ambiguous source patterns, missing lock file
14. **Notes** — additional observations, source-specific constructs, performance/security considerations

### IMPORTANT — Managed data store classification

When the source code uses `@azure/data-tables`, `TableClient`, `listEntities`, `createEntity`, `updateEntity`, `deleteEntity`, or calls Azure Table Storage REST endpoints (`*.table.core.windows.net`):

- Claims MUST classify these as a `managed table store`, NOT as an external HTTP `API`.
- The capabilities coverage MUST include `Table/database access`.
- Cloud-managed table/document stores (Azure Table Storage, Cosmos DB, DynamoDB) are data stores, not external HTTP APIs, regardless of their access protocol.
- When the source uses blob storage APIs (`BlobServiceClient`, `ContainerClient`, `S3Client`, `putObject`, `getObject`), classify as a `blob store` and cover `Blob storage`.
- When the source uses document database APIs (`MongoClient`, `CosmosClient` document API, `find`, `insertOne`), classify as a `document store` and cover `Document storage`.
- When the source loads data from a managed table store and caches it in memory, cover **both** `Table/database access` and `Key-value state`.
