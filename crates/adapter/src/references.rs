//! The MCP references server every adapter guest serves over `wasi:http`.
//!
//! The surface is identical across adapters — `list_docs` / `read_doc`
//! tools plus `doc://` resources over an embedded prose registry — so one
//! [`References`] implements `omnia_guest::mcp::McpServer` for all of
//! them; a shim differs only in its server name and doc table. The
//! implementation is target-neutral: the wasm shims bridge it through
//! `References::serve` (wasm-only), and the native harness mounts the
//! same value via `omnia_guest::mcp::router` on its own listener.

use omnia_guest::mcp::{
    CallToolResult, Implementation, McpError, McpServer, Resource, ResourceContents, Tool,
};
use serde_json::{Value, json};

use crate::registry::{self, Doc};

/// The adapter's own MCP references URL from the environment.
///
/// The deployment convention is `SPECIFY_<ADAPTER>_MCP_URL` (the adapter
/// name uppercased with `-` mapped to `_`; `contracts` reads
/// `SPECIFY_CONTRACTS_MCP_URL`). Accepts either the bare adapter name or
/// the axis-qualified adapter id (`target:contracts`) — the axis prefix
/// is stripped, matching the grant-name convention on
/// [`crate::seam::Context::grants`]. Absent means judgment legs run
/// without a reference grant.
#[must_use]
pub fn mcp_url(adapter: &str) -> Option<String> {
    let name = adapter.rsplit(':').next().unwrap_or(adapter);
    let key = format!("SPECIFY_{}_MCP_URL", name.to_uppercase().replace('-', "_"));
    std::env::var(key).ok()
}

/// The references-server identity for one adapter: `<name>-references`,
/// projected from the adapter's declared name.
///
/// Interned once per name (the projection leaks one string per adapter
/// per process), so the identity stays `&'static str` for
/// [`References::server_name`] without any shim restating its own name.
///
/// # Panics
///
/// If the intern table's lock is poisoned, which no caller can trigger:
/// the insertion closure does not panic.
#[must_use]
pub fn server_name(name: &'static str) -> &'static str {
    static NAMES: std::sync::Mutex<std::collections::BTreeMap<&'static str, &'static str>> =
        std::sync::Mutex::new(std::collections::BTreeMap::new());
    NAMES
        .lock()
        .expect("server-name intern table is never poisoned")
        .entry(name)
        .or_insert_with(|| Box::leak(format!("{name}-references").into_boxed_str()))
}

/// An embedded prose registry served over MCP, addressable by
/// adapter-relative path.
#[derive(Clone, Copy, Debug)]
pub struct References {
    /// Server identity reported in the `initialize` handshake, e.g.
    /// `contracts-references`.
    pub server_name: &'static str,
    /// Server version — the shim's own `CARGO_PKG_VERSION`.
    pub version: &'static str,
    /// The sorted embedded doc table to serve.
    pub docs: &'static [Doc],
}

#[cfg(target_arch = "wasm32")]
impl References {
    /// Serve one `wasi:http/incoming-handler` request over this
    /// server's MCP router.
    ///
    /// # Errors
    ///
    /// As `omnia_wasi_http::serve` over the router.
    pub async fn serve(
        self, request: wasip3::http::types::Request,
    ) -> Result<wasip3::http::types::Response, wasip3::http::types::ErrorCode> {
        omnia_wasi_http::serve(omnia_guest::mcp::router(self), request).await
    }
}

/// Serve one `wasi:http` request over an adapter's references identity:
/// the server name projected from `name` via [`server_name`], the
/// declaring crate's `version`, and its embedded doc table.
///
/// The `source!` / `target!` macro expansions route every reference
/// request here; only the `CARGO_PKG_VERSION` stamp expands at the leaf.
///
/// # Errors
///
/// As [`References::serve`].
#[cfg(target_arch = "wasm32")]
pub async fn serve(
    name: &'static str, version: &'static str, docs: &'static [Doc],
    request: wasip3::http::types::Request,
) -> Result<wasip3::http::types::Response, wasip3::http::types::ErrorCode> {
    References {
        server_name: server_name(name),
        version,
        docs,
    }
    .serve(request)
    .await
}

impl McpServer for References {
    fn info(&self) -> Implementation {
        Implementation::new(self.server_name, self.version)
    }

    fn tools(&self) -> Vec<Tool> {
        vec![
            Tool::new(
                "list_docs",
                "List every reference document path this adapter embeds.",
                json!({ "type": "object", "properties": {} }),
            ),
            Tool::new(
                "read_doc",
                "Read one embedded reference document in full by its path.",
                json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Adapter-relative document path, e.g. `prompts/build.md`."
                        }
                    },
                    "required": ["path"]
                }),
            ),
        ]
    }

    fn call_tool(&self, name: &str, arguments: &Value) -> Result<CallToolResult, McpError> {
        match name {
            "list_docs" => {
                let paths: Vec<&str> = self.docs.iter().map(|doc| doc.path).collect();
                Ok(CallToolResult::text(json!(paths).to_string()))
            }
            "read_doc" => {
                let path = arguments.get("path").and_then(Value::as_str).unwrap_or_default();
                registry::resolve(self.docs, path).map_or_else(
                    || Err(McpError::resource_not_found(path)),
                    |body| Ok(CallToolResult::text(body)),
                )
            }
            other => Err(McpError::unknown_tool(other)),
        }
    }

    fn resources(&self) -> Vec<Resource> {
        self.docs
            .iter()
            .map(|doc| {
                Resource::new(
                    format!("doc://{}", doc.path),
                    doc.path,
                    "Embedded adapter reference document.",
                    "text/markdown",
                )
            })
            .collect()
    }

    fn read_resource(&self, uri: &str) -> Result<ResourceContents, McpError> {
        let path = uri.strip_prefix("doc://").unwrap_or(uri);
        registry::resolve(self.docs, path).map_or_else(
            || Err(McpError::resource_not_found(uri)),
            |body| Ok(ResourceContents::text(uri, "text/markdown", body)),
        )
    }
}
