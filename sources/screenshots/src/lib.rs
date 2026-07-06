//! The screenshots adapter guest: `wasm32` shim over
//! `specify-screenshots-core`. See `specify_guest_kit::source` for
//! the shim contract.
#![cfg(target_arch = "wasm32")]

use specify_guest_kit::source::{AdapterId, Error, Evidence, Guest, Lead};
use specify_guest_kit::{WasiModel, seam, shelf};

struct Adapter;
specify_guest_kit::source::export!(Adapter with_types_in specify_guest_kit::source);

impl Guest for Adapter {
    async fn survey(id: AdapterId) -> Result<Vec<Lead>, Error> {
        let url = shelf::mcp_url("screenshots");
        let ctx = seam::Context::guest(&id, url.as_deref());
        specify_screenshots_core::operations::survey(&WasiModel, &ctx)
            .await
            .map(|leads| leads.into_iter().map(Into::into).collect())
            .map_err(Into::into)
    }

    async fn extract(id: AdapterId, lead: Lead) -> Result<Evidence, Error> {
        let lead = seam::Lead::from(lead);
        let url = shelf::mcp_url("screenshots");
        let ctx = seam::Context::guest(&id, url.as_deref());
        specify_screenshots_core::operations::extract(&WasiModel, &ctx, &lead)
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
            server_name: "specify-screenshots-references",
            version: env!("CARGO_PKG_VERSION"),
            docs: specify_screenshots_core::registry::docs(),
        }
        .serve(request)
        .await
    }
}
