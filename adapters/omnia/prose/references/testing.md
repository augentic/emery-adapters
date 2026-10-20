# Testing

One test per acceptance scenario, in the stem's `tests/` as the [layout](layout.md) places it, over the doubles of `omnia_test::guest`, so a slice's tests run in process with nothing of the host and nothing live. A stem's tests drive the stem's `router()` with a request and its `topics(..)` with a delivery, so a route or a topic is asserted as the wire reaches it, and the handler, being `pub(crate)`, is reached no other way; the two root rungs check that the composed routers build and refuse an unknown path or topic, and nothing of a slice's goes in them.

## The file

`crates/<stem>/tests/<part>.rs` builds the stem's router over a `Client` of the doubles, as the root does, and sends it one request per scenario. The adapter scaffolds the file before the turn: the prelude, then one ignored test per scenario over the act its requirement carries. Filled, the file reads:

```rust
//! The acceptance scenarios of the `placing` part of the `orders` stem, over the guest doubles.

use axum::body::{Body, to_bytes};
use axum::http::header::CONTENT_TYPE;
use axum::http::{Request, StatusCode};
use common::config;
use omnia_sdk::api::Client;
use omnia_test::guest::{MapConfig, Provider};
use orders::placing::Placed;
use tower::ServiceExt as _;

// The doubles every scenario starts from: set a setting or a response on them in the arrangement.
fn provider() -> Provider {
    Provider::default().config(MapConfig::default().with([(config::ENV, "test")]))
}

// One request through the stem's router, answered as its status and body.
async fn send(provider: Provider, request: Request<Body>) -> (StatusCode, Vec<u8>) {
    let router = orders::router().with_state(Client::new("test", provider));
    let response = router.oneshot(request).await.expect("response");
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.expect("body");
    (status, body.to_vec())
}

// REQ-030 orders.place: An order within the limit is placed
//   Given a limit of two lines
//   When an order of two lines is placed
//   Then it is answered and published once
#[tokio::test]
async fn an_order_within_the_limit_is_placed() {
    let provider = provider()
        .config(MapConfig::default().with([(config::ENV, "test"), ("ORDERS_LIMIT", "2")]));
    let request = Request::post("/orders")
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"lines": ["tea", "cake"]}"#))
        .expect("request");
    let (status, body) = send(provider.clone(), request).await;
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    let placed: Placed = serde_json::from_slice(&body).expect("a placement");
    assert_eq!(placed.lines, 2);
    let sent = provider.publish.sent();
    assert_eq!(sent.len(), 1, "the placement is published once");
    assert_eq!(sent[0].0, "test-order-placed");
}

// REQ-030 orders.place: An order past the limit is refused
//   Given a limit of two lines
//   When an order of three lines is placed
//   Then it is refused and nothing is published
#[tokio::test]
async fn an_order_past_the_limit_is_refused() {
    let provider = provider()
        .config(MapConfig::default().with([(config::ENV, "test"), ("ORDERS_LIMIT", "2")]));
    let request = Request::post("/orders")
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"lines": ["tea", "cake", "jam"]}"#))
        .expect("request");
    let (status, _body) = send(provider.clone(), request).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(provider.publish.sent().is_empty(), "nothing is published");
}
```

