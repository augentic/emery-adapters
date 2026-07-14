# Verification, error handling, and recovery

Before answering with the Evidence document, verify the coverage self-check in [design-template.md](design-template.md) and the extraction-specific checklist below. This adapter emits Evidence claims only — downstream synthesis (which owns `spec.md` and `design.md`) can only be as good as the claims it receives, so verify against the source, not against artifacts.

## Extraction verification checklist

### Evidence completeness

- [ ] **Requirement claims per behavior**: One `requirement` claim (stable dotted kebab `id`) for each distinct business rule, with scenario-grade synopses covering happy paths, error handling, and edge cases
- [ ] **Design surface covered**: Claims cover every design area the source exhibits (see [design-template.md](design-template.md) § Design surface the claims feed)

### Analysis fidelity

- [ ] **Config keys verbatim**: Environment variable names captured exactly as written in source (not renamed for clarity). If source reads `CC_STATIC_URL`, the claim must say `CC_STATIC_URL` -- not `GTFS_CC_STATIC_URL` or `GTFS_STATIC_URL`.
- [ ] **API response shapes**: Each external API response includes the actual deserialized type (e.g., `string[]` vs `{ data: { all: [...] } }`), traced from actual deserialization code, not inferred from type declarations. Include a concrete JSON example.
- [ ] **API URL fidelity**: API URL paths and query parameters match the source code exactly. Do not add or remove query parameters.
- [ ] **Authentication source**: For each authenticated API call, document whether the identity name is hardcoded or comes from a config variable (e.g., `AZURE_IDENTITY`).
- [ ] **Publication pattern precision**: Publication count, delay placement (before/after), and payload identity (identical or modified) documented from actual loop structure in source code.
- [ ] **Metrics**: Metric names, types, emission points, and labels captured in claims when explicit in the source
- [ ] **Message metadata**: Partition keys, headers, and topic construction patterns captured
- [ ] **Wire-format field names**: All deserialized/serialized types include wire-format field name annotations where the source uses renaming decorators/annotations/config
- [ ] **Custom converters**: Conversion logic for custom deserializers/serializers is documented (e.g., what `BooleanConverter` does)
- [ ] **Active subsets**: Lookup tables that are filtered at runtime note which entries are active (see [Context Gaps #11](context-gaps.md#11-active-subset-vs-full-dataset))
- [ ] **Field optionality**: Input type fields include an `Optional?` column indicating whether the field may be absent/null at runtime (yes/no/unknown)
- [ ] **Output type completeness**: Output types document ALL fields from the type definition (including fields not populated by this component), with notes on which fields this component populates vs. which are present in the shared schema
- [ ] **Output field types**: Output type fields document exact types (e.g., `integer` not `float` for integer fields, exact enum types not raw strings). When the source code uses a specific numeric type (e.g., `speed: integer`), do not generalize it (e.g., `speed: float`).
- [ ] **External service classification**: All external services categorized by type (database, managed table store, cache, message broker, identity provider, API, WebSocket). Managed data stores (Azure Table Storage, Cosmos DB, DynamoDB) classified as `managed table store`, not `API`.
- [ ] **Source capabilities coverage**: The generic adapter categories the source exercises are derivable from the External Services claims. `Table/database access` covered whenever source uses ORM, SQL, or managed table stores.
- [ ] **Keyword-collision renames**: Fields using language-reserved keywords as wire names are documented with their field-level rename attributes
- [ ] **Deserialization aliases**: Fields accepting multiple wire names are documented with all alias attributes
- [ ] **Unconditional serialization skips**: Unconditional skip attributes distinguished from conditional ones; both documented explicitly
- [ ] **Collection field defaults**: Each collection/array field's default-when-absent behavior checked individually (not assumed universal)
- [ ] **Guest/entry-point behaviors**: Middleware, error mapping, body injection, parameter sourcing documented
- [ ] **Transport projection ownership**: Deduplication table showing which HTTP/messaging/WebSocket/command boundary owns projection for shared plain outputs
- [ ] **Dependency versions**: Lock file versions captured; manifest specifiers used as primary in Dependencies section

## Error handling

### Common issues and resolutions

- **TypeScript source doesn't parse**: Cause: invalid TypeScript or missing dependencies. Resolution: run `tsc --noEmit` to verify the source compiles first.
- **Too many unknowns in the Evidence**: Cause: dynamic typing, metaprogramming, or unclear logic. Resolution: review the source for type annotations and add comments for clarity.
- **Evidence missing business logic**: Cause: functions not exported or in inaccessible modules. Resolution: check module imports and ensure key functions are exported.
- **Evidence missing API endpoints**: Cause: routes defined dynamically or in middleware. Resolution: check framework-specific routing patterns such as Express or Nest.
- **Config keys not captured**: Cause: environment variables accessed indirectly. Resolution: search for `process.env` patterns across all source files.
- **Type shapes incomplete**: Cause: complex generic types or union types. Resolution: document the full type definition and use `unknown` for unresolvable generics.
- **Dependency version drift**: Cause: versions captured from manifest ranges instead of lock file. Resolution: always read the lock file for exact resolved versions.

### Recovery process

1. Review the emitted Evidence against the source code
2. For missing sections: identify the source construct that should have been captured
3. Re-analyze the specific source file or function
4. For persistent [unknown] tags: add source code comments to clarify intent
5. Re-run the full analysis
