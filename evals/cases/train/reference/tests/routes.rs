//! Route rung: the production HTTP routing table driven natively.
//!
//! `omnia_test::guest::Provider` implements every capability, and `oneshot`
//! exercises the same `axum::Router` the WASI export serves. The train
//! position adapter registers no HTTP route — its one ingress is a topic —
//! so the rung pins that the router answers every path as unregistered.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use omnia_test::guest::{MapConfig, Provider};
use tower::ServiceExt as _;

/// Default doubles with the one config key every handler reads.
fn provider() -> Provider {
    Provider::default().config(MapConfig::default().with([("ENV", "dev")]))
}

#[tokio::test]
async fn an_unknown_path_is_not_found() {
    let request = Request::get("/nothing-here").body(Body::empty()).expect("request");

    let response = train::router(provider()).oneshot(request).await.expect("response");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = to_bytes(response.into_body(), usize::MAX).await.expect("body");
    assert!(body.is_empty());
}