- One `#[tokio::test]` per scenario, named by the scenario in snake_case: `an_order_with_no_items_is_refused`. The adapter scaffolds each before the turn, under `#[ignore = "<id> <scenario>: unfilled"]`, with the scenario's `GIVEN`, `WHEN`, and `THEN` in a comment above it and its act laid from the requirement's route or message — the request built and sent through `send`, a path parameter bound above it by a `let <param> = "<param>";` line, a body `"{}"`; a delivery through `deliver` with the payload `b"{}"`; a module with no route or topic over `provider()` alone — and a `todo!("<id> <scenario>: arrange, act, assert")` under it. Fill the test as one edit over the block the brief quotes: the `#[ignore = ..]` line gone, the parameter, body, or payload the scenario states in place of the placeholders, the `let _ = ..;` line that held the act's results replaced by the assertions. A scenario the specification gives you is a test; invent none, and delete none.
- The `WHEN` is the request — its method, path, headers, and body — and the state the doubles start in; the `THEN` is the response's status and body, and what the doubles hold after. A handler's `Ok` is `200` with the output as JSON; its `Err` is the class's status, `400` for `bad_request`, `404` for `not_found`, `500` for `server_error`, `502` for `bad_gateway`, with the body `{"error": "<code>", "message": ".."}`, so a test asserts the status and, where the scenario states the reason, the body's `message`. When a scenario's `THEN` is `[unknown]`, derive the assertion from the outcome stated in the requirement's statement itself (for example, `StatusCode::BAD_REQUEST` when the requirement states an input is rejected or skipped on validation failure, or asserting the returned value or published messages), and include the requirement in `covered`.
- The prelude is the adapter's, rendered with the functions the part's tests use and nothing more: `provider()`, the doubles every scenario starts from, with `ENV` set; `send(provider, request)`, one request through the stem's `router()` answered as `(StatusCode, Vec<u8>)`; `deliver(provider, topic, payload)`, one delivery through the stem's `topics(..)` under the test environment, the topic qualified for you. Each takes the provider by value, so a test that reads the doubles after the act hands it `provider.clone()`. A setting the part reads is set in the arrangement by building the config whole, `provider().config(MapConfig::default().with([(config::ENV, "test"), ("ORDERS_LIMIT", "2")]))`, since `.config(..)` sets the double rather than adding to it and `ENV` goes with it; a response an upstream answers is `.http(..)` on the same provider. A `GET` is `Request::get("/orders/42").body(Body::empty())`, a query `Request::get("/orders?status=open")`, as the laid act spells it.
- The file declares nothing `pub`: the workspace lints want a doc on every public item, a test file included, and a helper is a private `fn`.
- The tests reach the stem as another crate would, through `<stem>::router()`, `<stem>::topics(..)`, and `<stem>::<part>::<Type>` for the `pub` types a response or a payload decodes into; what the handler stored is read back from the doubles under the key the module spells, `provider.storage.state("orders/42")`, or through a `pub fn key` the module shares with its readers.
- A topic's handler is driven through the stem's `topics(..)` over a `messaging::Router` of the doubles, which the prelude's `deliver` builds: the delivery's `topic` the constant qualified under the test's environment and its `payload` the wire's bytes:

```rust
use common::topic;
use omnia_sdk::StateStore;

// REQ-032 orders.count: Placements are counted
//   Given two placements
//   When each is delivered
//   Then the count is their lines
#[tokio::test]
async fn placements_are_counted() {
    let provider = provider();
    for lines in [1, 2] {
        let payload = serde_json::to_vec(&Placed { lines }).expect("a placement");
        deliver(provider.clone(), topic::ORDER_PLACED, &payload)
            .await
            .expect("a placement is counted");
    }
    let total = StateStore::increment(&provider, "orders/lines-placed", 0).await.expect("the count");
    assert_eq!(total, 3, "the placements are counted");
}
```

  A delivery the handler rejects is `Err(DeliveryError::Rejected(..))`, `expect_err` and `matches!` over it; a delivery that arrives malformed fails in the decoder the same way.

