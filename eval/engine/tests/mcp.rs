//! Native MCP reference shelves — one per linked adapter.

use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode, header};
use harness::catalog::Catalog;
use harness::mcp;
use omnia_testkit::model::Scripted;
use serde_json::{Value, json};
use engine::catalog;
use tower::ServiceExt as _;

fn linked() -> Catalog<Scripted> {
    catalog::catalog()
}

async fn post(path: &str, message: &Value) -> (StatusCode, Value) {
    let request = Request::builder()
        .method(Method::POST)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(message.to_string()))
        .expect("build request");
    let response =
        mcp::router(&linked()).oneshot(request).await.expect("router serves the request");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.expect("collect body");
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

fn call(name: &str, arguments: &Value) -> Value {
    json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": { "name": name, "arguments": arguments }
    })
}

#[test]
fn shelves_match_linked_adapters() {
    let linked = linked();
    let names: Vec<&str> = mcp::shelves(&linked).iter().map(|shelf| shelf.name).collect();
    let catalog_names: Vec<&str> =
        linked.entries().iter().map(harness::catalog::Entry::name).collect();
    assert_eq!(names, catalog_names, "MCP shelves must derive from the native catalog");
    assert_eq!(
        catalog_names,
        [
            "captures",
            "contracts",
            "documentation",
            "intent",
            "omnia",
            "screenshots",
            "typescript",
            "vectis"
        ]
    );
}

#[tokio::test]
async fn read_doc_embedded_prose() {
    let (status, reply) =
        post("/mcp/omnia", &call("read_doc", &json!({ "path": "prompts/guidance.md" }))).await;
    assert_eq!(status, StatusCode::OK);
    let text = reply["result"]["content"][0]["text"].as_str().expect("text content");
    assert!(text.starts_with("# Omnia target — guidance prompt"), "{text:.80}");
}

#[tokio::test]
async fn list_docs_shelf_registry() {
    let (status, reply) = post("/mcp/intent", &call("list_docs", &json!({}))).await;
    assert_eq!(status, StatusCode::OK);
    let text = reply["result"]["content"][0]["text"].as_str().expect("text content");
    let paths: Vec<String> = serde_json::from_str(text).expect("path list JSON");
    assert!(paths.iter().any(|path| path == "prompts/survey.md"), "{paths:?}");
}

#[tokio::test]
async fn unmounted_shelf_not_found() {
    let (status, _) = post("/mcp/unknown", &call("list_docs", &json!({}))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
