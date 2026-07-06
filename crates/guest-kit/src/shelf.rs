//! The MCP reference shelf every adapter guest serves over `wasi:http`.
//!
//! The serving surface is identical across adapters — `list_docs` /
//! `read_doc` tools plus `doc://` resources over an embedded prose
//! registry — so one `Shelf` implements `omnia_guest::mcp::McpServer`
//! for all of them; a shim differs only in its server name and doc table.
//! The `McpServer` implementation is `wasm32`-gated with the rest of the
//! guest-only plumbing (the shelf is exercised end to end by the composed
//! runtime tests); the [`mcp_url`] env convention is wasm-free.

/// The adapter's own MCP reference-shelf URL from the environment.
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

#[cfg(target_arch = "wasm32")]
pub use wasm::Shelf;

#[cfg(target_arch = "wasm32")]
mod wasm {
    //! The guest-only `McpServer` implementation over an embedded doc
    //! table.

    use omnia_guest::mcp::{
        CallToolResult, Implementation, McpError, McpServer, Resource, ResourceContents, Tool,
    };
    use serde_json::{Value, json};

    use crate::registry::{self, Doc};

    /// An embedded prose registry served over MCP: every brief and
    /// reference document the adapter compiled in, addressable by its
    /// adapter-relative path.
    #[derive(Clone, Copy, Debug)]
    pub struct Shelf {
        /// Server identity reported in the `initialize` handshake, e.g.
        /// `specify-contracts-references`.
        pub server_name: &'static str,
        /// Server version reported alongside the name — the shim's own
        /// `CARGO_PKG_VERSION`.
        pub version: &'static str,
        /// The sorted embedded doc table the shelf serves.
        pub docs: &'static [Doc],
    }

    impl Shelf {
        /// Serve one `wasi:http/incoming-handler` request over this shelf's
        /// MCP router — the shared leg every adapter shim wires through
        /// its `HttpGuest`.
        pub async fn serve(
            self, request: wasip3::http::types::Request,
        ) -> Result<wasip3::http::types::Response, wasip3::http::types::ErrorCode> {
            omnia_wasi_http::serve(omnia_guest::mcp::router(self), request).await
        }
    }

    impl McpServer for Shelf {
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
                                "description": "Adapter-relative document path, e.g. `briefs/build.md`."
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
                    registry::find(self.docs, path).map_or_else(
                        || Err(McpError::resource_not_found(path)),
                        |doc| Ok(CallToolResult::text(doc.body)),
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
            registry::find(self.docs, path).map_or_else(
                || Err(McpError::resource_not_found(uri)),
                |doc| Ok(ResourceContents::text(uri, "text/markdown", doc.body)),
            )
        }
    }
}