- Where a handler or a helper deserializes an XML payload or an upstream JSON response, the test feeds the exact root wire format — `<Envelope><ShipmentNotice><primaryRef>5226</primaryRef>...` for XML, a top-level JSON array `[{"facility_code":"WH-101",...}]` or a root object `{"all":[{"carrierId":"..."}]}` without an outer `{"data":...}` wrapper for `MatchedHttp` — and asserts that the parsed fields (`primary_ref`, `secondary_ref`, `issue_date`, `legs`) and the published message's nested wire fields (`eventType`, `receivedAt`, `messageData.timestamp`, `remoteData.externalId`, `locationData.latitude`) match.
- A module with no route or topic is tested through the `pub fn`s it exports, each scenario a call and an assertion over its result.
- A rule another stem exports and your handler calls at its top — `auth::authentication::authenticate` — is asserted where the call stands: `a_request_without_a_token_is_refused` is a test of the stem whose route it is, driven through that stem's router over the double the rule reads, `FixedIdentity` for a token; the rule's own stem tests the function directly.
- The three dev-dependencies the stem's manifest carries are the doubles, the runtime, and the service adapter every test uses: `omnia-test`, `tokio`, `tower`, the last for `oneshot`. A crate an arrangement needs beyond them, `bytes` or `http`, is one line in the stem's `[dependencies]`, never a fourth dev-dependency; `axum`, `serde_json`, and `common` are there already, since the module uses them. No mocking crate, no test framework, no fixture file.

## The doubles

`Provider::default()` answers every capability; each is set with a builder method, and read back after the call through the field of the same name. Hand `send` and `deliver` a clone, so the test still holds it. The root resolves `ENV` once and hands it to every stem's `topics(..)`, so a test hands `"test"` itself; a handler that publishes qualifies its topic through `config::topic`, which reads `ENV`, so the arrangement sets it, `(config::ENV, "test")`, and the topic a test expects is `test-<suffix>`. A setting the part reads is set under its name, `("ORDERS_LIMIT", "2")`, since the constant is the part's own. The builders and readers of each double, with their full signatures, are in [the omnia API](omnia-api.md); the ones a scenario reaches for:

| Capability | Set with | Read back |
| --- | --- | --- |
| `Config` | `.config(MapConfig::default().with([(config::ENV, "test"), ("RATES_URL", "https://rates.example/v1")]))` | — |
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
let provider = provider()
    .config(MapConfig::default().with([
        (config::ENV, "test"),
        ("RATES_URL", "https://rates.example/v1"),
    ]))
    .http(MatchedHttp::default().on(Method::GET, "https://rates.example/v1/NZD", upstream));

let request = Request::get("/rates/NZD").body(Body::empty()).expect("request");
let (status, body) = send(provider.clone(), request).await;

assert_eq!(status, StatusCode::OK);
let seen: Rate = serde_json::from_slice(&body).expect("a rate");
assert_eq!(seen.currency, "NZD");
assert_eq!(provider.http.requests().len(), 1);
assert_eq!(provider.publish.sent()[0].0, "test-rates-seen.v1");
```

A request the `MatchedHttp` holds no answer for, or a statement the `ScriptedTables` holds no rule for, panics naming it, so the arrangement scripts every call the handler makes; a scenario of an upstream that fails is a scripted `503` response, never a missing match.

## The root rungs

`tests/routes.rs` builds the merged router over a seeded provider and sends it a path no stem serves; `tests/messaging.rs` builds the registered router and delivers a topic no stem consumes. Both are static: they prove the composition builds, which is where two stems serving one path under one method, or consuming one topic, panic with the path or the topic named, and that the routers refuse what no stem claims. Neither takes a line of a slice's, and neither stands in for a scenario of the stem's.

## What a test never does

- Listen on a port, open a socket, or connect to anything: the router is driven in process with `oneshot`, and a service the tree does not run is the double above, never a live connection.
- Read a file, an environment variable, or the clock for its arrangement: the test's data is in the test.
- Call the production provider, or anything under `cfg(target_arch = "wasm32")`: the tests compile natively, over `omnia_test::guest::Provider`.
- Sleep, retry, or wait on a timer.
- Keep its `#[ignore = ..]` line once filled, or pass by asserting nothing: a scenario the module does not yet satisfy keeps the ignored test the adapter scaffolded, as scaffolded, and its requirement stays out of `covered`; a filled test runs and asserts what the scenario states.
