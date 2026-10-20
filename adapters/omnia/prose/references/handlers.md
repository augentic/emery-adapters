# Handlers

A requirement lands as a handler in the slice's module and the lines that wire it: a path or a setting as a constant of the part, a `.route(..)` in the part's `routes()` or `topics(..)`, the part's entry in its stem's lists, and the stem's statements in the root lists, as the [layout](layout.md) places them. The shapes below compile against the `omnia-sdk` the root manifest pins; use them as they stand, and read the signatures behind them in [the omnia API](omnia-api.md) rather than probing the crates.

## A route

A requirement carrying a route, its `Route:` line, is one route, in the slice's module and one entry of its stem's routes list:

```rust
// crates/orders/src/placing.rs
use common::Capabilities;
use omnia_sdk::api::Client;
use omnia_sdk::api::http::post;

// The paths the part serves.
pub(crate) const ORDERS: &str = "/orders";

pub(crate) fn routes<P: Capabilities>() -> axum::Router<Client<P>> {
    axum::Router::new().route(ORDERS, post(place_order))
}
```

```rust
// crates/orders/src/lib.rs, the routes array, after its comment
        placing::routes(),
```

and the stem's statement in the root route list, `let router = router.merge(orders::router());`, and its two lines in the root manifest, `orders = { path = "crates/orders" }` after `common`'s and `orders.workspace = true` at the end, each once per stem, written by the first slice of the stem and found in the tree by every other.

- A path parameter is `{name}` in the path and a field of that name on the input; which route function decodes the input from where is stated beside its signature in [the omnia API](omnia-api.md). Two methods on one path are two `.route(..)` calls over one constant in one part, or one `.route(ORDERS, get(list_orders).post(place_order))`; two parts of one stem never serve one path.
- The handler's `Ok` is answered `200` with the output as JSON. For another status or body, the part declares a route function of its own over `handle_with(filter, handler, decode, encode)`, which takes the method, a decoder from the raw request to the input, and an encoder from the output to the response, and `routes()` names it in place of a route function of `omnia_sdk::api::http`. The adapter scaffolds one, `<name>_route`, for every `POST` handler and for every handler whose scenarios state a status other than `200`, over a `decode_json` of the part that reads the body as `post` does and an encoder spelling `StatusCode::OK`, so a `201` or a `503` is that one token changed; for any other handler the part declares the function as shown:

```rust
use axum::Json;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{MethodFilter, MethodRouter};
use omnia_sdk::api::Client;
use omnia_sdk::api::http::{DecodeError, RawRequest, handle_with};

// The `POST /orders` route: a placed order is answered `201`.
pub(crate) fn place_order_route<P>() -> MethodRouter<Client<P>>
where
    P: Config + Publish + StateStore + Send + Sync + 'static,
{
    handle_with(
        MethodFilter::POST,
        place_order,
        |raw: RawRequest<'_>| -> Result<PlaceOrder, DecodeError> {
            serde_json::from_slice(raw.body).map_err(|error| DecodeError::new(error.to_string()))
        },
        |order: Order| (StatusCode::CREATED, Json(order)).into_response(),
    )
}

pub(crate) fn routes<P: Capabilities>() -> axum::Router<Client<P>> {
    axum::Router::new().route(ORDERS, place_order_route())
}
```

A `204` is `|(): ()| StatusCode::NO_CONTENT.into_response()` over a handler answering `Ok(())`. The bounds on `P` are the handler's capabilities with `Send + Sync + 'static` after them; `common::Capabilities` satisfies every such bound, so `routes()` and `topics(..)` bound on it alone.

## A topic

A requirement that consumes a topic is one topic: a constant in the topic table where no slice has declared it, a `.route(..)` in the part's `topics(..)`, and one entry of the stem's topics list.

```rust
// crates/common/src/topic.rs, at the end
/// An order was placed.
pub const ORDER_PLACED: &str = "orders-placed.v1";
```

```rust
// crates/orders/src/fulfilment.rs
use common::{Capabilities, config, topic};
use omnia_sdk::api::messaging;

pub(crate) fn topics<P: Capabilities>(
    router: messaging::Router<P>, env: &str,
) -> messaging::Router<P> {
    router.route(config::topic_for(env, topic::ORDER_PLACED), messaging::consume(on_placed))
}
```

```rust
// crates/orders/src/lib.rs, the topics array, after its comment
        fulfilment::topics,
```

and the stem's statement in the root topic list, `let router = orders::topics(router, &env);`, once per stem. The handler is a `Handler` as a route's is, its input the payload decoded as JSON and its output dropped:

```rust
use omnia_sdk::api::Context;
use omnia_sdk::{StateStore, server_error};

