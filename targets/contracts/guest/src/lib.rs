//! The contracts adapter guest: `wasm32` shim over `specify-contracts-core`.
//!
//! Exports the `augentic:specify` `target-adapter` world — `guidance` /
//! `build` / `merge` route into the wasm-free core's operation template with
//! the WASI-backed `Model` provider — plus `wasi:http/incoming-handler`,
//! serving the core's embedded prose registry as an MCP reference shelf
//! (`list_docs` / `read_doc` tools, `doc://` resources).
//!
//! The seam operations are `async func`s, so the exports async-lift and the
//! judgment legs await the async `omnia:model/completion.create` import
//! directly — a sync-lifted export may never block, so there is no sync
//! bridge to hide behind. The MCP grant URL is read from the environment
//! (`SPECIFY_CONTRACTS_MCP_URL`), never hardcoded — absent, judgment legs
//! run without a reference grant.
#![cfg(target_arch = "wasm32")]

mod bindings {
    //! `wit_bindgen::generate!` output for the `target-adapter` world. The
    //! `export!` shim is invoked here too: lint levels resolve at the macro
    //! invocation's syntactic context, so the generated `unsafe(export_name)`
    //! plumbing must expand inside this allow scope.
    #![allow(
        missing_docs,
        unsafe_code,
        clippy::pedantic,
        clippy::nursery,
        reason = "wit-bindgen generated bindings are not hand-maintained; the generated code cannot carry this workspace's lint posture"
    )]

    use super::ContractsAdapter;

    wit_bindgen::generate!({
        world: "target-adapter",
        path: "../../../wit",
        // The seam operations are `async func`s (judgment legs await the
        // async `omnia:model` import mid-call), so the exports async-lift.
        async: true,
    });

    export!(ContractsAdapter);
}

use std::path::Path;

use bindings::exports::augentic::specify::target::{
    AdapterId, BuildOutput, Changeset, Error, Finding, Guest, Input, Platform, Report, Severity,
    Status, UiSurface, WorkingTree,
};
use omnia_guest::mcp::{
    self, CallToolResult, Implementation, McpError, McpServer, Resource, ResourceContents, Tool,
};
use serde_json::{Value, json};
use specify_contracts_core::{operations, registry, report};
use specify_guest_kit::Model;
use wasip3::http::types as http;

/// Environment key carrying this adapter's own MCP reference-shelf URL,
/// granted to the spawned agent on every judgment leg.
const MCP_URL_ENV: &str = "SPECIFY_CONTRACTS_MCP_URL";

/// The WASI-backed judgment provider: the [`Model`] trait's `wasm32`
/// default method body delegates to the `omnia-wasi-model` bindings.
struct WasiModel;

impl Model for WasiModel {}

struct ContractsAdapter;

impl Guest for ContractsAdapter {
    async fn guidance(_id: AdapterId) -> Result<String, Error> {
        Ok(operations::guidance().to_string())
    }

    async fn build(
        id: AdapterId, slice: String, inputs: Vec<Input>, tree: WorkingTree,
    ) -> Result<Report, Error> {
        let inputs: Vec<operations::Input> = inputs.into_iter().map(input_into_core).collect();
        let tree = tree_into_core(tree);
        let mcp_url = std::env::var(MCP_URL_ENV).ok();
        let ctx = context(&id, mcp_url.as_deref());
        operations::build(&WasiModel, &ctx, &slice, &inputs, &tree)
            .await
            .map(report_into_wit)
            .map_err(error_into_wit)
    }

    async fn merge(
        id: AdapterId, slice: String, delta: Changeset, tree: WorkingTree,
    ) -> Result<Report, Error> {
        let delta = operations::Changeset {
            base: delta.base,
            edits: delta
                .edits
                .into_iter()
                .map(|edit| operations::Edit {
                    path: edit.path,
                    content: edit.content,
                })
                .collect(),
        };
        let tree = tree_into_core(tree);
        let mcp_url = std::env::var(MCP_URL_ENV).ok();
        let ctx = context(&id, mcp_url.as_deref());
        operations::merge(&WasiModel, &ctx, &slice, &delta, &tree)
            .await
            .map(report_into_wit)
            .map_err(error_into_wit)
    }
}

