//! The contracts adapter guest: `wasm32` shim over `specify-contracts-core`.
//!
//! Exports the `augentic:specify` `target-adapter` world — `guidance` /
//! `build` / `merge` route into the wasm-free core's operation template with
//! the WASI-backed `Model` provider — plus `wasi:http/incoming-handler`,
//! serving the core's embedded prose registry through the guest-kit
//! `Shelf` as an MCP reference shelf (`list_docs` / `read_doc` tools,
//! `doc://` resources).
//!
//! The seam operations are `async func`s, so the exports async-lift and the
//! judgment legs await the async `omnia:model/completion.create` import
//! directly — a sync-lifted export may never block, so there is no sync
//! bridge to hide behind. The MCP grant URL is read from the environment
//! (`SPECIFY_CONTRACTS_MCP_URL`, the guest-kit `mcp_url` convention), never
//! hardcoded — absent, judgment legs run without a reference grant.
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
        path: "../../wit",
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
use omnia_guest::mcp;
use specify_contracts_core::{operations, registry};
use specify_guest_kit::shelf::{Shelf, mcp_url};
use specify_guest_kit::{Model, seam};
use wasip3::http::types as http;

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
        let inputs: Vec<seam::Input> = inputs.into_iter().map(input_into_core).collect();
        let tree = tree_into_core(tree);
        let url = mcp_url("contracts");
        let ctx = context(&id, url.as_deref());
        operations::build(&WasiModel, &ctx, &slice, &inputs, &tree)
            .await
            .map(report_into_wit)
            .map_err(error_into_wit)
    }

    async fn merge(
        id: AdapterId, slice: String, delta: Changeset, tree: WorkingTree,
    ) -> Result<Report, Error> {
        let delta = seam::Changeset {
            base: delta.base,
            edits: delta
                .edits
                .into_iter()
                .map(|edit| seam::Edit {
                    path: edit.path,
                    content: edit.content,
                })
                .collect(),
        };
        let tree = tree_into_core(tree);
        let url = mcp_url("contracts");
        let ctx = context(&id, url.as_deref());
        operations::merge(&WasiModel, &ctx, &slice, &delta, &tree)
            .await
            .map(report_into_wit)
            .map_err(error_into_wit)
    }
}

/// The call-scoped context: every guest in the deployment shares the same
/// `[[mount]]` preopens, so the operation root is the guest's own `"."`.
fn context<'a>(adapter_id: &'a str, mcp_url: Option<&'a str>) -> seam::Context<'a> {
    seam::Context {
        adapter_id,
        project_root: Path::new("."),
        mcp_url,
    }
}

fn input_into_core(input: Input) -> seam::Input {
    match input {
        Input::Proposal(body) => seam::Input::Proposal(body),
        Input::Design(body) => seam::Input::Design(body),
        Input::Tasks(body) => seam::Input::Tasks(body),
        Input::Spec(body) => seam::Input::Spec(body),
        Input::Other(body) => seam::Input::Other(body),
    }
}

fn tree_into_core(tree: WorkingTree) -> seam::WorkingTree {
    seam::WorkingTree {
        base: tree.base,
        subpath: tree.subpath,
    }
}

fn report_into_wit(report: seam::Report) -> Report {
    Report {
        status: match report.status {
            seam::Status::Success => Status::Success,
            seam::Status::Failure => Status::Failure,
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

const fn severity_into_wit(severity: seam::Severity) -> Severity {
    match severity {
        seam::Severity::Critical => Severity::Critical,
        seam::Severity::Important => Severity::Important,
        seam::Severity::Suggestion => Severity::Suggestion,
        seam::Severity::Optional => Severity::Optional,
    }
}

const fn platform_into_wit(platform: seam::Platform) -> Platform {
    match platform {
        seam::Platform::Core => Platform::Core,
        seam::Platform::Ios => Platform::Ios,
        seam::Platform::Android => Platform::Android,
        seam::Platform::Web => Platform::Web,
        seam::Platform::Desktop => Platform::Desktop,
    }
}

fn error_into_wit(error: seam::Error) -> Error {
    match error {
        seam::Error::InvalidRequest(detail) => Error::InvalidRequest(detail),
        seam::Error::Io(detail) => Error::Io(detail),
        seam::Error::Internal(detail) => Error::Internal(detail),
    }
}

struct HttpGuest;

wasip3::http::service::export!(HttpGuest);

impl wasip3::exports::http::handler::Guest for HttpGuest {
    async fn handle(request: http::Request) -> Result<http::Response, http::ErrorCode> {
        let shelf = Shelf {
            server_name: "specify-contracts-references",
            version: env!("CARGO_PKG_VERSION"),
            docs: registry::docs(),
        };
        omnia_wasi_http::serve(mcp::router(shelf), request).await
    }
}