pub(crate) async fn on_placed<P>(input: Order, context: Context<P>) -> omnia_sdk::Result<()>
where
    P: StateStore,
{
    let bytes = serde_json::to_vec(&input).map_err(|error| server_error!("order `{}`: {error}", input.id))?;
    context.provider().set(&key(&input.id), &bytes, None).await?;
    Ok(())
}
```

- A payload that is not JSON is decoded by a `fn(&Delivery) -> Result<I, DecodeError>` of the part, and `topics(..)` registers the pair through `messaging::consume_with(on_notice, decode_notice)`. A malformed wire payload fails in the decoder with `Err(DecodeError::new(..))`. `I` is the wire's root type itself — the one a stage slice of the stem declared where it owns the type key — read straight from `&delivery.payload` through the format's `serde` reader or the stage's own parse function; the decoder never hands the handler the raw bytes or text under a carrier of its own (`struct InboundBody { xml: String }`) for the handler to parse again.
- A topic handler returns `Err(omnia_sdk::Error)` when a delivery is rejected or cannot be processed — `bad_request!` when validation or timeliness rejects the message, `bad_gateway!` when an upstream fails — rather than catching the error and returning `Ok(())`: the host owns the consumer loop and advances past a rejected delivery on `Err`.
- The suffix never carries the environment: `topics(..)` qualifies it as `{env}-<suffix>` through `config::topic_for` over the `env` the root resolved once, and a publisher through `config::topic`, so the constant is the same on both sides.
- A handler that publishes names the constant its consumer is registered under, and the slice that consumes declares it; where no slice consumes, the publisher declares it. The requirement that consumes a topic owns the handler; the one that publishes owns the `send`. Two parts of one stem never consume one topic.

## The handler

```rust
use omnia_sdk::api::Context;
use omnia_sdk::{StateStore, not_found, server_error};
use serde::{Deserialize, Serialize};

/// The input of `GET /orders/{id}`: the path parameter.
#[derive(Debug, Deserialize)]
pub struct GetOrder {
    /// The order's id.
    pub id: String,
}

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

pub(crate) async fn get_order<P>(input: GetOrder, context: Context<P>) -> omnia_sdk::Result<Order>
where
    P: StateStore,
{
    let GetOrder { id } = input;
    let bytes = context
        .provider()
        .get(&key(&id))
        .await?
        .ok_or_else(|| not_found!("no order `{id}`"))?;
    serde_json::from_slice(&bytes).map_err(|error| server_error!("order `{id}`: {error}"))
}

