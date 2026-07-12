//! The `wasm32` shim: bindings and export glue over the wasm-free
//! sibling modules. See `adapter::source` for the shim contract.

use adapter::source::{AdapterId, AdapterMetadata, Error, Evidence, Guest, Lead};
use adapter::{WasiModel, references, seam};

use crate::{operations, registry};

struct Adapter;
adapter::source::export!(Adapter with_types_in adapter::source);

impl Guest for Adapter {
    fn metadata(_id: AdapterId) -> AdapterMetadata {
        operations::metadata().into()
    }

    async fn survey(id: AdapterId) -> Result<Vec<Lead>, Error> {
        let url = references::mcp_url("intent");
        let ctx = seam::Context::guest(&id, url.as_deref());
        operations::survey(&WasiModel, &ctx)
            .await
            .map(|leads| leads.into_iter().map(Into::into).collect())
            .map_err(Into::into)
    }

    async fn extract(id: AdapterId, lead: Lead) -> Result<Evidence, Error> {
        let lead = seam::Lead::from(lead);
        let url = references::mcp_url("intent");
        let ctx = seam::Context::guest(&id, url.as_deref());
        operations::extract(&WasiModel, &ctx, &lead).await.map(Into::into).map_err(Into::into)
    }
}

struct HttpGuest;
wasip3::http::service::export!(HttpGuest);

impl wasip3::exports::http::handler::Guest for HttpGuest {
    async fn handle(
        request: wasip3::http::types::Request,
    ) -> Result<wasip3::http::types::Response, wasip3::http::types::ErrorCode> {
        references::References {
            server_name: "intent-references",
            version: env!("CARGO_PKG_VERSION"),
            docs: registry::docs(),
        }
        .serve(request)
        .await
    }
}
