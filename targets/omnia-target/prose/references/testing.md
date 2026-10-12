# Testing

One test per acceptance scenario, in the stem's `tests/` as the [layout](layout.md) places it, over the doubles of `omnia_test::guest`, so a slice's tests run in process with nothing of the host and nothing live.

## The file

`crates/<stem>/tests/<part>.rs` mounts the module alone on a fresh router over a `Client` and drives it with tower's `oneshot`:

```rust
use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::header::CONTENT_TYPE;
use axum::http::{Request, StatusCode};
use omnia_sdk::api::Client;
use omnia_test::guest::Provider;
use tower::ServiceExt as _;

fn app(provider: Provider) -> Router {
    app_orders::placing::mount(Router::new()).with_state(Client::new("test", provider))
}

fn post(body: &'static str) -> Request<Body> {
    Request::post("/orders")
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .expect("request")
}

#[tokio::test]
async fn an_order_with_items_is_placed() {
    let provider = Provider::default();

    let response = app(provider.clone()).oneshot(post(r#"{"items":["widget"]}"#)).await.expect("response");

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = to_bytes(response.into_body(), usize::MAX).await.expect("body");
    let order: app_orders::reading::Order = serde_json::from_slice(&body).expect("json");
    assert_eq!(order.items, ["widget"]);
    assert!(provider.storage.state(&app_orders::reading::key(&order.id)).is_some());
}

#[tokio::test]
async fn an_order_with_no_items_is_refused() {
    let response = app(Provider::default()).oneshot(post(r#"{"items":[]}"#)).await.expect("response");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
```

- One `#[tokio::test]` per scenario, named by the scenario in snake_case: `an_order_with_no_items_is_refused`. A scenario the specification gives you is a test; invent none.
- The `WHEN` is the request and the state the doubles start in; the `THEN` is the status, the body read through `to_bytes(response.into_body(), usize::MAX)` and deserialised into the module's own type, and what the doubles hold after.
- The tests reach the module as another crate would, through `app_<stem>::<part>::..`; a `pub fn key` the module shares with its readers is how a test names what the handler stored.
- The four dev-dependencies the stem's manifest carries are everything a test uses: `omnia-test`, `serde_json`, `tokio`, `tower`. No mocking crate, no test framework, no fixture file.

## The doubles

`Provider::default()` answers every capability; each is set with a builder method, and read back after the call through the field of the same name. Clone the provider before handing it to `app`, so the test still holds it:

| Capability | Set with | Read back |
| --- | --- | --- |
| `Config` | `.config(MapConfig::default().with([("RATES_URL", "https://rates.example/v1")]))` | — |
| `StateStore`, `BlobStore` | `provider.storage.insert_state(key, bytes)`, `provider.storage.insert_object(container, name, bytes)` | `provider.storage.state(key)`, `provider.storage.object(container, name)` |
| `HttpRequest` | `.http(MatchedHttp::default().on(Method::GET, "https://rates.example/v1/NZD", response))`, `response` an `http::Response<Bytes>` | `provider.http.requests()` |
| `Publish` | `.publish(Sink::default())` | `provider.publish.sent()`, a `(topic, Message)` per call |
| `Broadcast` | `.broadcast(Sink::default())` | `provider.broadcast.broadcasts()` |
| `Identity` | `.identity(FixedIdentity::new("token"))` | `provider.identity.asked()` |
| `DocumentStore` | `.docs(MemoryDocs::default())` | `provider.docs.documents(store).await` |
| `TableStore` | `.tables(ScriptedTables::default().on_query(matches, rows).on_exec(matches, affected))` | `provider.tables.statements()` |

An arrangement over an upstream and a topic:

```rust
let upstream = Response::builder()
    .status(200)
    .body(Bytes::from_static(br#"{"currency":"NZD","rate":1.62}"#))
    .expect("response");
let provider = Provider::default()
    .config(MapConfig::default().with([("RATES_URL", "https://rates.example/v1")]))
    .http(MatchedHttp::default().on(Method::GET, "https://rates.example/v1/NZD", upstream))
    .publish(Sink::default());

let response = app(provider.clone()).oneshot(request).await.expect("response");

assert_eq!(provider.http.requests().len(), 1);
assert_eq!(provider.publish.sent().len(), 1);
```

A request the `MatchedHttp` holds no answer for, or a statement the `ScriptedTables` holds no rule for, panics naming it, so the arrangement scripts every call the handler makes; a scenario of an upstream that fails is a scripted `503` response, never a missing match.

## What a test never does

- Listen on a port, open a socket, or connect to anything: a route is driven in process through `oneshot`, and a service the tree does not run is the double above, never a live connection.
- Read a file, an environment variable, or the clock for its arrangement: the test's data is in the test.
- Call the production provider, or anything under `cfg(target_arch = "wasm32")`: the tests compile natively, over `omnia_test::guest::Provider`.
- Sleep, retry, or wait on a timer.
- Carry `#[ignore]`, or pass by asserting nothing: a scenario the module does not yet satisfy is a test that fails, and a requirement the report leaves uncovered.
