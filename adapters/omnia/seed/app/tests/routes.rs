//! The whole router over the test doubles: every slice's routes mount together.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use omnia_test::guest::Provider;
use tower::ServiceExt as _;

#[tokio::test]
async fn an_unknown_path_is_not_found() {
    let request = Request::get("/nothing-here").body(Body::empty()).expect("request");

    let response = app::router(Provider::default()).oneshot(request).await.expect("response");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
