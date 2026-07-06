//! The contracts adapter guest: `wasm32` shim over
//! `specify-contracts-core`. See `specify_guest_kit::target` for the
//! shim contract.
#![cfg(target_arch = "wasm32")]

use specify_guest_kit::target::{
    AdapterId, Changeset, Error, Guest, Input, Manifest, Report, WorkingTree,
};
use specify_guest_kit::{WasiModel, seam, shelf};

struct Adapter;
specify_guest_kit::target::export!(Adapter with_types_in specify_guest_kit::target);

impl Guest for Adapter {
    fn describe(_id: AdapterId) -> Manifest {
        specify_contracts_core::operations::describe().into()
    }

    async fn guidance(_id: AdapterId) -> Result<String, Error> {
        Ok(specify_contracts_core::operations::guidance().to_string())
    }

    async fn build(
        id: AdapterId, slice: String, inputs: Vec<Input>, tree: WorkingTree,
    ) -> Result<Report, Error> {
        let inputs: Vec<seam::Input> = inputs.into_iter().map(Into::into).collect();
        let tree = seam::WorkingTree::from(tree);
        let url = shelf::mcp_url("contracts");
        let ctx = seam::Context::guest(&id, url.as_deref());
        specify_contracts_core::operations::build(&WasiModel, &ctx, &slice, &inputs, &tree)
            .await
            .map(Into::into)
            .map_err(Into::into)
    }

    async fn merge(
        id: AdapterId, slice: String, delta: Changeset, tree: WorkingTree,
    ) -> Result<Report, Error> {
        let delta = seam::Changeset::from(delta);
        let tree = seam::WorkingTree::from(tree);
        let url = shelf::mcp_url("contracts");
        let ctx = seam::Context::guest(&id, url.as_deref());
        specify_contracts_core::operations::merge(&WasiModel, &ctx, &slice, &delta, &tree)
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
            server_name: "specify-contracts-references",
            version: env!("CARGO_PKG_VERSION"),
            docs: specify_contracts_core::registry::docs(),
        }
        .serve(request)
        .await
    }
}
