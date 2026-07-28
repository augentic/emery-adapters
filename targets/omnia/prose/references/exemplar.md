# Exemplar checkout

The worked-code reference for Omnia builds is [`augentic/omnia-exemplar`](https://github.com/augentic/omnia-exemplar): a compiling, CI-green Omnia workspace (a fictional transit operator) that demonstrates current SDK idioms as running code — connectors, adapters, both guest styles, tests, and fixtures. The build agent reads it in place of static code listings; upstream CI, not the consumer build, compiles it.

## Checkout contract

At the start of the generation leg, before writing any code, prepare the checkout inside the consumer workspace's `target/` directory (git-ignored, outside the cargo workspace):

```bash
if [ -d target/omnia-exemplar/.git ]; then
  git -C target/omnia-exemplar fetch --depth 1 origin main \
    && git -C target/omnia-exemplar reset --hard origin/main
else
  git clone --depth 1 https://github.com/augentic/omnia-exemplar target/omnia-exemplar
fi
```

- The checkout tracks `main` unpinned — each build reads current `main`.
- **Read-only.** Never edit the checkout, never add it to the workspace members, and never copy files wholesale into the consumer workspace; read it to match idioms, then write consumer code against the slice's artifacts.
- Validate only the generated consumer workspace. Exemplar code is compiled by exemplar CI, never by the consumer build.

**Failure handling.** If the clone fails (network, availability) and no checkout exists, **stop**: surface a stop hint per the build prompt's `## § Stop hint contract` (`failing-task`: the exemplar checkout step; `next-action`: retry after restoring access) — there is no embedded fallback for the worked code, so proceeding would generate from weaker guidance than the operator expects. If a refresh fails but a previous checkout exists, proceed with the stale checkout and record the staleness as a non-blocking finding in the build report.

## Compatibility contract

`exemplar.yaml` at the checkout root declares the exact Omnia contract the exemplar is green against:

```yaml
omnia:
  version: <semver>
  repository: https://github.com/augentic/omnia
  rev: <commit>
```

- **Create mode** — adopt that contract when authoring dependencies: use the declared `version` for the `omnia`/`omnia-*` workspace dependencies and mirror the exemplar's `[patch.crates-io]` block at the declared `rev`. Do not resolve a different omnia version.
- **Update mode** — preserve the consumer's existing pin; never upgrade as a side effect. When the consumer's pin differs from the exemplar's, note the mismatch as a soft warning **and prefer idioms evidenced in the consumer's existing crates over exemplar idioms wherever the two conflict** — the exemplar's `main` moves ahead of consumers by default, and copying newer-SDK patterns into an older-SDK workspace burns the verify-repair budget. Stop (per `## § Stop hint contract`) only on a concrete API incompatibility that prevents the requested build.

## Navigation map

| Purpose | Path in the checkout |
| ------- | -------------------- |
| Minimal connector (HTTP ingress → validate → publish) | `crates/tally-connector/` |
| Connector with a vendor transport (SOAP/XML decode) | `crates/pulse-connector/` |
| Compact adapter (transform + enrich + publish) | `crates/pulse-adapter/` |
| Full-size adapter (state, upstream APIs, feature gate) | `crates/gtfs-adapter/` |
| Config key catalog, route/topic tables, shared API clients | `crates/common/` |
| Typed-router guest (style A — prefer this) | `guests/typed/` |
| Hand-written Axum guest (style B) | `guests/axum/` |
| Mock-provider tests and fixtures | `crates/tally-connector/tests`, `pulse-connector/tests`, `gtfs-adapter/tests`, each crate's `data/` |
| Replay tests over live captures | `crates/pulse-adapter/tests/` + `data/replay/` |
| Workspace shape: members, lints, dependencies, profiles | root `Cargo.toml` |
| Omnia compatibility contract | `exemplar.yaml` |
| Guest runtime examples (`omnia::runtime!`) | `guests/*/examples/` |

The repository README carries the architecture, the route/topic tables, and a "Copy this, not that" list separating general patterns from Acme domain quirks — read it before the crates.

Do **not** read `templates/guest/` from the checkout during builds: that subtree is the scaffold-template source baked into this adapter at adapter-build time, and the deterministic scaffold prelude has already written its output.

## Authority

Scope the exemplar to **current SDK implementation idioms**. Emery artifacts (`spec.md`, `design.md`, `tasks.md`) and the `OMNIA-*` / `UNI-*` engineering rules outrank it; in update mode, the consumer's existing code remains authoritative for unchanged behavior. The exemplar outranks the retained explanatory references under [`examples/`](examples/) and model inference. Full hierarchy: [`hard-rules.md`](hard-rules.md).
