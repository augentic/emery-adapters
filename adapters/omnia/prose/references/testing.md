# Testing

One test per acceptance scenario, in the stem's `tests/` as the [layout](layout.md) places it, over the doubles of `omnia_test::guest`, so a slice's tests run in process with nothing of the host and nothing live. The two root rungs, `tests/routes.rs` and `tests/messaging.rs`, check the wiring over their tables; a stem's tests check the behaviour by calling the handler.

## The file

`crates/<stem>/tests/<part>.rs` calls the handler through a `Client` over the doubles, as the two routers do, with no router and no request:

```rust
//! The `placing` part of `orders`: one test per acceptance scenario.

use omnia_sdk::api::{Client, Metadata};
use omnia_test::guest::{MapConfig, Provider, Sink};
use orders::placing::{PlaceOrder, key, place_order};

fn provider() -> Provider {
    Provider::default()
        .config(MapConfig::default().with([(common::config::ENV, "test")]))
        .publish(Sink::default())
}

#[tokio::test]
async fn an_order_with_items_is_placed() {
    let provider = provider();
    let input = PlaceOrder {
        items: vec!["widget".to_owned()],
    };

    let order = Client::new("test", provider.clone())
        .call(place_order, input, &Metadata::default())
        .await
        .expect("placed");

    assert_eq!(order.items, ["widget"]);
    assert!(provider.storage.state(&key(&order.id)).is_some());
    let sent = provider.publish.sent();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].0, "test-orders-placed.v1");
}

#[tokio::test]
async fn an_order_with_no_items_is_refused() {
    let input = PlaceOrder { items: vec![] };

    let error = Client::new("test", provider())
        .call(place_order, input, &Metadata::default())
        .await
        .expect_err("refused");

    assert_eq!(error.code(), "bad_request");
}
```

- One `#[tokio::test]` per scenario, named by the scenario in snake_case: `an_order_with_no_items_is_refused`. A scenario the specification gives you is a test; invent none.
- The `WHEN` is the input and the state the doubles start in; the `THEN` is the handler's `Ok` or its `Err` — `error.code()` the class, `error.description()` the message — and what the doubles hold after. A route's status is the error's class, so a test asserts the code and never a status.
- The file opens with a `//!` and declares nothing `pub`: the workspace lints want a doc on every public item and on every crate, a test file included, and a helper is a private `fn`.
- The tests reach the module as another crate would, through `<stem>::<part>::..`; a `pub fn key` the module shares with its readers is how a test names what the handler stored.
- A topic's handler is called the same way, its input the payload's type; a scenario of a delivery that arrives malformed is the root messaging rung's, not the stem's.
- A module with no route or topic is tested through the `pub fn`s it exports, each scenario a call and an assertion over its result.
- A rule another stem exports and your handler calls at its top — `start::authentication::authenticate` — is asserted where the call stands: `a_request_without_a_token_is_refused` is a test of the stem whose handler it is, driven through that handler over the double the rule reads, `FixedIdentity` for a token; the rule's own stem tests the function directly.
- The three dev-dependencies the stem's manifest carries are the doubles and the runtime every test uses: `omnia-test`, `serde_json`, `tokio`. A crate an arrangement needs beyond them, `bytes` or `http`, is one line in the stem's `[dependencies]`, never a fourth dev-dependency. No mocking crate, no test framework, no fixture file.

## The doubles

`Provider::default()` answers every capability; each is set with a builder method, and read back after the call through the field of the same name. Clone the provider into the `Client`, so the test still holds it. A handler that qualifies a topic reads `ENV`, so the arrangement sets it, `(common::config::ENV, "test")`, and the topic a test expects is `test-<suffix>`. The builders and readers of each double, with their full signatures, are in [the omnia API](omnia-api.md); the ones a scenario reaches for:

| Capability | Set with | Read back |
| --- | --- | --- |
| `Config` | `.config(MapConfig::default().with([(common::config::ENV, "test"), (common::config::RATES_URL, "https://rates.example/v1")]))` | — |
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
    .config(MapConfig::default().with([
        (common::config::ENV, "test"),
        (common::config::RATES_URL, "https://rates.example/v1"),
    ]))
    .http(MatchedHttp::default().on(Method::GET, "https://rates.example/v1/NZD", upstream))
    .publish(Sink::default());

let rate = Client::new("test", provider.clone())
    .call(rate, RateQuery { currency: "NZD".to_owned() }, &Metadata::default())
    .await
    .expect("rated");

assert_eq!(provider.http.requests().len(), 1);
assert_eq!(provider.publish.sent()[0].0, "test-rates-seen.v1");
```

A request the `MatchedHttp` holds no answer for, or a statement the `ScriptedTables` holds no rule for, panics naming it, so the arrangement scripts every call the handler makes; a scenario of an upstream that fails is a scripted `503` response, never a missing match.

## The root rungs

Your lines in `tests/routes.rs` and `tests/messaging.rs` are the two root rungs' tables, one entry per route and per topic you registered, and nothing more: `dispatch` sends every table entry an empty request and fails on the ones the router answers itself, `routed` delivers every entry an empty payload under the test environment and fails on the ones the router reports unhandled. They prove a route and a topic reach a handler; the handler's answer to an empty request is its own, and a handler that panics on one fails the rung, so validate the input before reaching the host. Neither rung is edited beyond its table, and neither stands in for a scenario of the stem's.

## What a test never does

- Listen on a port, open a socket, or connect to anything: a handler is called in process, and a service the tree does not run is the double above, never a live connection.
- Read a file, an environment variable, or the clock for its arrangement: the test's data is in the test.
- Call the production provider, or anything under `cfg(target_arch = "wasm32")`: the tests compile natively, over `omnia_test::guest::Provider`.
- Sleep, retry, or wait on a timer.
- Carry `#[ignore]`, or pass by asserting nothing: a scenario the module does not yet satisfy is a test that fails, and a requirement the report leaves uncovered.
