# Train — the SOAP connector and position adapter

A trimmed copy of the hand-written [omnia](https://github.com/augentic/omnia)
guest for realtime train services, cut to the one pipeline the `train-pair`
eval case ports from two TypeScript services into one guest: train SOAP/XML
in over HTTP, raw train-update XML across one topic, vehicle location events
out. It is the reference a `--build --target omnia` run of the eval compares
the built guest with, and the source of the recorded fixtures the built guest
is driven through; nothing in the eval runs its tests.

Domain logic lives under `crates/*` as plain
`pub async fn name<P>(input, Context<P>)` handlers. The **guest is the
workspace root package** (`src/lib.rs`): an `axum::Router` with one
`handle_with` route carrying the vendor's SOAP/XML codecs, and an
exact-topic messaging router over a provider-owning `Client`. The layout
follows [omnia-exemplar](https://github.com/augentic/omnia-exemplar).

## Quick start

```shell
# build the guest
cargo build --target wasm32-wasip2 --release

# run it with the example host runtime
cp .env.example .env   # then fill in values
set -a; source .env; set +a
cargo run --features host --example runtime -- run target/wasm32-wasip2/release/train.wasm
```

## Architecture

```mermaid
flowchart LR
  xml["POST /inbound/xml"] --> soapConn[soap-connector]
  soapConn -->|"{env}-realtime-train.v1"| posAdapter[position-adapter]
  posAdapter -->|"{env}-realtime-train-positions.v1"| downstream["downstream (out of scope)"]
  posAdapter --> stops["CC Static GTFS stops"]
  posAdapter --> allocations["Block Management trip allocations"]
```

Two shapes of service meet in the pipeline:

- **Connector** — thin ingress: decode the transport payload, validate it,
  key it, and publish it to a topic. No enrichment, no state.
  `soap-connector` carries a vendor-specific (SOAP/XML) transport.
- **Adapter** — domain transformation: validate, enrich via upstream APIs,
  and publish derived events. `position-adapter` consumes what the connector
  publishes.

| HTTP route | Handler |
| --- | --- |
| `POST /inbound/xml` | `soap_connector::receive_message` — train SOAP/XML position ingress |

| Messaging topic | Handler |
| --- | --- |
| `{env}-realtime-train.v1` | `position_adapter::train_update` (XML) |

Published-only topic: `{env}-realtime-train-positions.v1`. Its consumer
lives outside this repository.

## Crate map

| Crate | Role |
| --- | --- |
| `crates/common` | Config key catalog, canonical route/topic tables, `TIMEZONE`, and the Block Management trip-allocations client |
| `crates/soap-connector` | HTTP ingress for the train SOAP/XML position feed; republishes the embedded train update to messaging |
| `crates/position-adapter` | Converts train updates into vehicle location events |
| root package (`train`) | Typed-router HTTP + exact-topic messaging WASM guest binary |

Domain crates depend only on the `omnia-sdk` capability traits (`Config`,
`HttpRequest`, `Identity`, `Publish`), so the same code runs inside the WASM
guest and against `omnia_test::guest::Provider` doubles in tests. The
connector and the adapter never import each other: the topic constant
`common::routes::topic::TRAIN` is the one thing they share, and the root
guest wires the connector's publish to the adapter's subscription.

## Configuration

All keys are declared as constants in `common::config`; `.env.example`
carries placeholder values.

| Key | Used by | Purpose |
| --- | --- | --- |
| `ENV` | soap-connector, position-adapter | Deployment environment; prefixes every topic (`dev-…`). Defaults to `dev` with a warning when unset |
| `BLOCK_MGT_URL` | common, position-adapter | Block Management API base URL |
| `CC_STATIC_URL` | position-adapter | CC Static (static GTFS) API base URL |
| `AZURE_IDENTITY` | position-adapter | Identity used to acquire access tokens for the upstream APIs |

The `IDENTITY_CLIENT_ID` / `IDENTITY_CLIENT_SECRET` / `IDENTITY_TOKEN_URL`
entries in `.env.example` are read by the example host's identity backend,
not the guest.

## Domain quirks

Service-specific requirements that are **not** general patterns:

- **SOAP fault as the handler error** — `soap-connector` rejects requests
  with a pre-rendered XML `<Fault>` (`HttpError::with_body`, `text/xml`)
  because the vendor protocol demands it, and parses the envelope inside
  the handler so even malformed bodies get the fault.
- **Duplicate publish** — `position-adapter` publishes each vehicle event twice
  (`PUBLISH_REPEATS`) because a downstream schedule-adherence process needs
  the repeat. The legacy system also spaced the repeats with a blocking
  sleep; that was removed — never block a WASM guest.
- **Freshness window** — a train update older than sixty seconds, or more
  than thirty seconds in the future, is refused rather than published.

## Tests

- `crates/soap-connector/tests/static.rs` — the captured vendor envelope
  (`data/receive-message.xml`) forwarded through a mock publisher, plus the
  SOAP fault sad paths.
- `crates/position-adapter/tests/static.rs` — the recorded fixtures under
  `data/static/`, each one XML message with the upstream answers it needs
  and the events it publishes, re-anchored to now.
- `crates/position-adapter/tests/replay.rs` — the recorded production messages
  under `data/replay/`, driven the same way.

Every fixture under `crates/position-adapter/data/` carries an `output`: the
seven recorded with none (`static/0001`–`0005`, `replay/0049`,
`replay/4247`) were given the outcome `static.rs` and `replay.rs` assert
for them, so the eval's oracle reads each fixture as its own expectation.

- `tests/routes.rs` and `tests/messaging.rs` — the production route and
  topic tables driven natively over `omnia_test::guest::Provider` doubles:
  the vendor envelope posted to `/inbound/xml` lands on
  `dev-realtime-train.v1`, and a train update delivered there lands on
  `dev-realtime-train-positions.v1`.

```shell
cargo test --workspace
```