/// The state-store key of an order.
#[must_use]
pub fn key(id: &str) -> String {
    format!("orders/{id}")
}
```

- A handler is `pub(crate) async fn name<P: Capabilities>(input: I, context: Context<P>) -> omnia_sdk::Result<O>`, scaffolded by the adapter for each route a requirement states over an empty `I` deriving `Deserialize` and an empty `O` deriving `Serialize`, its body a `todo!("REQ-.. <subject>: the handler")` under a `let _ = (&input, &context);` line. Fill `I` with one field per path parameter, which the adapter scaffolded as a `String`, query parameter, or body field, and `O` with the response's fields; a route with no input keeps the empty struct. Replace the two lines together with the body, as one `edit_files` edit whose `find` is the block the brief quotes; the bound `P: Capabilities` narrows to the capabilities the body uses, or stays. The input is taken by value, so destructure it (`let PlaceOrder { items } = input;`) rather than reading its fields through a reference the lints flag. A handler is reached through the stem's `router()` or `topics(..)` alone, so it is `pub(crate)`, as the part's paths, settings, state keys, `routes()`, and `topics(..)` are; what another module uses — a type, a key function, a rule — is `pub`. A topic's handler, which the adapter does not scaffold, takes the same shape over the payload's type.
- Every `pub` item carries a `///`: each type, each field, each helper; a fallible `pub fn` carries `# Errors` naming when it fails; a `pub fn` that answers a value and reaches nothing carries `#[must_use]`. The workspace lints deny their absence.
- The design's types are `pub` structs and enums deriving `Debug`, `Clone`, `PartialEq`, `Eq`, `Serialize`, and `Deserialize`, declared once in the module of the slice that owns the type key and used from there by every other module, through the stem's crate. The adapter lays each from the design's signature, fenced in the type's doc: a TypeScript interface, object alias, or class, a Python model, dataclass, or typed dict, or a Rust struct as a `pub struct` of documented fields, a `string` or `str` a `String`, a `number` an `i64` where the field's last word counts (`cents`, `count`, `id`, `index`, `page`, `size`, `total`, `quantity`, `seconds`, `millis`) and an `f64` otherwise, an `int` an `i64`, a `float` an `f64`, a `Date` or `datetime` a `String`, an array a `Vec`, a `Record` or `dict` a `BTreeMap<String, ..>`, an optional field or a `None` default an `Option`, `unknown` or `Any` a `serde_json::Value`, a field's wire name kept through `#[serde(rename = "..")]` over its snake_case spelling, a Python default a `#[serde(default = "default_<field>")]` over a fn beside the struct; an enumeration — a Rust `enum`, a union of string literals, a TypeScript `enum`, a class over `Enum` — as a `pub enum` of PascalCase variants, each `#[serde(rename = "<literal>")]` where the wire carries one; a field typed by a design type the slice owns by that type's name, and by one another slice declares as a `serde_json::Value` under a doc naming the design type, since that slice's module is not in the tree. `Eq` is dropped where a field is a float or holds a type that dropped it, since `assert_eq!` in a test wants `PartialEq` and `f64` has no `Eq`. A signature the adapter cannot lay whole — an alias inferred from a schema, a union of object types, a table — is an empty `pub struct {}` or `pub enum {}` under the fenced signature, the brief's marker, which the slice fills by the same rules. What a module stores is the type serialised with `serde_json`, and what it reads back is the same type deserialised.
- A type that crosses the wire — a message published, a payload consumed, an upstream's response read — is declared in the wire's shape, as the design's `apis` section spells it: each field at the path and nesting the wire carries, a nested object a struct of its own, a constant the wire carries a field set to it, and a field whose wire name is not a Rust identifier, or not snake_case, carried through `#[serde(rename = "..")]` — `#[serde(rename = "orderId")] pub order_id: String`, `#[serde(rename = "shippingAddress")] pub shipping_address: ShippingAddress` — never renamed to read better in Rust, since the consumer on the other side reads the wire, not the struct. Even when the design's `apis` section spells `response.data` or `data.all[].carrierId` (extracted from Axios in TypeScript), `data` is the HTTP client's property on its response object and **never** a JSON field on the wire, and the adapter refuses any `Deserialize` struct whose body holds a single `data` field: a JSON array endpoint (`/rates`) deserializes `response.body()` directly into `Vec<Rate>`, and a JSON object endpoint (`/shipments/allocations`) deserializes its fields (`pub all: Vec<CarrierAllocation>`) at the root of the struct, never inside `struct ... { data: ... }`. In `quick_xml::de::from_str::<T>` and `from_slice::<T>`, `T` itself is the root XML element (`<Envelope>`), and `T`'s fields are that root element's direct children (`#[serde(rename = "ShipmentNotice")] pub notice: Option<ShipmentNotice>`); never wrap `T` in an outer struct with `#[serde(rename = "Envelope")]`, which makes `quick_xml` expect `<Outer><Envelope>...` and fail on every valid `<Envelope><ShipmentNotice>...` payload. An element the wire repeats under one name — the items of an order, the legs of a shipment — is a plain `Vec<T>` field under `#[serde(default)]` and nothing else: the XML reader collects repeated siblings into a sequence for a plain `Vec` alone and reports the second as a duplicate field under a `deserialize_with`, so a test of the type parses a payload holding two of them. An element the design types `boolean` is a `bool` under `#[serde(default)]`, never an `i32`, since the wire spells `true` and `false`. Every field is one the wire carries, named as the wire names it and placed on the element that carries it, never invented or lifted to the root, as the [translation](translation.md)'s root-element item spells out.
- A requirement that states a count, a measurement, or a level the operator observes is one `tracing` event where the behaviour happens, read by the host's metrics layer through its field's prefix: `tracing::info!(monotonic_counter.orders.rejected_expired = 1)` for a count, `tracing::info!(histogram.orders.fulfilment_latency_ms = elapsed)` for a measurement, `tracing::info!(gauge.catalog.rates_cached = rates.len())` for a level, the name in snake_case under the stem; no counter type, no `StateStore` key, no exporter.
- The host is reached through `context.provider()` alone, bounded on the handler as `P: StateStore` or whichever capabilities it uses, each a trait of `omnia_sdk`. Where two bounded capabilities share a method name — `Config::get` beside `StateStore::get` — call the trait's method by name: `StateStore::get(context.provider(), &key)`.
- A setting is read by a constant the part declares, `pub(crate) const RATES_URL: &str = "RATES_URL";`, its value its name and a `//` above it naming what the deployment provides, through `Config::get(context.provider(), RATES_URL)`, never by a literal; two parts that read one setting each declare it under the same name, and no part reads a setting another declares. The identity `Identity::access_token` is asked for is such a setting — `pub(crate) const UPSTREAM_IDENTITY: &str = "UPSTREAM_IDENTITY";`, read through `Config::get` and handed on, `Identity::access_token(provider, Config::get(provider, UPSTREAM_IDENTITY).await?).await?` — never a literal name. What is a setting and what is a constant of the part is the [layout](layout.md)'s rule.
- A failure is a classified error, never a panic: `bad_request!`, `not_found!`, `server_error!`, and `bad_gateway!` each take a format string and build an `omnia_sdk::Error`, answered as `400`, `404`, `500`, or `502` with the JSON body `{"error": "not_found", "message": ".."}`. A provider call answers `anyhow::Result`, so `?` on it carries the host's failure into the handler's error as a `server_error`, and a handler that ends on such a call closes with `Ok(())` after it rather than returning the call's result.
- A status outside those four is an error type of the module's: an enum deriving `thiserror::Error`, with a variant wrapping `omnia_sdk::Error` under `#[from]` so `?` still works, converted into the response by `From<ThatError> for omnia_sdk::HttpError`. The handler answers `Result<O, ThatError>`; `thiserror` is pinned in the root manifest and named in the stem's manifest as every other crate is:

