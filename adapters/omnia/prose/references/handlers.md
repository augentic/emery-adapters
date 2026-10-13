# Handlers

A requirement lands as a handler in the slice's module and the lines that wire it: a constant in a table of `common`, a statement in a root list, an entry in a root rung's table, as the [layout](layout.md) places them. The shapes below compile against the `omnia-sdk` the root manifest pins; use them as they stand, and read the signatures behind them in [the omnia API](omnia-api.md) rather than probing the crates.

## A route

A requirement that states a method and a path is one route, in four lines outside the module:

```rust
// crates/common/src/routes/http.rs, at the end
/// Place an order.
pub const ORDERS: &str = "/orders";
```

```rust
// src/routes.rs, before `router.with_state(..)`
    let router = router.route(
        common::routes::http::ORDERS,
        omnia_sdk::api::http::post(orders::placing::place_order),
    );
```

```rust
// tests/routes.rs, before the table's `];`
    (Method::POST, common::routes::http::ORDERS),
```

and the stem's manifest naming `orders` is the root's, `orders.workspace = true`, at the end of the root manifest's `[dependencies]`, once per stem.

- A path parameter is `{name}` in the path and a field of that name on the input; which route function decodes the input from where is stated beside its signature in [the omnia API](omnia-api.md). Two methods on one path share one constant: two statements and two table entries over it, each slice adding its own.
- The handler's `Ok` is answered `200` with the output as JSON. For another status or body, the stem exports a route function of its own over `handle_with(filter, handler, decode, encode)`, which takes the method, a decoder from the raw request to the input, and an encoder from the output to the response, and the list's statement names it in place of a route function of `omnia_sdk::api::http`:

```rust
use axum::Json;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{MethodFilter, MethodRouter};
use omnia_sdk::api::Client;
use omnia_sdk::api::http::{DecodeError, RawRequest, handle_with};

/// The `POST /orders` route: a placed order is answered `201`.
#[must_use]
pub fn place_order_route<P>() -> MethodRouter<Client<P>>
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
```

```rust
    let router = router.route(common::routes::http::ORDERS, orders::placing::place_order_route());
```

A `204` is `|(): ()| StatusCode::NO_CONTENT.into_response()` over a handler answering `Ok(())`. The bounds on `P` are the handler's capabilities with `Send + Sync + 'static` after them; the root's `Capabilities` satisfies every such bound.

## A topic

A requirement that consumes a topic is one topic, in the same four lines: a constant in the topic table, a statement in the topic list, an entry in the messaging rung's table, and the stem named in the root manifest's `[dependencies]`.

```rust
// crates/common/src/routes/topic.rs, at the end
/// An order was placed.
pub const ORDER_PLACED: &str = "orders-placed.v1";
```

```rust
// src/messaging.rs, before `tracing::debug!(..)`
    let router = router.route(
        config::topic_for(&env, common::routes::topic::ORDER_PLACED),
        messaging::consume(orders::fulfilment::on_placed),
    );
```

```rust
// tests/messaging.rs, before the table's `];`
    common::routes::topic::ORDER_PLACED,
```

The handler is a `Handler` as a route's is, its input the payload decoded as JSON and its output dropped:

```rust
use omnia_sdk::api::Context;
use omnia_sdk::{StateStore, server_error};