/// The call-scoped context: every guest in the deployment shares the same
/// `[[mount]]` preopens, so the operation root is the guest's own `"."`.
fn context<'a>(adapter_id: &'a str, mcp_url: Option<&'a str>) -> operations::Context<'a> {
    operations::Context {
        adapter_id,
        project_root: Path::new("."),
        mcp_url,
    }
}

fn input_into_core(input: Input) -> operations::Input {
    match input {
        Input::Proposal(body) => operations::Input::Proposal(body),
        Input::Design(body) => operations::Input::Design(body),
        Input::Tasks(body) => operations::Input::Tasks(body),
        Input::Spec(body) => operations::Input::Spec(body),
        Input::Other(body) => operations::Input::Other(body),
    }
}

fn tree_into_core(tree: WorkingTree) -> operations::WorkingTree {
    operations::WorkingTree {
        base: tree.base,
        subpath: tree.subpath,
    }
}

fn report_into_wit(report: report::Report) -> Report {
    Report {
        status: match report.status {
            report::Status::Success => Status::Success,
            report::Status::Failure => Status::Failure,
        },
        findings: report
            .findings
            .into_iter()
            .map(|finding| Finding {
                rule_id: finding.rule_id,
                severity: severity_into_wit(finding.severity),
                detail: finding.detail,
            })
            .collect(),
        outputs: report
            .outputs
            .into_iter()
            .map(|output| BuildOutput {
                platform: platform_into_wit(output.platform),
                path: output.path,
            })
            .collect(),
        ui_surface: report.ui_surface.map(|surface| UiSurface {
            screens: surface.screens,
        }),
    }
}

const fn severity_into_wit(severity: report::Severity) -> Severity {
    match severity {
        report::Severity::Critical => Severity::Critical,
        report::Severity::Important => Severity::Important,
        report::Severity::Suggestion => Severity::Suggestion,
        report::Severity::Optional => Severity::Optional,
    }
}

const fn platform_into_wit(platform: report::Platform) -> Platform {
    match platform {
        report::Platform::Core => Platform::Core,
        report::Platform::Ios => Platform::Ios,
        report::Platform::Android => Platform::Android,
        report::Platform::Web => Platform::Web,
        report::Platform::Desktop => Platform::Desktop,
    }
}

fn error_into_wit(error: operations::Error) -> Error {
    match error {
        operations::Error::InvalidRequest(detail) => Error::InvalidRequest(detail),
        operations::Error::Io(detail) => Error::Io(detail),
        operations::Error::Internal(detail) => Error::Internal(detail),
    }
}

struct HttpGuest;

wasip3::http::service::export!(HttpGuest);

impl wasip3::exports::http::handler::Guest for HttpGuest {
    async fn handle(request: http::Request) -> Result<http::Response, http::ErrorCode> {
        omnia_wasi_http::serve(mcp::router(References), request).await
    }
}

/// The embedded prose registry served over MCP: every brief and reference
/// document `specify-contracts-core` compiled in, addressable by its
/// adapter-relative path.
struct References;

impl McpServer for References {
    fn info(&self) -> Implementation {
        Implementation::new("specify-contracts-references", env!("CARGO_PKG_VERSION"))
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
                let paths: Vec<&str> = registry::docs().iter().map(|doc| doc.path).collect();
                Ok(CallToolResult::text(json!(paths).to_string()))
            }
            "read_doc" => {
                let path = arguments.get("path").and_then(Value::as_str).unwrap_or_default();
                registry::doc(path).map_or_else(
                    || Err(McpError::resource_not_found(path)),
                    |doc| Ok(CallToolResult::text(doc.body)),
                )
            }
            other => Err(McpError::unknown_tool(other)),
        }
    }

    fn resources(&self) -> Vec<Resource> {
        registry::docs()
            .iter()
            .map(|doc| {
                Resource::new(
                    format!("doc://{}", doc.path),
                    doc.path,
                    "Embedded contracts-adapter reference document.",
                    "text/markdown",
                )
            })
            .collect()
    }

    fn read_resource(&self, uri: &str) -> Result<ResourceContents, McpError> {
        let path = uri.strip_prefix("doc://").unwrap_or(uri);
        registry::doc(path).map_or_else(
            || Err(McpError::resource_not_found(uri)),
            |doc| Ok(ResourceContents::text(uri, "text/markdown", doc.body)),
        )
    }
}
