# Translation

How a requirement that names a mechanism lands in the [layout](layout.md). The specification was read from a source that ran somewhere — on a broker, behind a web server, against a vault, under an APM agent — and a requirement, a scenario, or a design type may still spell that platform's parts: the consumer, the producer, the client, the agent, the group, the timer. The guest is a WebAssembly component on the omnia host, and the host plays every one of those parts through a capability of the provider, as the [handlers](handlers.md) reach them, so the specification's technology is never the tree's.

## The rule

- The specification decides what the program does; this layout decides how. A requirement that states a mechanism is read for the behaviour behind it — what arrives, what is checked, what is answered or published, what fails — and that behaviour is implemented through the capability that plays the role. The mechanism's own parts are laid nowhere: no crate for the product, no type named for it, no setting for its address, no state standing in for its session.
- The design's `apis` section names each system the source reached by its role, with the technology beside it. The role is what a module reaches — a stream through `Publish` and the topic list, a store through `StateStore`, `TableStore`, `DocumentStore`, or `BlobStore`, an upstream through `HttpRequest`, an identity provider through `Identity`, a setting through `Config` — and the technology is what the deployment binds behind the host, which no line of the tree names.
- A requirement that states behaviour and mechanism together is covered by the behaviour: `the consumer in group X rejects an update older than the last seen` is covered once a handler of the topic list rejects such an update, whatever the group was.
- A requirement whose whole statement is the mechanism — the broker's addresses, the consumer group, the transport security, the client's construction, the agent's initialisation, the delay between two sends — states nothing the tree can hold. It is left out of `covered`, with no stub, no constant, and no comment standing for it, as [build.md](../build.md)'s report rule has it. A design type the design labels `(mechanism)` — a client, a connection, a settings bag, a retriever, a lifecycle — is laid by no slice, and the plan assigns its key to none; the domain types beside it, the payloads, the events, the records, each assigned to one slice, are laid as the handlers reference spells them.
- A setting is kept where it configures behaviour and dropped where it configured the mechanism: `OVERDUE_DAYS` is a constant of the catalog, read where the rule applies; `KAFKA_HOSTS`, `KAFKA_GROUP_ID`, `APM_SERVICE_NAME`, `PORT`, and `VAULT_URL` are the host's and get no constant. A key the source read a topic from is no setting here either: the topic is a constant of the topic table, qualified by `config::topic` as every topic is.
- The catalog, `crates/common/src/config.rs`, declares what the deployment supplies and may vary between deployments: an upstream's endpoint, an identity or a credential's key, a bound an operator tunes. A value the specification states as a value is not a setting: a mapping from one code to another, a format string, a timezone or a locale, a threshold a scenario spells out is a `const` of the module that reads it, spelled as the scenario states it, with no catalog constant and no default standing for it; and a setting that configured the mechanism alone — the broker's, the vault's, the agent's — gets none either.

## The table