/// Reserve stock for a placed order.
///
/// # Errors
///
/// Fails when the order cannot be stored.
pub async fn on_placed<P>(input: Order, context: Context<P>) -> omnia_sdk::Result<()>
where
    P: StateStore,
{
    let bytes = serde_json::to_vec(&input).map_err(|error| server_error!("order `{}`: {error}", input.id))?;
    context.provider().set(&key(&input.id), &bytes, None).await?;
    Ok(())
}
```

- A payload that is not JSON is decoded by a `pub fn` the stem exports beside the handler, `fn(&Delivery) -> Result<I, DecodeError>`, and the statement registers the pair through `messaging::consume_with(<handler>, <decode>)`.
- The suffix never carries the environment: the list qualifies it as `{env}-<suffix>` through `config::topic_for`, and a publisher through `config::topic`, so the constant is the same on both sides.
- A handler that publishes names the constant its consumer is registered under, and the slice that consumes declares it; where no slice consumes, the publisher declares it. The requirement that consumes a topic owns the handler; the one that publishes owns the `send`.

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

/// Read one order.
///
/// # Errors
///
/// `not_found` when no order has the id; `server_error` when the stored order does not read back.
pub async fn get_order<P>(input: GetOrder, context: Context<P>) -> omnia_sdk::Result<Order>
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

- A handler is `pub async fn name<P>(input: I, context: Context<P>) -> omnia_sdk::Result<O>`: `I` a struct deriving `Deserialize`, one field per path parameter, query parameter, or body field; `O` a struct deriving `Serialize`. A route with no input takes an empty struct, `pub struct HealthQuery {}`. The input is taken by value, so destructure it (`let PlaceOrder { items } = input;`) rather than reading its fields through a reference the lints flag.
- Every `pub` item carries a `///`: the handler, each type, each field, each helper; a fallible `pub fn` carries `# Errors` naming when it fails; a `pub fn` that answers a value and reaches nothing carries `#[must_use]`. The workspace lints deny their absence.
- The design's types are `pub` structs deriving `Debug`, `Clone`, `Serialize`, and `Deserialize`, declared once in the module of the slice that owns the type key and used from there by every other module, through the stem's crate. What a module stores is the type serialised with `serde_json`, and what it reads back is the same type deserialised.
- The host is reached through `context.provider()` alone, bounded on the handler as `P: StateStore` or whichever capabilities it uses, each a trait of `omnia_sdk`. Where two bounded capabilities share a method name — `Config::get` beside `StateStore::get` — call the trait's method by name: `StateStore::get(context.provider(), &key)`.
- A setting is read by the key the configuration catalog declares, `Config::get(context.provider(), common::config::RATES_URL)`, never by a literal; the slice that first reads a key adds its constant to the catalog, after `ENV`.
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
| `Config` | a setting of the deployment, under a key of `common::config` |
| `StateStore` | a key-value store of bytes |
| `BlobStore` | objects in named containers |
| `DocumentStore` | JSON documents by id in a named store |
| `TableStore` | SQL over a named connection; `Row` and `DataType` are the `omnia-wasi-sql` crate's, named in the stem's manifest at the version the root pins `omnia-sdk` |
| `HttpRequest` | an outbound HTTP call |
| `Identity` | an access token for a named identity |
| `Publish` | a message to a topic of `common::routes::topic`, qualified through `config::topic` |
| `Broadcast` | an event to connected WebSocket clients |

A handler that reads a setting, fetches, and publishes:

```rust
use axum::body::Body;
use axum::http::Request;
use common::{config, routes};
use omnia_sdk::api::Context;
use omnia_sdk::{Config, HttpRequest, Message, Publish, bad_gateway};

/// Read the rate of a currency from the upstream and announce it.
///
/// # Errors
///
/// `bad_gateway` when the upstream cannot be reached or answers badly.
pub async fn rate<P>(input: RateQuery, context: Context<P>) -> omnia_sdk::Result<Rate>
where
    P: Config + HttpRequest + Publish,
{
    let RateQuery { currency } = input;
    let provider = context.provider();
    let base = Config::get(provider, config::RATES_URL).await?;
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
    let topic = config::topic(provider, routes::topic::RATE_SEEN).await;
    Publish::send(provider, &topic, &Message::new(&payload)).await?;
    Ok(rate)
}
```

The capability is the one way to each of these; what a guest reaches no other way is in the [guardrails](guardrails.md).

## Ids and time

An id the design leaves to the program is `uuid::Uuid::new_v4().to_string()`, and a timestamp is `chrono::Utc::now()`, both from crates the root manifest pins and the stem's manifest names with `.workspace = true`. A key in the state store joins the stem and the id, `format!("orders/{id}")`, through one `pub fn key` the module's readers and writers share.
