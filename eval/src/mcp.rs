//! Per-adapter MCP reference shelves mounted at `/mcp/<name>`.

use adapter::references::References;
use omnia_guest::axum::Router;

use crate::catalog;

/// One mounted reference shelf.
#[derive(Clone, Copy, Debug)]
pub struct Shelf {
    /// Axis-stripped adapter name.
    pub name: &'static str,
    /// The adapter's references server over its embedded registry.
    pub references: References,
}

/// Every linked adapter's reference shelf.
#[must_use]
pub fn shelves() -> Vec<Shelf> {
    catalog::entries()
        .iter()
        .map(|entry| Shelf {
            name: entry.name(),
            references: References {
                server_name: entry.server_name(),
                version: env!("CARGO_PKG_VERSION"),
                docs: entry.docs(),
            },
        })
        .collect()
}

/// Shelf router nested at `/mcp/<name>`, ready to merge onto the verb router.
pub fn router() -> Router {
    shelves().into_iter().fold(Router::new(), |router, shelf| {
        router.nest(&format!("/mcp/{}", shelf.name), omnia_guest::mcp::router(shelf.references))
    })
}

/// Serve the shelves on an ephemeral background listener.
///
/// Returns the base URL for `Provider::mcp_base`, or `None` when no port can be bound.
pub async fn ephemeral_base() -> Option<String> {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.ok()?;
    let base = format!("http://127.0.0.1:{}", listener.local_addr().ok()?.port());
    tokio::spawn(async move {
        drop(axum::serve(listener, router()).await);
    });
    Some(base)
}
