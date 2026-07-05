//! The documentation adapter guest: `wasm32` shim over
//! `specify-documentation-core`.
//!
//! Exports the `augentic:specify` `source-adapter` world — `survey` /
//! `extract` route into the wasm-free core's judgment operations with the
//! WASI-backed `Model` provider — plus `wasi:http/incoming-handler`,
//! serving the core's embedded prose registry through the guest-kit
//! `Shelf` as an MCP reference shelf (`list_docs` / `read_doc` tools,
//! `doc://` resources).
//!
//! The seam operations are `async func`s, so the exports async-lift and the
//! judgment legs await the async `omnia:model/completion.create` import
//! directly — a sync-lifted export may never block, so there is no sync
//! bridge to hide behind. The MCP grant URL is read from the environment
//! (`SPECIFY_DOCUMENTATION_MCP_URL`, the guest-kit `mcp_url` convention),
//! never hardcoded — absent, judgment legs run without a reference grant.
#![cfg(target_arch = "wasm32")]

mod bindings {
    //! `wit_bindgen::generate!` output for the `source-adapter` world. The
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

    use super::DocumentationAdapter;

    wit_bindgen::generate!({
        world: "source-adapter",
        path: "../../wit",
        // The seam operations are `async func`s (judgment legs await the
        // async `omnia:model` import mid-call), so the exports async-lift.
        async: true,
    });

    export!(DocumentationAdapter);
}

use std::path::Path;

use bindings::exports::augentic::specify::source::{
    AdapterId, Authority, Backing, Claim, ClaimKind, Error, Evidence, Guest, Lead,
};
use omnia_guest::mcp;
use specify_documentation_core::{operations, registry};
use specify_guest_kit::shelf::{Shelf, mcp_url};
use specify_guest_kit::{Model, seam};
use wasip3::http::types as http;

/// The WASI-backed judgment provider: the [`Model`] trait's `wasm32`
/// default method body delegates to the `omnia-wasi-model` bindings.
struct WasiModel;

impl Model for WasiModel {}

struct DocumentationAdapter;

impl Guest for DocumentationAdapter {
    async fn survey(id: AdapterId) -> Result<Vec<Lead>, Error> {
        let url = mcp_url("documentation");
        let ctx = context(&id, url.as_deref());
        operations::survey(&WasiModel, &ctx)
            .await
            .map(|leads| leads.into_iter().map(lead_into_wit).collect())
            .map_err(error_into_wit)
    }

    async fn extract(id: AdapterId, lead: Lead) -> Result<Evidence, Error> {
        let lead = lead_into_core(lead);
        let url = mcp_url("documentation");
        let ctx = context(&id, url.as_deref());
        operations::extract(&WasiModel, &ctx, &lead)
            .await
            .map(evidence_into_wit)
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

fn lead_into_core(lead: Lead) -> seam::Lead {
    seam::Lead {
        lead: lead.lead,
        synopsis: lead.synopsis,
        topics: lead.topics,
    }
}

fn lead_into_wit(lead: seam::Lead) -> Lead {
    Lead {
        lead: lead.lead,
        synopsis: lead.synopsis,
        topics: lead.topics,
    }
}

fn evidence_into_wit(evidence: seam::Evidence) -> Evidence {
    Evidence {
        authority: authority_into_wit(evidence.authority),
        claims: evidence.claims.into_iter().map(claim_into_wit).collect(),
    }
}

const fn authority_into_wit(authority: seam::Authority) -> Authority {
    match authority {
        seam::Authority::Intent => Authority::Intent,
        seam::Authority::Documentation => Authority::Documentation,
        seam::Authority::Behaviour => Authority::Behaviour,
    }
}

fn claim_into_wit(claim: seam::Claim) -> Claim {
    Claim {
        kind: kind_into_wit(claim.kind),
        id: claim.id,
        path: claim.path,
        synopsis: claim.synopsis,
        backing: claim.backing.map(|backing| match backing {
            seam::Backing::Payload(payload) => Backing::Payload(payload),
            seam::Backing::Path(path) => Backing::Path(path),
        }),
    }
}

const fn kind_into_wit(kind: seam::ClaimKind) -> ClaimKind {
    match kind {
        seam::ClaimKind::Intent => ClaimKind::Intent,
        seam::ClaimKind::Requirement => ClaimKind::Requirement,
        seam::ClaimKind::Criterion => ClaimKind::Criterion,
        seam::ClaimKind::Decision => ClaimKind::Decision,
        seam::ClaimKind::Section => ClaimKind::Section,
        seam::ClaimKind::Diagram => ClaimKind::Diagram,
        seam::ClaimKind::Contract => ClaimKind::Contract,
        seam::ClaimKind::Example => ClaimKind::Example,
        seam::ClaimKind::Excerpt => ClaimKind::Excerpt,
        seam::ClaimKind::Type => ClaimKind::Type,
        seam::ClaimKind::Call => ClaimKind::Call,
        seam::ClaimKind::Region => ClaimKind::Region,
        seam::ClaimKind::Container => ClaimKind::Container,
        seam::ClaimKind::Leaf => ClaimKind::Leaf,
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
            server_name: "specify-documentation-references",
            version: env!("CARGO_PKG_VERSION"),
            docs: registry::docs(),
        };
        omnia_wasi_http::serve(mcp::router(shelf), request).await
    }
}