| The specification or design says | The guest construct | The slice lays | `covered` |
| --- | --- | --- | --- |
| a message is consumed from a topic or a queue; a consumer's handler, `onMessage`, `eachMessage`, a listener that calls the next step | a topic of the topic list, its handler a `Handler` over the payload | a `pub const` in `crates/common/src/routes/topic.rs`, a statement in `src/messaging.rs` through `messaging::consume(<handler>)` — or `messaging::consume_with(<handler>, <decode>)` with a `pub fn(&Delivery) -> Result<I, DecodeError>` beside the handler where the payload is XML, CSV, or bytes rather than JSON, the decoder reading the payload into a type deriving `Deserialize` through the format's `serde` reader (`quick_xml::de::from_str` for XML, `quick-xml` with its `serialize` feature named in the stem's manifest), a repeated element a `Vec` field of it, never an event loop walking the reader — an entry in `tests/messaging.rs`, and the handler with one test per scenario, each a `Client::call` over a payload | yes, by the handler and its tests |
| a message is published, produced, or sent, with a key, headers, or a partition | `Publish::send` to a constant of the topic table | the constant, where no consuming slice declares it; the `send` under `config::topic(provider, routes::topic::<NAME>).await`, the payload as the requirement states it, a key or a header an entry of `Message::headers` (`key`); a test reads `provider.publish.sent()` | yes |
| the broker's addresses, ports, TLS or SASL, the consumer group, the client id, offsets and commits, a connect or a disconnect, a dormant or secondary subscription switched by a flag | nothing: the host binds every topic of the two lists | nothing, and no catalog constant | no |
| a secret read from a vault or a key store; a retriever class; a watcher that restarts on rotation | `Config::get` at the moment of use, or `Identity::access_token` for a token exchanged | a catalog constant named for what the value is (`UPSTREAM_API_KEY`, never the vault), read in the handler that uses it; no cache, no refresh, no watcher | yes where a handler reads the value; no for the retriever, the cache, or the rotation itself |
| an upstream REST or GraphQL call with a bearer token or an API key | `HttpRequest::fetch` over an `http::Request`, the token from `Identity::access_token` or `Config::get` | the base URL a catalog constant, the request built in the handler, the response read into a struct in the body's own shape — its fields at the root as the design's `apis` section spells them, never under a `data` or `body` field the source's HTTP client held it in — a failed upstream `bad_gateway!`; a test answers it through `MatchedHttp` | yes |
| a reference table or a mapping loaded at start and refreshed every N minutes | cache-aside over `StateStore` with a TTL, as [the fragment below](#a-reference-table) spells it | one `pub async fn` of the stem whose handlers read the table, laid once in the tree and never under `start`: `StateStore::get` under one key of the stem; on a miss — `Ok(None)` — the fetch or the store read, then `StateStore::set` with `ttl_secs` of N × 60, and the table answered, never an empty one; no load at start, no timer, no key recording the next fetch or that the first load ran, no handler that loads, no retry loop around the fetch, and no second loader under `start` or any other stem over a key of its own for the same table | yes for the load, the lookup, and the staleness bound, by the slice that lays the function; no for the schedule itself |
| a store: a SQL database, a document or table service, a blob container, a cache server | `TableStore` for SQL, `DocumentStore` for documents and managed tables, `BlobStore` for objects, `StateStore` for keyed bytes; never `HttpRequest` to a store's own REST endpoint | the calls in the handler over the named connection, store, or container; the doubles of `omnia_test::guest` in the tests | yes |
| a delay or a sleep between two actions, a retry loop with waits, a backoff, N attempts before giving up | nothing: a handler runs to its answer, and a failed delivery is the host's to redeliver | the actions back to back, once; a failure carried as the classified error. A `loop {` in a module is named with its line and refused, whatever it counts | no for the delay, the wait, or the retry count; yes for the actions |
| an APM agent, a transaction or span opened and closed, a metric counter, an error notice, a status line logged | a `tracing` event with the fields the requirement names; a metric a `tracing` event the host's metrics layer reads by its field's prefix | `tracing::info!`, `warn!`, or `error!` at the point the requirement states; a counter `tracing::info!(monotonic_counter.<name> = 1)`, a measurement `tracing::info!(histogram.<name> = <value>)`, a level `tracing::info!(gauge.<name> = <value>)`, `<name>` in snake_case under the stem (`orders.rejected_expired`) with the agent's prefix dropped; no agent, no exporter crate, no `StateStore` key standing for a span or a count | yes where the requirement states what is observed and with what fields; no where it states the agent or the exporter |
| a web server, a port, listening, framework middleware run for every route | the host answers HTTP through the route list; a rule every route obeys is a `pub fn` of `start` | nothing for the server; the `pub fn` and the calls at the top of the handlers it guards for the rule | as [what `start` becomes](guardrails.md) says |
| a health or status route stated by method and path | a route | the route in the stem that states it, answering what the requirement says, never a lifecycle's flag | yes |
| a WebSocket push to connected clients | `Broadcast::send` | the `send` in the handler; a test reads `provider.broadcast.broadcasts()` | yes |
| a design type the design labels `(mechanism)` — a client, a connection, a settings struct, a retriever, a `Main` or an `App` | nothing | no struct, under no name: the plan assigns a mechanism type to no slice, and a slice whose requirements mention one reaches the capability that plays its role; its setting fields become catalog constants where a handler reads them | a type is no requirement; a requirement stating its construction is left out |
| a design type without the label — a payload, an event, a record, a result | a `pub` struct of the slice the plan assigns the key to, in the wire shape the design's `apis` section spells | the struct as the [handlers](handlers.md) reference lays it, `serde(rename)` where the wire's field names are not Rust's | a type is no requirement |

## A reference table

A stem that reads a table the source loaded at start and refreshed on a schedule lays one function the handlers call at the moment of use, and nothing of the schedule:

```rust
use common::config;
use omnia_sdk::{Config, HttpRequest, StateStore, server_error};

/// How long a loaded table serves before it is fetched again.
const TTL_SECS: u64 = 10 * 60;

/// The state-store key of the rate table.
const RATES: &str = "pricing/rates";

/// The rate table, fetched when the store holds none or the held one has aged out.
///
/// # Errors
///
/// `bad_gateway` when the upstream cannot be reached; `server_error` when a held table does not read back.
pub async fn rates<P>(provider: &P) -> omnia_sdk::Result<Vec<CurrencyRate>>
where
    P: Config + HttpRequest + StateStore,
{
    if let Some(bytes) = StateStore::get(provider, RATES).await? {
        return serde_json::from_slice(&bytes).map_err(|error| server_error!("rate table: {error}"));
    }
    let rates = fetch_rates(provider).await?;
    let bytes = serde_json::to_vec(&rates).map_err(|error| server_error!("rate table: {error}"))?;
    StateStore::set(provider, RATES, &bytes, Some(TTL_SECS)).await?;
    Ok(rates)
}
```

`fetch_rates` is the upstream call of the row above it, over `HttpRequest::fetch`, failing as `bad_gateway!`. A miss — `StateStore::get` answering `Ok(None)` — fetches, stores, and answers the table it fetched, never an empty one: a function that answers `Vec::new()` on a miss has laid the cache and lost the table. There is no key holding when the next fetch is due, no handler that loads the table, no retry around the fetch, and no flag that the first load ran: a failure is the handler's error, and the host redelivers or the caller retries. The function is laid once, by the slice whose requirements state the table's use, in that stem's crate, and `start` never lays one under `crates/start/`, whatever its requirements state of the prefetch: a loader laid first under `start` is a second one the moment the reading stem lays its own, so `start` lays nothing of the table and covers nothing of it, as [what `start` becomes](guardrails.md) says.

## A worked example

A stem `shipments` whose requirements were read from a consumer: `the Kafka consumer on topic KAFKA_SRC_TOPIC in group KAFKA_GROUP_ID parses each XML message into a ShipmentNotice`, `a notice whose timestamp precedes the last one seen for its shipment is rejected and the notice counter incremented`, `each remaining notice is converted and sent to the producer's topic with the shipment id as the key, with DELAY_MS between sends`, and the design's `apis` section names an inbound stream and an outbound stream, both over Kafka, with the suffixes `shipment-notices.v1` and `shipments-dispatched.v1`.

The slice lays two constants in the topic table, `SHIPMENT_NOTICES` and `SHIPMENTS_DISPATCHED`; one statement in the topic list, `messaging::consume_with(shipments::consuming::on_notice, shipments::consuming::decode)`, the decoder reading the XML payload into the design's `ShipmentNotice` through `quick_xml::de::from_str`; one entry in the messaging rung; and the handler, bounded on `StateStore + Publish + Config`, which reads the last timestamp seen under `shipments/<shipment id>`, rejects an older notice with a `tracing::warn!` naming the shipment and answers `Ok(())`, stores the newer one, converts it, and publishes it once through `Publish::send` under `config::topic(provider, routes::topic::SHIPMENTS_DISPATCHED)` with the shipment id in the `key` header. The tests call `on_notice` with an XML payload through `Client::call` over a `Provider` whose `storage` holds the earlier timestamp, and read `publish.sent()` for the one message and its key.

The report covers the first requirement by the parse, whatever its group and its setting said, the second by the rejection, and the third by the conversion and the publication, the delay laid nowhere. A requirement whose whole statement is the delay, or the counter as the agent's metric rather than what is observed, is left out: nothing in the tree holds it, and the operator reads the gap in the plan rather than a stub in a module.

## What it is not

- A `KafkaConsumerSettings`, a `BrokerConfig`, a `ConsumerGroup`, a `Producer` struct, or a module that connects: the host subscribes and publishes, and the topic list is the whole of the subscription.
- A `StateStore` key standing for a span, a transaction, or an agent's counter (`set("apm/transaction", ..)`): a trace is a `tracing` event, and a count the host does not keep is not kept.
- A topic suffix composed at run time from an environment prefix and a dormant flag: the environment is `config::topic`'s, the dormant switch is laid nowhere, and the constant is the suffix as the design states it.
- A `start` module that constructs the clients the source built, reads the settings into a struct, loads a table another stem reads, or holds the program's state, which [what `start` becomes](guardrails.md) lays nothing for.
- A struct wrapping an upstream's response in the `data` or `body` field the source's HTTP client held it in: the wire carries the body alone, and the struct is the body's shape.
- An event loop over an XML or CSV reader, matching each element or record by hand: the payload is read through the format's `serde` reader into a type deriving `Deserialize`, and a repeated element is a `Vec` field.
- A catalog constant for a value the specification states — a code mapping, a format string, a timezone, a threshold a scenario spells — which is a `const` of the module that reads it, as the scenario states it.
- A `#![allow(..)]` at the head of a module or a crate, silencing the workspace's lints over code the translation left as the source had it: the lints are the tree's, a module that needs them off is laid wrong, and the verify refuses it with the file named.
