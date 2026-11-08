# Handlers

A requirement lands as a handler in the slice's module and the lines that wire it: a path as a constant of the part, a setting as a field of its `Settings` struct, a `.route(..)` or `.topic(..)` in the part's `handlers()`, the part's entry in its stem's array, and the stem's entry in the root's, as the [layout](layout.md) places them. The shapes below are the exemplar's fixture stem and compile against the `omnia-sdk` the root manifest pins; use them as they stand, and read the signatures behind them in [the omnia API](omnia-api.md) rather than probing the crates.

## A route

A requirement carrying a route, its `Route:` line, is one route, in the slice's module and one entry of its stem's array:

```rust
// crates/orders/src/placing.rs
use axum::routing::post;
use omnia_sdk::Capabilities;
use omnia_sdk::api::Handlers;

// The paths the part serves.
pub(crate) const ORDERS: &str = "/orders";

pub(crate) fn handlers<P: Capabilities>() -> Handlers<P> {
    Handlers::new().route(ORDERS, post(place))
}
```

```rust
// crates/orders/src/lib.rs, the array
    Handlers::from([placing::handlers()])
```

and the stem's entry in the root router's array, `orders::handlers()`, and its line in the root manifest, `orders = { path = "crates/orders" }` at the end, each once per stem, written by the first slice of the stem and found in the tree by every other.

- A path parameter is `{name}` in the path and a `Path` extractor on the handler, `Path(id): Path<String>`, or `Path((id, line)): Path<(String, u32)>` for several, typed where the design keys it so a malformed one is axum's `400`; the query string is `Query(input): Query<ListCatalogue>` over a struct deriving `Deserialize`, each field an `Option` where the query may leave it out; a JSON body is `Json(input): Json<PlaceOrder>`, last. Two methods on one path are two `.route(..)` calls over one constant in one part, or one `.route(ORDERS, get(list).post(place))`; two parts of one stem never serve one path.
- The handler's `Ok` is what it answers: `Json<O>` is `200` with the output as JSON; `(StatusCode, Json<O>)` the status named, `StatusCode::CREATED` for a creation, `StatusCode::ACCEPTED` for work still settling, `StatusCode::SERVICE_UNAVAILABLE` where a probe found its store down; `StatusCode::NO_CONTENT` alone an empty `204`. The adapter scaffolds `Result<Json<O>>` and, where a scenario states a status, `Result<(StatusCode, Json<O>)>`; changing the one is the return type and the `Ok` expression.

## A topic

A requirement that consumes a topic is one topic: a constant in the topic table where no slice has declared it, a `.topic(..)` in the part's `handlers()`, the `common` line in the stem's manifest, and the part's entry in the stem's array.

```rust
// crates/common/src/topic.rs, at the end
/// An order was placed.
pub const ORDER_PLACED: &str = "order-placed";
```

```rust
// crates/orders/src/placing.rs
use common::topic;

pub(crate) fn handlers<P: Capabilities>() -> Handlers<P> {
    Handlers::new().route(ORDERS, post(place)).topic(topic::ORDER_PLACED, on_placed)
}
```

The consumer is a `Handler` as a route's is, its input the payload decoded as JSON in place of an extractor and its output dropped:

```rust
use omnia_sdk::api::Context;
use omnia_sdk::{Result, StateStore, bad_request};

// The state key of the running count of lines placed.
pub(crate) const LINES_PLACED: &str = "orders/lines-placed";

pub(crate) async fn on_placed<P>(context: Context<P>, input: Placed) -> Result<()>
where
    P: StateStore,
{
    let delta = i64::try_from(input.lines).map_err(|error| bad_request!("{error}"))?;
    let total = context.state().increment(LINES_PLACED, delta).await?;
    tracing::info!(lines = input.lines, total, "order placed");
    Ok(())
}
```

