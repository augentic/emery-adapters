# Handlers

A requirement lands as a route and the handler behind it, over the `omnia-sdk` the root manifest of the [layout](layout.md) pins. The shapes below compile against that pin; use them as they stand, and read the signatures behind them in [the omnia API](omnia-api.md) rather than probing the crates.

## The route

A module's `mount` merges a router of its own, one `route` per path, each method a route function of `omnia_sdk::api::http` over a handler:

```rust
use axum::Router;
use omnia_sdk::api::Client;
use omnia_sdk::api::http::{delete, get, post};

pub fn mount<P>(router: Router<Client<P>>) -> Router<Client<P>>
where
    P: StateStore + Send + Sync + 'static,
{
    router.merge(
        Router::new()
            .route("/orders", post(place_order))
            .route("/orders/{id}", get(get_order).merge(delete(cancel_order))),
    )
}
```

- A path parameter is `{name}` in the path and a field of that name on the input; which route function decodes the input from where is stated beside its signature in [the omnia API](omnia-api.md). Two methods on one path are two route functions merged, as above.
- The handler's `Ok` is answered `200` with the output as JSON. For another status or body, `handle_with(filter, handler, decode, encode)` takes the method, a decoder from the raw request to the input, and an encoder from the output to the response:

```rust
use axum::Json;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::MethodFilter;
use omnia_sdk::api::http::{DecodeError, RawRequest, handle_with};

let place = handle_with(
    MethodFilter::POST,
    place_order,
    |raw: RawRequest<'_>| -> Result<PlaceOrder, DecodeError> {
        serde_json::from_slice(raw.body).map_err(|error| DecodeError::new(error.to_string()))
    },
    |order: Order| (StatusCode::CREATED, Json(order)).into_response(),
);
router.merge(Router::new().route("/orders", place))
```

A `204` is `|(): ()| StatusCode::NO_CONTENT.into_response()` over a handler answering `Ok(())`.

- The bounds on `P` in `mount` are the capabilities the module's handlers use, `Send + Sync + 'static` after them; a module that reaches no capability bounds `P: Send + Sync + 'static` alone.

## The handler

```rust
use omnia_sdk::api::Context;
use omnia_sdk::{StateStore, not_found, server_error};
use serde::{Deserialize, Serialize};

/// The input of `GET /orders/{id}`: the path parameter.
#[derive(Debug, Deserialize)]
pub struct GetOrder {
    pub id: String,
}

/// An order as the design states it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Order {
    pub id: String,
    pub items: Vec<String>,
    pub placed_at: chrono::DateTime<chrono::Utc>,
}

pub async fn get_order<P>(input: GetOrder, context: Context<P>) -> omnia_sdk::Result<Order>
where
    P: StateStore,
{
    let bytes = context
        .provider()
        .get(&key(&input.id))
        .await?
        .ok_or_else(|| not_found!("no order `{}`", input.id))?;
    serde_json::from_slice(&bytes).map_err(|error| server_error!("order `{}`: {error}", input.id))
}
```

- A handler is `pub async fn name<P>(input: I, context: Context<P>) -> omnia_sdk::Result<O>`: `I` a struct deriving `Deserialize`, one field per path parameter, query parameter, or body field; `O` a struct deriving `Serialize`. A route with no input takes an empty struct, `pub struct HealthQuery {}`.
- The design's types are `pub` structs deriving `Debug`, `Clone`, `Serialize`, and `Deserialize`, declared once in the module of the slice that owns the type key and used from there by every other module. What a module stores is the type serialised with `serde_json`, and what it reads back is the same type deserialised.
- The host is reached through `context.provider()` alone, bounded on the handler as `P: StateStore` or whichever capabilities it uses. Where two bounded capabilities share a method name — `Config::get` beside `StateStore::get` — call the trait's method by name: `StateStore::get(context.provider(), &key)`.
- A failure is a classified error, never a panic: `bad_request!`, `not_found!`, `server_error!`, and `bad_gateway!` each take a format string and build an `omnia_sdk::Error`, answered as `400`, `404`, `500`, or `502` with the JSON body `{"error": "not_found", "message": ".."}`. A provider call answers `anyhow::Result`, so `?` on it carries the host's failure into the handler's error as a `server_error`, and a handler that ends on such a call closes with `Ok(())` after it rather than returning the call's result.
- A status outside those four is an error type of the module's: an enum deriving `thiserror::Error`, with a variant wrapping `omnia_sdk::Error` under `#[from]` so `?` still works, converted into the response by `From<ThatError> for omnia_sdk::HttpError`. The handler answers `Result<O, ThatError>`; `thiserror` is pinned in the root manifest and named in the stem's manifest as every other crate is:

```rust
use axum::http::StatusCode;
use omnia_sdk::HttpError;

#[derive(Debug, thiserror::Error)]
pub enum CancelError {
    #[error("order `{0}` is shipped")]
    Shipped(String),
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
| `Config` | a setting of the deployment |
| `StateStore` | a key-value store of bytes |
| `BlobStore` | objects in named containers |
| `DocumentStore` | JSON documents by id in a named store |
| `TableStore` | SQL over a named connection; `Row` and `DataType` are the `omnia-wasi-sql` crate's, named in the stem's manifest at the version the root pins `omnia-sdk` |
| `HttpRequest` | an outbound HTTP call |
| `Identity` | an access token for a named identity |
| `Publish` | a message to a topic |
| `Broadcast` | an event to connected WebSocket clients |

A handler that fetches, reads a setting, and publishes:

```rust
use axum::body::Body;
use axum::http::Request;
use omnia_sdk::api::Context;
use omnia_sdk::{Config, HttpRequest, Message, Publish, bad_gateway};

pub async fn rate<P>(input: RateQuery, context: Context<P>) -> omnia_sdk::Result<Rate>
where
    P: Config + HttpRequest + Publish,
{
    let base = Config::get(context.provider(), "RATES_URL").await?;
    let request = Request::get(format!("{base}/{}", input.currency))
        .body(Body::empty())
        .map_err(|error| bad_gateway!("{error}"))?;
    let response = HttpRequest::fetch(context.provider(), request).await?;
    if !response.status().is_success() {
        return Err(bad_gateway!("rates upstream answered {}", response.status()));
    }
    let rate: Rate = serde_json::from_slice(response.body())
        .map_err(|error| bad_gateway!("rates upstream body: {error}"))?;
    let payload = serde_json::to_vec(&rate).map_err(|error| bad_gateway!("{error}"))?;
    Publish::send(context.provider(), "rates.seen", &Message::new(&payload)).await?;
    Ok(rate)
}
```

The capability is the one way to each of these; what a guest reaches no other way is in the [guardrails](guardrails.md).

## Ids and time

An id the design leaves to the program is `uuid::Uuid::new_v4().to_string()`, and a timestamp is `chrono::Utc::now()`, both from crates the root manifest pins and the stem's manifest names with `.workspace = true`. A key in the state store joins the stem and the id, `format!("orders/{id}")`, through one `pub fn key` the module's readers and writers share.
