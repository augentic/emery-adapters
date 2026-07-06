# Omnia reference material

Reference documentation for the Omnia target adapter at [`adapters/targets/omnia/`](..). In Specify, Omnia is a **target adapter** — `guidance`, `build`, `merge` — not a slash-command plugin.

The Omnia crate / test / guest / review instructions live in [`../prompts/build.md`](../prompts/build.md) and the per-leg prompts under [`../prompts/build/`](../prompts/build/); the adapter core (`../core/src/operations.rs`) owns leg sequencing. The depth (templates, hard rules, mapping tables, mock-provider patterns, specialist prompts, rules) and worked examples live in this folder.

## Prompts

| Prompt | Purpose |
|--------|---------|
| [`guidance.md`](../prompts/guidance.md) | Idiom guidance (provider DI, WASM guardrails, error variants, validation placement) consumed by core synthesis. |
| [`build.md`](../prompts/build.md) | Shared build preamble: bindings, mode detection, verify-repair loop, stop-hint contract, report shape. |
| [`build/crate.md`](../prompts/build/crate.md) | Generation leg: generate or update the Rust crate. |
| [`build/test.md`](../prompts/build/test.md) | Generation leg: generate or update the test suite. |
| [`build/guest.md`](../prompts/build/guest.md) | Generation leg (create mode only): scaffold the WASM guest wrapper. |
| [`build/review.md`](../prompts/build/review.md) | Review leg: agent-team code review and remediation cycle. |
| [`build/replay.md`](../prompts/build/replay.md) | Replay leg (self-skipping): runtime capture replay when a `captures` source is bound. Delegates hook contract to [`../../../shared/prose/references/replay/`](../../../shared/prose/references/replay/). |
| [`merge.md`](../prompts/merge.md) | Merge leg: delta fold plus the pre-merge gate (cargo + clippy + test + wasm32 build). |

## References

### Authority and hard constraints

- [`hard-rules.md`](hard-rules.md) — full hard-rules set and authority hierarchy.
- [`guardrails.md`](guardrails.md) — forbidden crates, std APIs, WASM constraints, serde / timestamp / DST idioms.
- [`wasm-constraints.md`](wasm-constraints.md) — translating `[runtime]` constraints to Omnia/WASM patterns.

### Capabilities and providers

- [`capabilities.md`](capabilities.md) — provider trait signatures and adapter triggers (all nine providers).
- [`capability-mapping.md`](capability-mapping.md) — mapping from Specify artifact adapters to Omnia provider traits.
- [`providers/`](providers/) — per-trait deep dives (blobstore, broadcast, config, document-store, http-request, identity, publish, state-store).

### Crate writer depth

- [`sdk-api.md`](sdk-api.md) — `Handler<P>`, `Context`, `Reply`, `IntoBody`, `Client`, `Error`; Input Type Decision Tree; Response Types.
- [`cargo-toml.md`](cargo-toml.md) — workspace and crate `Cargo.toml` templates.
- [`error-handling.md`](error-handling.md) — domain error enums, `omnia_sdk::Error` mapping, troubleshooting.
- [`cross-cutting-matrices.md`](cross-cutting-matrices.md) — Side-Effect / Outbound-Message / Transaction-Boundary matrices.
- [`update-patterns.md`](update-patterns.md) — strategy patterns per update category.
- [`change-classification.md`](change-classification.md) — classifying artifact-vs-code diffs.
- [`repair-patterns.md`](repair-patterns.md) — common verify-loop repair recipes.
- [`checklists.md`](checklists.md) — pre-generation and verification checklists.
- [`todo-markers.md`](todo-markers.md) — TODO marker rules, adapter overrides, cache-aside patterns.
- [`output-documents.md`](output-documents.md) — `Migration.md`, `Architecture.md`, `CHANGELOG.md`, `.env.example` shapes.

### Test writer depth

- [`mock-provider.md`](mock-provider.md) — Static and Replay MockProvider patterns per provider trait.
- [`spec-to-test-mapping.md`](spec-to-test-mapping.md) — how spec scenarios map to test functions; `REQ-XXX` traceability.
- [`replay-fixtures.md`](replay-fixtures.md) — `setup` block, `INSTRUCTIONS.md`, TestDef → MockProvider mapping for runtime captures.
- [`replay-crate-layout.md`](replay-crate-layout.md) — generated-crate paths and capture loading for replay tests.

### Guest writer depth

- [`configuration.md`](configuration.md) — guest workspace `Cargo.toml`, `.cargo/config.toml`, `deny.toml`, the five GitHub workflows, `.env.example` shape (templates).
- [`handlers.md`](handlers.md) — HTTP routing, message subscriptions, WebSocket events, `lib.rs` wiring patterns.
- [`guest-patterns.md`](guest-patterns.md) — HTTP / Messaging / WebSocket guest export patterns.
- [`guest-wiring.md`](guest-wiring.md) — crate → guest injection contract.
- [`runtime.md`](runtime.md) — `omnia::runtime!` macro, WASI host options, `.env.example` shape.
- [`project-layout.md`](project-layout.md) — directory layout for the guest project.

### Code reviewer depth

- [`review-categories.md`](review-categories.md) — full SEC/COR/QUA/UNI check library and codex `rule_id` mapping.
- [`team-protocol-crate.md`](team-protocol-crate.md) — verbatim specialist spawn prompts, antagonist protocol, synthesis rules.
- [`review-auto-fix.md`](review-auto-fix.md) — `fix` scope, per-category success-rate table, regression guard.
- [`review-output-template.md`](review-output-template.md) — `REVIEW.md` template and finding-ID conventions.
- [`agent-teams.md`](agent-teams.md) — shared multi-agent review pattern (specialists + antagonist + lead synthesis).
- [`../rules/`](../rules/) — Omnia-specific rules (`OMNIA-001`, `OMNIA-002`, `RUST-001`, `SEC-001`).
- [`../../../shared/prose/rules/universal/`](../../../shared/prose/rules/universal/) — shared `UNI-*` rules.

### Worked examples

- [`examples/crates/`](examples/crates/) — single-handler, multi-handler, anti-patterns; per-capability walkthroughs under `capabilities/`; per-update-category walkthroughs under `updates/`.
- [`examples/tests/`](examples/tests/) — per-provider testing patterns (HTTP, StateStore, Publisher, Blobstore, DocumentStore).
- [`examples/replay/`](examples/replay/) — runtime capture replay (worked handler, test, and capture examples for migration).
