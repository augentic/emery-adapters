//! The TypeScript / JavaScript adapter guest: `wasm32` shim over
//! `typescript-core`. See `adapter::source` for the
//! shim contract.
#![cfg(target_arch = "wasm32")]

use adapter::source::{AdapterId, Error, Evidence, Guest, Lead, Manifest};
use adapter::{WasiModel, seam, shelf};

struct Adapter;
adapter::source::export!(Adapter with_types_in adapter::source);

impl Guest for Adapter {
    fn describe(_id: AdapterId) -> Manifest {
        typescript_core::operations::describe().into()
    }

    async fn survey(id: AdapterId) -> Result<Vec<Lead>, Error> {
        let url = shelf::mcp_url("typescript");
        let ctx = seam::Context::guest(&id, url.as_deref());
        typescript_core::operations::survey(&WasiModel, &ctx)
            .await
            .map(|leads| leads.into_iter().map(Into::into).collect())
            .map_err(Into::into)
    }

    async fn extract(id: AdapterId, lead: Lead) -> Result<Evidence, Error> {
        let lead = seam::Lead::from(lead);
        let url = shelf::mcp_url("typescript");
        let ctx = seam::Context::guest(&id, url.as_deref());
        typescript_core::operations::extract(&WasiModel, &ctx, &lead)
            .await
            .map(Into::into)
            .map_err(Into::into)
    }
}

struct HttpGuest;
wasip3::http::service::export!(HttpGuest);

impl wasip3::exports::http::handler::Guest for HttpGuest {
    async fn handle(
        request: wasip3::http::types::Request,
    ) -> Result<wasip3::http::types::Response, wasip3::http::types::ErrorCode> {
        shelf::Shelf {
            server_name: "typescript-references",
            version: env!("CARGO_PKG_VERSION"),
            docs: typescript_core::registry::docs(),
        }
        .serve(request)
        .await
    }
}
