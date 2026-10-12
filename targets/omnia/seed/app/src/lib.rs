//! The application: every stem's routes on one router over one provider.

mod routes;

use omnia_sdk::{
    BlobStore, Broadcast, Config, DocumentStore, HttpRequest, Identity, Publish, StateStore,
    TableStore,
};
pub use routes::router;

/// The tenant that owns this deployment.
pub const OWNER: &str = "app";

/// Every capability a stem may bound its provider on.
pub trait Capabilities:
    BlobStore
    + Broadcast
    + Config
    + DocumentStore
    + HttpRequest
    + Identity
    + Publish
    + StateStore
    + TableStore
    + Send
    + Sync
    + 'static
{
}

impl<P> Capabilities for P where
    P: BlobStore
        + Broadcast
        + Config
        + DocumentStore
        + HttpRequest
        + Identity
        + Publish
        + StateStore
        + TableStore
        + Send
        + Sync
        + 'static
{
}

/// The production provider: WASI's own implementation of every capability.
#[cfg(target_arch = "wasm32")]
pub struct Provider;

#[cfg(target_arch = "wasm32")]
impl BlobStore for Provider {}
#[cfg(target_arch = "wasm32")]
impl Broadcast for Provider {}
#[cfg(target_arch = "wasm32")]
impl Config for Provider {}
#[cfg(target_arch = "wasm32")]
impl DocumentStore for Provider {}
#[cfg(target_arch = "wasm32")]
impl HttpRequest for Provider {}
#[cfg(target_arch = "wasm32")]
impl Identity for Provider {}
#[cfg(target_arch = "wasm32")]
impl Publish for Provider {}
#[cfg(target_arch = "wasm32")]
impl StateStore for Provider {}
#[cfg(target_arch = "wasm32")]
impl TableStore for Provider {}

/// The WASI HTTP export.
#[cfg(target_arch = "wasm32")]
pub struct Http;

#[cfg(target_arch = "wasm32")]
wasip3::http::service::export!(Http);

#[cfg(target_arch = "wasm32")]
impl wasip3::exports::http::handler::Guest for Http {
    async fn handle(
        request: wasip3::http::types::Request,
    ) -> Result<wasip3::http::types::Response, wasip3::http::types::ErrorCode> {
        omnia_sdk::api::http::serve(router(Provider), request).await
    }
}
