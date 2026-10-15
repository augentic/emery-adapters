//! Route rung: the production HTTP routing table driven natively.
//!
//! `omnia_test::guest::Provider` implements every capability, and `oneshot`
//! exercises the same `axum::Router` the WASI export serves, so a route,
//! codec, or handler regression surfaces here without a component build.
//!
//! `dispatch` checks **wiring, not semantics**: every `(method, path)` the
//! router registers must reach its handler. The remaining tests pin the
//! codec contract that lives in `src/lib.rs` itself (the `handle_with`
//! route). Business logic is covered by each crate's own tests.

use axum::body::{Body, to_bytes};
use axum::http::header::CONTENT_TYPE;
use axum::http::{Method, Request, Response, StatusCode};
use common::routes;
use omnia_test::guest::{MapConfig, Provider};
use soap_connector::SoapRequest;
use tower::ServiceExt as _;

const RECEIVE_MESSAGE: &[u8] = include_bytes!("../crates/soap-connector/data/receive-message.xml");

/// Every `(method, path)` the router registers, in `src/lib.rs` order.
const ROUTES: &[(Method, &str)] = &[(Method::POST, routes::http::TRAIN_XML)];

/// Default doubles with the one config key every handler reads.
fn provider() -> Provider {
    Provider::default().config(MapConfig::default().with([("ENV", "dev")]))
}

/// Replace each `{param}` segment of a route pattern with a concrete id.
fn concrete(path: &str) -> String {
    path.split('/')
        .map(|segment| if segment.starts_with('{') { "1" } else { segment })
        .collect::<Vec<_>>()
        .join("/")
}

async fn send(provider: Provider, request: Request<Body>) -> (StatusCode, Vec<u8>) {
    let response = train::router(provider).oneshot(request).await.expect("response");
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.expect("body");
    (status, body.to_vec())
}

fn content_type(response: &Response<Body>) -> &str {
    response.headers().get(CONTENT_TYPE).and_then(|value| value.to_str().ok()).unwrap_or_default()
}

/// Whether the router, rather than a handler, produced this response.
///
/// axum answers an unregistered path with an empty `404` and a registered
/// path with the wrong method with `405`. A handler's own `not_found!`
/// carries its message in the body, so it counts as dispatched.
fn router_miss(status: StatusCode, body: &[u8]) -> bool {
    status == StatusCode::METHOD_NOT_ALLOWED || (status == StatusCode::NOT_FOUND && body.is_empty())
}

#[tokio::test]
async fn dispatch() {
    let provider = provider();
    let mut misses = Vec::new();
    for (method, pattern) in ROUTES {
        let path = concrete(pattern);
        let request = Request::builder()
            .method(method.clone())
            .uri(&path)
            .body(Body::empty())
            .expect("request");
        let (status, body) = send(provider.clone(), request).await;
        if router_miss(status, &body) {
            misses.push(format!("{method} {path} -> {status}"));
        }
    }
    assert!(misses.is_empty(), "routes not dispatched:\n{}", misses.join("\n"));
}

#[tokio::test]
async fn an_unknown_path_is_not_found() {
    let request = Request::get("/nothing-here").body(Body::empty()).expect("request");

    let (status, body) = send(provider(), request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body, b"");
}

/// Malformed XML on the SOAP route: the codec contract is that it is
/// answered with the vendor's fault, not the framework's JSON `invalid_request` 400.
#[tokio::test]
async fn soap_malformed_xml() {
    let request = Request::post(routes::http::TRAIN_XML)
        .header(CONTENT_TYPE, "text/xml")
        .body(Body::from(&b"<garbage/>"[..]))
        .expect("request");

    let response = train::router(provider()).oneshot(request).await.expect("response");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(content_type(&response).starts_with("text/xml"), "{:?}", content_type(&response));
    let body = to_bytes(response.into_body(), usize::MAX).await.expect("body");
    assert!(String::from_utf8_lossy(&body).contains("<Fault>"));
}

/// The captured vendor envelope: acknowledged in the vendor's XML shape,
/// its embedded train update forwarded verbatim to the adapter's topic.
#[tokio::test]
async fn soap_receive() {
    let provider = provider();
    let request = Request::post(routes::http::TRAIN_XML)
        .header(CONTENT_TYPE, "text/xml")
        .body(Body::from(RECEIVE_MESSAGE))
        .expect("request");

    let response = train::router(provider.clone()).oneshot(request).await.expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    assert!(content_type(&response).starts_with("text/xml"), "{:?}", content_type(&response));
    let body = to_bytes(response.into_body(), usize::MAX).await.expect("body");
    assert_eq!(body.as_ref(), b"<Return>OK</Return>");

    let expected = SoapRequest::from_xml(RECEIVE_MESSAGE).expect("fixture envelope");
    let published = provider.publish.sent();
    assert_eq!(published.len(), 1);
    let (topic, record) = &published[0];
    assert_eq!(topic, "dev-realtime-train.v1");
    assert_eq!(record.payload, expected.body.receive_message.axml_message.as_bytes());
}