```rust
use axum::http::StatusCode;
use omnia_sdk::HttpError;

/// Why an order cannot be cancelled.
#[derive(Debug, thiserror::Error)]
pub enum CancelError {
    /// The order has shipped.
    #[error("order `{0}` is shipped")]
    Shipped(String),
    /// The host failed.
    #[error(transparent)]
    Host(#[from] omnia_sdk::Error),
}

impl From<CancelError> for HttpError {
    fn from(error: CancelError) -> Self {
        match error {
            CancelError::Shipped(_) => Self::new(StatusCode::CONFLICT, error.to_string()),
            CancelError::Host(inner) => inner.into(),
        }
    }
}
```

## The capabilities

Each capability is a trait on the provider, bounded where it is used and called through `context.provider()`; every call is `async`, and `?` carries its failure into the handler's error. The methods, their arguments, and the types they carry are in [the omnia API](omnia-api.md), one block per trait.

| Capability | What it reaches |
| --- | --- |
| `Config` | a setting of the deployment, under a constant of the part that reads it |
| `StateStore` | a key-value store of bytes |
| `BlobStore` | objects in named containers |
| `DocumentStore` | JSON documents by id in a named store |
| `TableStore` | SQL over a named connection; `Row` and `DataType` are the `omnia-wasi-sql` crate's, named in the stem's manifest as `omnia-wasi-sql.workspace = true` |
| `HttpRequest` | an outbound HTTP call |
| `Identity` | an access token for a named identity, the name read through `Config::get` under a setting of the part, never a literal |
| `Publish` | a message to a topic of `common::topic`, qualified through `config::topic` |
| `Broadcast` | an event to connected WebSocket clients |

A handler that reads a setting, fetches, and publishes:

```rust
use axum::body::Body;
use axum::http::Request;
use common::{config, topic};
use omnia_sdk::api::Context;
use omnia_sdk::{Config, HttpRequest, Message, Publish, bad_gateway};

// Where the rates upstream answers.
pub(crate) const RATES_URL: &str = "RATES_URL";

pub(crate) async fn rate<P>(input: RateQuery, context: Context<P>) -> omnia_sdk::Result<Rate>
where
    P: Config + HttpRequest + Publish,
{
    let RateQuery { currency } = input;
    let provider = context.provider();
    let base = Config::get(provider, RATES_URL).await?;
    let request = Request::get(format!("{base}/{currency}"))
        .body(Body::empty())
        .map_err(|error| bad_gateway!("{error}"))?;
    let response = HttpRequest::fetch(provider, request).await?;
    if !response.status().is_success() {
        return Err(bad_gateway!("rates upstream answered {}", response.status()));
    }
    let rate: Rate = serde_json::from_slice(response.body())
        .map_err(|error| bad_gateway!("rates upstream body: {error}"))?;
    let payload = serde_json::to_vec(&rate).map_err(|error| bad_gateway!("{error}"))?;
    let topic = config::topic(provider, topic::RATE_SEEN).await;
    Publish::send(provider, &topic, &Message::new(&payload)).await?;
    Ok(rate)
}
```

The capability is the one way to each of these; what a guest reaches no other way is in the [guardrails](guardrails.md).

## Ids and time

An id the design leaves to the program is `uuid::Uuid::new_v4().to_string()`, and a timestamp is `chrono::Utc::now()`, both from crates the root manifest pins and the stem's manifest names with `.workspace = true`. A key in the state store joins the stem and the id, `format!("orders/{id}")`, through one `key` function the module's readers and writers share, `pub(crate)` within the stem and `pub` where another stem reads the same entries.
