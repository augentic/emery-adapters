//! The vectis adapter guest: `wasm32` shim over `core`.
//! See `adapter::target` for the shim contract.
#![cfg(target_arch = "wasm32")]

use adapter::target::{AdapterId, Changeset, Error, Guest, Input, Manifest, Report, WorkingTree};
use adapter::{WasiModel, seam, references};

struct Adapter;
adapter::target::export!(Adapter with_types_in adapter::target);

impl Guest for Adapter {
    fn describe(_id: AdapterId) -> Manifest {
        core::operations::describe().into()
    }

    async fn guidance(_id: AdapterId) -> Result<String, Error> {
        Ok(core::operations::guidance().to_string())
    }

    async fn build(
        id: AdapterId, slice: String, inputs: Vec<Input>, tree: WorkingTree,
    ) -> Result<Report, Error> {
        let inputs: Vec<seam::Input> = inputs.into_iter().map(Into::into).collect();
        let tree = seam::WorkingTree::from(tree);
        let url = references::mcp_url("vectis");
        let ctx = seam::Context::guest(&id, url.as_deref());
        core::operations::build(&WasiModel, &ctx, &slice, &inputs, &tree)
            .await
            .map(Into::into)
            .map_err(Into::into)
    }

    async fn merge(
        id: AdapterId, slice: String, delta: Changeset, tree: WorkingTree,
    ) -> Result<Report, Error> {
        let delta = seam::Changeset::from(delta);
        let tree = seam::WorkingTree::from(tree);
        let url = references::mcp_url("vectis");
        let ctx = seam::Context::guest(&id, url.as_deref());
        core::operations::merge(&WasiModel, &ctx, &slice, &delta, &tree)
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
        references::References {
            server_name: "vectis-references",
            version: env!("CARGO_PKG_VERSION"),
            docs: core::registry::docs(),
        }
        .serve(request)
        .await
    }
}