- A payload that is not JSON is decoded by a `fn decode(delivery: &Delivery) -> std::result::Result<I, DecodeError>` of the part, and `handlers()` registers the pair through `.topic_with(topic::ORDER_DISPATCHED, on_dispatched, decode_dispatch_xml)`. A malformed wire payload fails in the decoder with `Err(DecodeError::new(..))`. `I` is the wire's root type itself — the one a stage slice of the stem declared where it owns the type key — read straight from `delivery.payload` through the format's `serde` reader or the stage's own parse function, `quick_xml::de::from_reader(delivery.payload.as_slice())` for XML; the decoder never hands the handler the raw bytes or text under a carrier of its own (`struct InboundBody { xml: String }`) for the handler to parse again.
- A consumer returns `Err(omnia_sdk::Error)` when a delivery is rejected or cannot be processed — `bad_request!` when validation or timeliness rejects the message, `bad_gateway!` when an upstream fails — rather than catching the error and returning `Ok(())`: the host owns the consumer loop and advances past a rejected delivery on `Err`. A valid delivery the requirement says to skip is no rejection: it answers `Ok(())`, as [a skipped message](#a-skipped-message) shows.
- The topic carries no environment: the constant is the topic as the host maps it, the same on both sides.
- A handler that publishes names the constant its consumer is registered under, `context.publish().send(topic::ORDER_PLACED, &Message::json(&placed)?).await?`, and the slice that consumes declares it; where no slice consumes, the publisher declares it. The requirement that consumes a topic owns the handler; the one that publishes owns the `send`. Two parts of one stem never consume one topic.

## The handler

```rust
use axum::Json;
use axum::extract::Path;
use omnia_sdk::api::Context;
use omnia_sdk::{Result, StateStore, StateStoreExt as _, not_found};
use serde::{Deserialize, Serialize};

/// An order as the design states it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Order {
    /// The order's id.
    pub id: String,
    /// The items ordered.
    pub items: Vec<String>,
    /// When the order was placed.
    pub placed_at: chrono::DateTime<chrono::Utc>,
}

pub(crate) async fn get_order<P>(context: Context<P>, Path(id): Path<String>) -> Result<Json<Order>>
where
    P: StateStore,
{
    let order = context
        .state()
        .get_json(&order_key(&id))
        .await?
        .ok_or_else(|| not_found!("no order `{id}`"))?;
    Ok(Json(order))
}

/// The state-store key of an order.
#[must_use]
pub fn order_key(id: &str) -> String {
    format!("orders/order/{id}")
}
```

- A handler is `pub(crate) async fn name<P>(context: Context<P>, <extractors>) -> Result<Json<O>> where P: Capabilities`, scaffolded by the adapter for each route a requirement states over the route's extractors, an empty `I` deriving `Deserialize` where the method carries a body and the route's requirements or scenarios state one, and an empty `O` deriving `Serialize`, its body a `todo!("REQ-.. <subject>: the handler")` under a `let _ = (&context, ..);` line borrowing each parameter. Fill `I` with the body's fields and `O` with the response's; a route with no body keeps no `I`, and a `POST` whose requirements state none takes no `Json` extractor, since axum refuses a bodiless request through one with `415`. Replace the two lines together with the body, as one `edit_files` edit whose `find` is the marker's key the brief quotes, the `todo!`'s text alone, and narrow `P: Capabilities` to the traits the body reaches through the context, `where P: StateStore + Config`, since a bound wider than the body is a finding on the report and on the verdict. A `Json(input)` is taken by value, so destructure it (`let PlaceOrder { lines } = input;`) or read its fields by move rather than through a reference the lints flag. A handler is reached through the stem's `handlers()` alone, so it is `pub(crate)`, as the part's paths, settings, state keys, and `handlers()` are; what another module uses — a type, a key function, a rule — is `pub`. A consumer takes the same shape over the payload's type in place of the extractors.
- Every `pub` item carries a `///`: each type, each field, each helper; a fallible `pub fn` carries `# Errors` naming when it fails; a `pub fn` that answers a value and reaches nothing carries `#[must_use]`. The workspace lints deny their absence.
- The design's types are `pub` structs and enums deriving `Debug`, `Clone`, `PartialEq`, `Eq`, `Serialize`, and `Deserialize`, declared once in the module of the slice that owns the type key and used from there by every other module, through the stem's crate. The adapter lays each from the design's signature, fenced in the type's doc: a TypeScript interface, object alias, or class, a Python model, dataclass, or typed dict, or a Rust struct as a `pub struct` of documented fields, a `string` or `str` a `String`, a `number` an `i64` where the design spells the field a whole number — a typed declaration of it in any signature, a column of a schema the design holds (`weight INTEGER`), or a sentence calling it an integer — or where the field's last word counts (`cents`, `count`, `id`, `index`, `page`, `size`, `total`, `quantity`, `seconds`, `millis`, `grams`, `days`, `ms`, `bytes`, `units`), and an `f64` otherwise or where the design spells it a fraction (`total NUMERIC`), an `int` an `i64`, a `float` an `f64`, a `Date` or `datetime` a `String`, an array a `Vec`, a `Record` or `dict` a `BTreeMap<String, ..>`, an optional field or a `None` default an `Option`, `unknown` or `Any` a `serde_json::Value`, a field's wire name kept through `#[serde(rename = "..")]` over its snake_case spelling, a Python default a `#[serde(default = "default_<field>")]` over a fn beside the struct; an enumeration — a Rust `enum`, a union of string literals, a TypeScript `enum`, a class over `Enum` — as a `pub enum` of PascalCase variants, each `#[serde(rename = "<literal>")]` where the wire carries one; a field typed by a design type the slice owns by that type's name, and by one another slice declares as a `serde_json::Value` under a doc naming the design type, since that slice's module is not in the tree. `Eq` is dropped where a field is a float or holds a type that dropped it, since `assert_eq!` in a test wants `PartialEq` and `f64` has no `Eq`. A signature the adapter cannot lay whole — an alias inferred from a schema, a union of object types, a table — is an empty `pub struct {}` or `pub enum {}` under the fenced signature, the brief's marker, which the slice fills by the same rules. What a module stores is the type as JSON through `set_json`, and what it reads back is the same type through `get_json`.
- A type that crosses the wire — a message published, a payload consumed, an upstream's response read — is declared in the wire's shape, as the design's `apis` section spells it: each field at the path and nesting the wire carries, a nested object a struct of its own, a constant the wire carries a field set to it, and a field whose wire name is not a Rust identifier, or not snake_case, carried through `#[serde(rename = "..")]` — `#[serde(rename = "orderId")] pub order_id: String`, `#[serde(rename = "shippingAddress")] pub shipping_address: ShippingAddress` — never renamed to read better in Rust, since the consumer on the other side reads the wire, not the struct. Even when the design's `apis` section spells `response.data` or `data.all[].carrierId` (extracted from Axios in TypeScript), `data` is the HTTP client's property on its response object and **never** a JSON field on the wire, and the adapter refuses any `Deserialize` struct whose body holds a single `data` field: a JSON array endpoint (`/rates`) is read as `Vec<Rate>`, and a JSON object endpoint (`/shipments/allocations`) as a struct whose fields (`pub all: Vec<CarrierAllocation>`) sit at its root, never inside `struct ... { data: ... }`. In `quick_xml::de::from_str::<T>` and `from_reader::<T>`, `T` itself is the root XML element (`<Envelope>`), and `T`'s fields are that root element's direct children (`#[serde(rename = "ShipmentNotice")] pub notice: Option<ShipmentNotice>`); never wrap `T` in an outer struct with `#[serde(rename = "Envelope")]`, which makes `quick_xml` expect `<Outer><Envelope>...` and fail on every valid `<Envelope><ShipmentNotice>...` payload. An element the wire repeats under one name — the items of an order, the legs of a shipment — is a plain `Vec<T>` field under `#[serde(default)]` and nothing else: the XML reader collects repeated siblings into a sequence for a plain `Vec` alone and reports the second as a duplicate field under a `deserialize_with`, so a test of the type parses a payload holding two of them. An element the design types `boolean` is a `bool` under `#[serde(default)]`, never an `i32`, since the wire spells `true` and `false`. Every field is one the wire carries, named as the wire names it and placed on the element that carries it, never invented or lifted to the root, as the [translation](translation.md)'s root-element item spells out.
- A requirement that states a count, a measurement, or a level the operator observes is one `tracing` event where the behaviour happens, read by the host's metrics layer through its field's prefix: `tracing::info!(monotonic_counter.orders.rejected_expired = 1)` for a count, `tracing::info!(histogram.orders.fulfilment_latency_ms = elapsed)` for a measurement (`elapsed` the difference of two `chrono::Utc::now()` readings, or `Utc::now()` minus a timestamp the input carries, never `std::time::Instant::now()`), `tracing::info!(gauge.catalog.rates_cached = rates.len())` for a level, the name in snake_case under the stem; no counter type, no `StateStore` key, no exporter.
- The host is reached through the context's accessor for each capability, `context.state()`, `context.config()`, `context.http()`, `context.publish()`, `context.tables()`, `context.docs()`, `context.blobs()`, `context.identity()`, `context.broadcast()`, each present where the handler's `P` is bounded on its trait, so two traits that share a method name — `Config::get` beside `StateStore::get` — are told apart by the accessor. The `*Ext` methods — `get_json`, `set_json`, `query_as`, `fetch_json`, `get_parsed`, `get_as`, `access_token` — are the trait's extension, imported `as _`: `use omnia_sdk::{StateStore, StateStoreExt as _}`. A helper several capabilities serve takes `context.provider()` under a bound of its own, `fn read_order<P: TableStore>(provider: &P, id: &str)`.
- A setting is a field of the part's `Settings` struct, loaded where a body needs one, `let settings = PlacingSettings::load(context.config()).await?;` under `P: Config`, and read as `settings.limit`, never by a literal key in a handler; two parts that read one setting each declare it under the same key, and no part reads a setting another declares. The identity `access_token` is asked for is such a setting — a field `upstream_identity: String` under `#[setting(key = "UPSTREAM_IDENTITY")]`, handed on, `context.identity().access_token(&settings.upstream_identity).await?` — never a literal name. What is a setting and what is a constant of the part is the [layout](layout.md)'s rule.
- A failure is a classified error, never a panic: `bad_request!`, `not_found!`, `server_error!`, and `bad_gateway!` each take a format string and build an `omnia_sdk::Error`, answered as `400`, `404`, `500`, or `502` with the JSON body `{"error": "<code>", "message": ".."}`. Because `bad_request!` spells the code `bad_request`, when a requirement or scenario names a specific `400` reason code (`"outdated"`, `"no_update"`, `"wrong_time"`), or the design's mapping of failures to wire codes spells one (`invalid-request` for a malformed body, `not-found` for an unknown id), call `omnia_sdk::Error::bad_request("<reason>", format!(..))` so the reason code reaches the wire's `"error"` field; the mapping is every part's, whether the design states it under `start` or under the part. A refusal the specification codes is checked by the handler over optional fields, `Option<String>` in the input struct and the handler refusing a missing or malformed one with the coded error, since axum's own rejection of a body that does not deserialise answers `422`, or a plain `400`, with no code of the design's. A provider call answers `anyhow::Result`, so `?` on it carries the host's failure into the handler's error as a `server_error`, and a handler that ends on such a call closes with `Ok(())` after it rather than returning the call's result.

## A status of its own

A status outside the four errors' is an error type of the module's: an enum deriving `thiserror::Error`, with a variant wrapping `omnia_sdk::Error` under `#[from]` so `?` still works and a `From<anyhow::Error>` so a provider call's `?` works too, implementing `IntoResponse` itself. The handler answers `Result<(StatusCode, Json<O>), ThatError>`, its `Ok` the status the outcome earns; `thiserror` is pinned in the root manifest and named in the stem's manifest as every other crate is. The fixture's `lookup.rs`:

```rust
use axum::Json;
use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use omnia_sdk::api::Context;
use omnia_sdk::{StateStore, StateStoreExt as _, not_found};

/// Why an order lookup was refused.
#[derive(Debug, thiserror::Error)]
pub enum LookupError {
    /// Another operation holds a lock on the order.
    #[error("order `{0}` is locked")]
    Conflict(String),
    /// The store or a precondition failed.
    #[error(transparent)]
    Host(#[from] omnia_sdk::Error),
}

impl From<anyhow::Error> for LookupError {
    fn from(error: anyhow::Error) -> Self {
        Self::Host(error.into())
    }
}

impl IntoResponse for LookupError {
    fn into_response(self) -> Response {
        match self {
            Self::Conflict(ref id) => {
                let body = serde_json::json!({
                    "error": "conflict",
                    "message": self.to_string(),
                    "id": id,
                });
                (StatusCode::CONFLICT, Json(body)).into_response()
            }
            Self::Host(inner) => inner.into_response(),
        }
    }
}

// `202 Accepted` while the order is still settling.
pub(crate) async fn lookup<P>(
    context: Context<P>, Path(id): Path<String>,
) -> Result<(StatusCode, Json<OrderStatus>), LookupError>
where
    P: StateStore,
{
    let key = order_key(&id);
    let status: OrderStatus = context
        .state()
        .get_json(&key)
        .await?
        .ok_or_else(|| not_found!("order `{id}` not found"))?;
    if status.locked {
        return Err(LookupError::Conflict(id));
    }
    let code = if status.settled { StatusCode::OK } else { StatusCode::ACCEPTED };
    Ok((code, Json(status)))
}
```

Where every status the handler answers is a success or one of the four, no error type is needed: `Result<(StatusCode, Json<O>)>` over `omnia_sdk::Error` answers `201` and `404` alike, and the adapter scaffolds that shape where a scenario states a status.

## A gate

A rule every route under one prefix obeys before anything else, authentication under `/api`, is one function, not a layer and not a route: the host runs a stem through its `handlers()` alone, so there is no middleware to mount it on. Where the design's rules name the prefix, the adapter lays the gate in the first stem of the slice with a route under it, and every handler under the prefix is laid calling it first; a later stem imports it from that crate (`<stem> = { path = "../<stem>" }` under its `[dependencies]`) rather than writing a second. The fixture's `customers.rs`:

```rust
use axum::Json;
use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use omnia_sdk::api::Context;
use omnia_sdk::{Config, Settings, StateStore, StateStoreExt as _, not_found};

// The settings the gate reads: `API_TOKENS`, `BEARER_PREFIX`.
#[derive(Debug, Settings)]
pub(crate) struct GateSettings {
    api_tokens: String,
    bearer_prefix: String,
}

/// Why the gate under `/api` refused a request.
#[derive(Debug, thiserror::Error)]
pub enum GateError {
    /// The request carried no accepted credential.
    #[error("unauthorized")]
    Unauthorized,
    /// The settings or a capability failed.
    #[error(transparent)]
    Host(#[from] omnia_sdk::Error),
}

impl From<anyhow::Error> for GateError {
    fn from(error: anyhow::Error) -> Self {
        Self::Host(error.into())
    }
}

impl IntoResponse for GateError {
    fn into_response(self) -> Response {
        match self {
            Self::Unauthorized => {
                let body = serde_json::json!({ "error": "unauthorized" });
                (StatusCode::UNAUTHORIZED, Json(body)).into_response()
            }
            Self::Host(inner) => inner.into_response(),
        }
    }
}

/// The gate every route under `/api` passes first, as the design's rules state it.
///
/// # Errors
///
/// [`GateError::Unauthorized`] where the request carries no accepted credential.
pub async fn authenticate<P>(
    context: &Context<P>, headers: &HeaderMap,
) -> std::result::Result<(), GateError>
where
    P: Config,
{
    let settings = GateSettings::load(context.config()).await?;
    let bearer = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix(settings.bearer_prefix.as_str()))
        .map(str::trim)
        .ok_or(GateError::Unauthorized)?;
    if settings.api_tokens.split(',').map(str::trim).any(|token| token == bearer) {
        Ok(())
    } else {
        Err(GateError::Unauthorized)
    }
}

// REQ-016 customers.read: Answers the customer by its id.
pub(crate) async fn read<P>(
    context: Context<P>, headers: HeaderMap, Path(id): Path<String>,
) -> std::result::Result<Json<ReadOutput>, GateError>
where
    P: StateStore + Config,
{
    authenticate(&context, &headers).await?;
    let customer: ReadOutput = context
        .state()
        .get_json(&customer_key(&id))
        .await?
        .ok_or_else(|| not_found!("customer `{id}`"))?;
    Ok(Json(customer))
}
```

- The gate's body is the rule as the design states it: the credential read from the headers, checked against `GateSettings`, `Ok(())` where it is accepted and `Err(GateError::Unauthorized)` where it is missing or unknown. The signature stays as laid, since every handler under the prefix calls it so.
- A gated handler takes `headers: HeaderMap` after the context, calls the gate before it reads a body or reaches a capability, and answers `std::result::Result<_, GateError>`: the gate's refusal and the host's `omnia_sdk::Error` both reach the wire through `?`, the latter through the `Host` variant.
- A route outside the prefix, `GET /health/live`, takes no `HeaderMap` and calls no gate; the design's rule names the prefix for that reason.
- A test of a gated route sends the credential, `.header(AUTHORIZATION, authorization)` over a value the seeded `GateSettings` accept, and `provider()` seeds the gate's keys beside the part's. A scenario stating the refusal sends no header, or an unknown credential, and asserts `StatusCode::UNAUTHORIZED` and the body's `error` field.
- One gate per prefix: a stem whose handlers fall under a prefix another stem's gate guards imports that gate and writes none of its own, and the adapter refuses a filled handler behind a gate still left as its stub.

## The capabilities

Each capability is a trait on the provider, bounded where it is used and reached through the context's accessor; every call is `async`, and `?` carries its failure into the handler's error. The methods, their arguments, and the types they carry are in [the omnia API](omnia-api.md), one block per trait.

| Capability | Accessor | What it reaches |
| --- | --- | --- |
| `Config` | `context.config()` | a setting of the deployment, under a field of the part's `Settings` struct |
| `StateStore` | `context.state()` | a key-value store of bytes; `get_json` and `set_json` through `StateStoreExt` |
| `BlobStore` | `context.blobs()` | objects in named containers |
| `DocumentStore` | `context.docs()` | JSON documents by id in a named store; `get_as` through `DocumentStoreExt` |
| `TableStore` | `context.tables()` | SQL over a named connection; `query_as` into a `FromRow` type through `TableStoreExt`, `Row` and `DataType` from `omnia_sdk::sql` |
| `HttpRequest` | `context.http()` | an outbound HTTP call; `fetch_json` through `HttpRequestExt` |
| `Identity` | `context.identity()` | an access token for a named identity, the name a setting of the part, never a literal |
| `Publish` | `context.publish()` | a message to a topic of `common::topic` |
| `Broadcast` | `context.broadcast()` | an event to connected WebSocket clients |
| `Vault` | `context.vault()` | a secret by key |

A handler that reads a setting, fetches, and publishes:

```rust
use axum::Json;
use axum::body::Body;
use axum::extract::Path;
use axum::http::Request;
use common::topic;
use omnia_sdk::api::Context;
use omnia_sdk::{Config, HttpRequest, HttpRequestExt as _, Message, Publish, Result, Settings, bad_gateway};

// The settings the part reads: `RATES_URL`.
#[derive(Debug, Settings)]
pub(crate) struct RatingSettings {
    rates_url: String,
}

pub(crate) async fn rate<P>(context: Context<P>, Path(currency): Path<String>) -> Result<Json<Rate>>
where
    P: Config + HttpRequest + Publish,
{
    let settings = RatingSettings::load(context.config()).await?;
    let request = Request::get(format!("{}/{currency}", settings.rates_url))
        .body(Body::empty())
        .map_err(|error| bad_gateway!("{error}"))?;
    let rate: Rate = context.http().fetch_json(request).await?;
    context.publish().send(topic::RATE_SEEN, &Message::json(&rate)?).await?;
    Ok(Json(rate))
}
```

`fetch_json` answers a non-2xx status or an unreadable body as a `bad_gateway!`; where a status needs handling of its own, `context.http().fetch(request).await?` answers the `Response<Bytes>` and the handler reads `response.status()` and `response.body()`. The capability is the one way to each of these; what a guest reaches no other way is in the [guardrails](guardrails.md).

## A probe

A readiness or health requirement probes a store with one call through the capability that reaches it, awaited: the probe passed when the call answered what a live store answers, and failed when it answered `Err`. The host bounds every call, so the probe sets no deadline of its own: no timer, no race, no `within` helper, no `PROBE_TIMEOUT_MS` setting, and the source's parallel probes run one after the other. A SQL store is probed by `SELECT 1` over `context.tables().query(..)` under the connection's constant, one row the pass and `Err` or no row the failure; a key-value store by `context.state().get(..)` of a probe key, a miss as much a pass as a hit, since the store answered. Never an `HttpRequest` to a store's address: the connection is the host's, named by a constant of the part whose value is the name the deployment binds, never a URL. The status follows the outcome, so the handler answers the `(StatusCode, Json<O>)` pair:

```rust
use axum::Json;
use axum::http::StatusCode;
use omnia_sdk::api::Context;
use omnia_sdk::{Result, StateStore, TableStore};
use serde::Serialize;

// The SQL connection the deployment binds for the orders tables.
pub(crate) const ORDERS_DB: &str = "orders";

// The key the cache probe reads; what it holds, if anything, is not read.
const PROBE: &str = "health/probe";

/// Whether each store answered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Readiness {
    /// The SQL store answered `SELECT 1` with a row.
    pub store: bool,
    /// The cache answered the probe read.
    pub cache: bool,
}

// `503` where the store did not answer.
pub(crate) async fn ready<P>(context: Context<P>) -> Result<(StatusCode, Json<Readiness>)>
where
    P: StateStore + TableStore,
{
    let store = context
        .tables()
        .query(ORDERS_DB, "SELECT 1", &[])
        .await
        .is_ok_and(|rows| !rows.is_empty());
    let cache = context.state().get(PROBE).await.is_ok();
    let status = if store { StatusCode::OK } else { StatusCode::SERVICE_UNAVAILABLE };
    Ok((status, Json(Readiness { store, cache })))
}
```

Every test of the probe arranges the store's answer on `ScriptedTables`, since a statement the double holds no rule for panics naming it: `.on_query(|sql, _| sql == "SELECT 1", vec![])` is the store down; one `Row { index: "1".into(), fields: Vec::new() }` is the store up, `Row` being `omnia_sdk::sql::Row`, so no manifest is edited for it. `Memory` answers every read, so the cache's failure is the host's to produce and no test arranges it.

```rust
// In `tests/<part>.rs`, over the scaffolded `provider()` and `call`:
use omnia_sdk::sql::Row;
use omnia_test::guest::ScriptedTables;

// REQ-004 health.ready: The store is down
//   Given the store answers no row
//   When readiness is asked
//   Then it is answered 503 with the store unready
#[tokio::test]
async fn the_store_is_down() {
    let provider =
        provider().tables(ScriptedTables::default().on_query(|sql, _| sql == "SELECT 1", vec![]));
    let request = Request::get("/health/ready").body(Body::empty()).expect("request");
    let (status, body) = call(&provider, request).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["store"], false);
}

// REQ-004 health.ready: The store is up
//   Given the store answers a row
//   When readiness is asked
//   Then it is answered 200 with the store ready
#[tokio::test]
async fn the_store_is_up() {
    let up = vec![Row {
        index: "1".into(),
        fields: Vec::new(),
    }];
    let provider =
        provider().tables(ScriptedTables::default().on_query(|sql, _| sql == "SELECT 1", up));
    let request = Request::get("/health/ready").body(Body::empty()).expect("request");
    let (status, body) = call(&provider, request).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["store"], true);
}
```

## A cache-aside read

A record read by id through a cache in front of a store is one handler: the cache read first through `get_json`, a hit answered, a miss read from the store, written to the cache through `set_json` under its TTL, and answered, and a store miss `not_found`. A miss never answers an empty record or an empty list in place of the store's read, and the write path deletes the key after the store's update rather than refreshing it. A table the source loaded whole and refreshed on a schedule is the same shape over one key, as the [translation's fragment](translation.md#a-reference-table) spells it. The fixture's `catalogue.rs`:

```rust
use axum::Json;
use axum::extract::Query;
use omnia_sdk::api::Context;
use omnia_sdk::sql::{DataType, FromRow};
use omnia_sdk::{Result, StateStore, StateStoreExt as _, TableStore, TableStoreExt as _};
use serde::{Deserialize, Serialize};

// The relational connection the part queries.
pub(crate) const CONNECTION: &str = "db";

// The state key and TTL of the cached catalogue.
pub(crate) const CATALOGUE_KEY: &str = "orders/catalogue";
const CATALOGUE_TTL_SECS: u64 = 60;

/// Optional filter on the catalogue listing.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ListCatalogue {
    /// Minimum price in cents (`0` when omitted).
    pub min_cents: Option<i64>,
}

/// One item in the catalogue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct CatalogueItem {
    /// Item SKU.
    pub sku: String,
    /// Unit price in cents.
    pub cents: i64,
    /// Whether the item is currently in stock.
    pub in_stock: bool,
}

/// The catalogue items matching the query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogueReply {
    /// Matching items ordered by SKU.
    pub items: Vec<CatalogueItem>,
}

pub(crate) async fn list_catalogue<P>(
    context: Context<P>, Query(input): Query<ListCatalogue>,
) -> Result<Json<CatalogueReply>>
where
    P: StateStore + TableStore,
{
    let min_cents = input.min_cents.unwrap_or(0);

    if min_cents == 0
        && let Some(items) = context.state().get_json(CATALOGUE_KEY).await?
    {
        return Ok(Json(CatalogueReply { items }));
    }

    let items: Vec<CatalogueItem> = context
        .tables()
        .query_as(
            CONNECTION,
            "SELECT sku, cents, in_stock FROM catalogue WHERE cents >= $1 ORDER BY sku ASC",
            &[DataType::Int64(Some(min_cents))],
        )
        .await?;

    if min_cents == 0 {
        context.state().set_json(CATALOGUE_KEY, &items, Some(CATALOGUE_TTL_SECS)).await?;
    }

    Ok(Json(CatalogueReply { items }))
}
```

Where the design makes the TTL a setting of the deployment, it is a `u64` field of the part's `Settings` struct, loaded at the moment of use, as the [layout](layout.md) has every setting. In a test, `Memory` holds what `insert_state` put there, so a hit is arranged by inserting the record's JSON bytes under the key and asserted by `provider.tables.statements()` being empty, and a miss by `on_query` answering the rows and `provider.storage.state(CATALOGUE_KEY)` holding the bytes after the call; an entry set with a TTL expires in `Memory` as it does in the store.

## A SQL query

A query over `TableStore` binds parameters as `DataType` values (`$1`, `$2`, ..) and reads its rows into a type deriving `omnia_sdk::sql::FromRow` through `query_as`, each field read from the column of its name; a single column is `row.get::<T>("column")?` over the `Row`s `query` answers. Never read `row.fields` by hand, and never call `.as_i64()` on a `Field`. A test seeds a column as the schema types it — `DataType::Int64` for an `INTEGER` column, `DataType::Double` for a `NUMERIC` one — and the struct's field is the same kind, since a row read into `f64` from an `Int64` column, or the reverse, fails the read with "is not a float":

```rust
use omnia_sdk::api::Context;
use omnia_sdk::sql::{DataType, FromRow};
use omnia_sdk::{Result, TableStore, TableStoreExt as _, not_found};
use serde::Serialize;

// The relational connection the part queries.
pub(crate) const DB: &str = "db";

/// A customer as the table holds one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, FromRow)]
pub struct Customer {
    /// The customer's id.
    pub id: String,
    /// The customer's email.
    pub email: String,
}

pub(crate) async fn read_customer<P: TableStore>(provider: &P, id: &str) -> Result<Customer> {
    let mut customers: Vec<Customer> = provider
        .query_as(
            DB,
            "SELECT id, email FROM customers WHERE id = $1",
            &[DataType::Str(Some(id.to_owned()))],
        )
        .await?;
    customers.pop().ok_or_else(|| not_found!("no customer `{id}`"))
}

pub(crate) async fn count_orders<P: TableStore>(context: &Context<P>, id: &str) -> Result<i64> {
    let rows = context
        .tables()
        .query(DB, "SELECT COUNT(*) AS orders FROM orders WHERE customer_id = $1", &[
            DataType::Str(Some(id.to_owned())),
        ])
        .await?;
    let row = rows.first().ok_or_else(|| not_found!("no customer `{id}`"))?;
    Ok(row.get::<i64>("orders")?)
}
```

A write is `context.tables().exec(DB, "UPDATE ..", &[..]).await?`, answering the rows affected. In a test, `ScriptedTables::default().on_query(|sql, params| sql.starts_with("SELECT id, email") && matches!(params, [DataType::Str(Some(id))] if id == "c-1"), vec![Row { index: "0".to_owned(), fields: vec![Field { name: "id".to_owned(), value: DataType::Str(Some("c-1".to_owned())) }, ..] }])` scripts the rows, `Row`, `Field`, and `DataType` from `omnia_sdk::sql`, and `provider.tables.statements()` records what ran.

## A compare-and-swap update

An optimistic-concurrency update reads the current bytes with `context.state().get(..)`, checks the caller's expected version or ETag, and writes the updated bytes through `context.state().cas(&key, Some(&current), &next)`. Because `omnia_sdk::Error` has no `409` variant, map `Err(CasError::Conflict(_))` (and a version mismatch) to `StatusCode::CONFLICT` through a module error type as "A status of its own" shows:

```rust
use axum::Json;
use axum::extract::Path;
use omnia_sdk::api::Context;
use omnia_sdk::{CasError, StateStore, not_found, server_error};

pub(crate) async fn amend_order<P>(
    context: Context<P>, Path(id): Path<String>, Json(input): Json<AmendOrder>,
) -> Result<Json<Order>, AmendError>
where
    P: StateStore,
{
    let key = order_key(&id);
    let current = context
        .state()
        .get(&key)
        .await?
        .ok_or_else(|| not_found!("no order `{id}`"))?;
    let mut order: Order = serde_json::from_slice(&current).map_err(|error| server_error!("{error}"))?;
    if order.version != input.expected_version {
        return Err(AmendError::Conflict(id));
    }
    order.version += 1;
    order.items = input.items;
    let next = serde_json::to_vec(&order).map_err(|error| server_error!("{error}"))?;
    match context.state().cas(&key, Some(&current), &next).await {
        Ok(()) => Ok(Json(order)),
        Err(CasError::Conflict(_)) => Err(AmendError::Conflict(id)),
        Err(CasError::Store(error)) => Err(server_error!("{error}").into()),
    }
}
```

## A skipped message

A requirement that says a delivery is skipped, ignored, or discarded, or produces no events — a message about a station the service does not track, a type it does not handle, a vehicle it has no stop for — is a valid delivery the rule filters out, not a rejection. The consumer answers `Ok(())` after one `tracing` event counting the skip, and publishes nothing; it never answers `Err`, which the host records as a rejected delivery and redelivers or dead-letters. `Err` is the malformed or invalid delivery's alone — a payload the decoder cannot read, a field outside its range — as the `A topic` bullets say:

```rust
use omnia_sdk::api::Context;
use omnia_sdk::{Message, Publish, Result, StateStore, StateStoreExt as _};

pub(crate) async fn on_position<P>(context: Context<P>, input: Position) -> Result<()>
where
    P: StateStore + Publish,
{
    let Some(stop) = context.state().get_json::<Stop>(&stop_key(&input.station)).await? else {
        tracing::info!(station = %input.station, "position skipped: no stop tracked");
        return Ok(());
    };
    let arrival = Arrival::at(&stop, &input);
    context.publish().send(topic::ARRIVAL_ESTIMATED, &Message::json(&arrival)?).await?;
    Ok(())
}
```

The test for the skip delivers the message over a provider seeded with no stop, asserts the delivery is accepted, and asserts nothing was published; the test for the refusal delivers a malformed payload and asserts the `Err`.

## Ids and time

An id the design leaves to the program is `uuid::Uuid::new_v4().to_string()`, and a timestamp is `chrono::Utc::now()`, both from crates the root manifest pins and the stem's manifest names with `.workspace = true`. A key in the state store joins the stem and the id, `format!("orders/order/{id}")`, through one key function the module's readers and writers share, `pub(crate)` within the stem and `pub` where another stem reads the same entries.
