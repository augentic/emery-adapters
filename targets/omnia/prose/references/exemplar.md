# Exemplar checkout

The worked-code reference for Omnia builds is [`augentic/omnia-exemplar`](https://github.com/augentic/omnia-exemplar): a compiling, CI-green Omnia workspace (a fictional transit operator) that demonstrates current SDK idioms as running code — connectors, adapters, a root-package Axum guest, tests, and fixtures. The build agent reads it in place of static code listings; upstream CI, not the consumer build, compiles it.

## Checkout contract

The build's preparation leg ([`prompts/build/prepare.md`](../prompts/build/prepare.md)) owns producing the checkout at `target/omnia-exemplar/` (git-ignored, outside the cargo workspace) — the clone/refresh algorithm, the stale-checkout fallback, and the stop-hint path live there. By the time any writer prompt runs, the checkout exists and the adapter has validated its template contract.

- The checkout tracks `main` unpinned — each build reads current `main`.
- **Read-only.** Never edit the checkout, never add it to the workspace members, and never copy files wholesale into the consumer workspace; read it to match idioms, then write consumer code against the slice's artifacts.
- Validate only the generated consumer workspace. Exemplar code is compiled by exemplar CI, never by the consumer build.

## Compatibility contract

`exemplar.yaml` at the checkout root declares the exact Omnia contract the exemplar is green against:

```yaml
omnia:
  version: <semver>
  repository: https://github.com/augentic/omnia
  rev: <commit>
```

- **Create mode** — adopt that contract when authoring dependencies: use the declared `version` for the `omnia`/`omnia-*` workspace dependencies and mirror the exemplar's `[patch.crates-io]` block at the declared `rev`. Do not resolve a different omnia version.
- **Update mode** — preserve the consumer's existing pin; never upgrade as a side effect. When the consumer's pin differs from the exemplar's, the scaffold prelude records a soft warning in the generation user prompt **and prefer idioms evidenced in the consumer's existing crates over exemplar idioms wherever the two conflict** — the exemplar's `main` moves ahead of consumers by default, and copying newer-SDK patterns into an older-SDK workspace burns the engine's verification-repair budget. Stop (per `## § Stop hint contract`) only on a concrete API incompatibility that prevents the requested build.

### Schema-version coupling

The adapter's scaffold prelude pins exact `schema-version` values for `exemplar.yaml` and `templates/guest/manifest.yaml`. Bumping either version in the exemplar is a coordinated release: land the exemplar contract change and the matching adapter bump together, or consumer builds fail closed at the prelude.

## Navigation map

| Purpose | Path in the checkout |
| ------- | -------------------- |
| Minimal connector (HTTP ingress → validate → publish) | `crates/tally-connector/` |
| Connector with a vendor transport (SOAP/XML decode) | `crates/pulse-connector/` |
| Compact adapter (transform + enrich + publish) | `crates/pulse-adapter/` |
| Full-size adapter (state, upstream APIs, feature gate) | `crates/gtfs-adapter/` |
| Config key catalog, route/topic tables, shared API clients | `crates/common/` |
| Extra capability ops + mocks (`BlobStore`, `Broadcast`, `DocumentStore`, `TableStore`) | `crates/capability-examples/` |
| Root-package Axum guest (preferred compiling shape) | `src/lib.rs` |
| Guest runtime example (`omnia::runtime!`) | `examples/runtime.rs` |
| Mock-provider tests and fixtures | `crates/tally-connector/tests`, `pulse-connector/tests`, `gtfs-adapter/tests`, each crate's `data/` |
| Replay tests over live captures | `crates/pulse-adapter/tests/` + `data/replay/` |
| Workspace shape: root guest package, members, lints, dependencies, profiles | root `Cargo.toml` |
| Omnia compatibility contract | `exemplar.yaml` |

The repository README carries the architecture, the route/topic tables, and a "Copy this, not that" list separating general patterns from Acme domain quirks — read it before the crates.

Do **not** hand-copy from `templates/guest/` or the exemplar's root tooling files during builds: the adapter's deterministic scaffold prelude reads that contract (`exemplar.yaml` → `templates/guest/manifest.yaml`) from this same checkout and has already written its output before the writer prompts run.

## Authority

Scope the exemplar to **current SDK implementation idioms**. Emery artifacts (`spec.md`, `design.md`, `tasks.md`) and the `OMNIA-*` / `UNI-*` engineering rules outrank it; in update mode, the consumer's existing code remains authoritative for unchanged behavior. The exemplar outranks the retained explanatory references under [`examples/`](examples/) and model inference. Full hierarchy: [`hard-rules.md`](hard-rules.md).
