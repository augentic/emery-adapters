//! # Root-package guest
//!
//! Wires the SOAP connector and position adapter to WASI HTTP and WASI
//! Messaging with the explicit typed routers from `omnia_sdk::api`: the
//! one HTTP route is a `handle_with` binding over a provider-owning
//! `Client`, supplying the SOAP/XML codecs the vendor protocol demands, and
//! the one messaging topic dispatches through an exact-topic
//! `messaging::Router` to `position_adapter::train_update`, under a codec
//! that reads the raw XML payload. The connector's outbound topic is the
//! adapter's inbound one, so the two crates form one pipeline.
//!
//! Routes and topics come from the canonical tables in [`common::routes`].
//!
//! Only the WASI exports are `wasm32`-gated. The routers are generic over
//! the provider so the native rungs (`tests/routes.rs`, `tests/messaging.rs`)
//! drive the production tables under `omnia_test::guest::Provider` doubles.

use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
use common::{config, routes};
#[cfg(target_arch = "wasm32")]
use omnia_sdk::api::http::serve;
use omnia_sdk::api::http::{MethodFilter, RawRequest, handle_with};
use omnia_sdk::api::messaging::{self, Delivery, consume_with};
use omnia_sdk::api::{Client, DecodeError};
use omnia_sdk::{Config, HttpError, HttpRequest, Identity, Publish, StateStore};
#[cfg(target_arch = "wasm32")]
use omnia_wasi_messaging::types::{Error, Message};
use position_adapter::TrainMessage;
use soap_connector::{SoapReply, SoapXml};
#[cfg(target_arch = "wasm32")]
use tracing::Level;
#[cfg(target_arch = "wasm32")]
use wasip3::exports::http::handler::Guest;
#[cfg(target_arch = "wasm32")]
use wasip3::http::types as p3;

/// The tenant that owns this deployment.
pub const OWNER: &str = "transit";

/// Bare provider backed by the default WASI capability implementations.
///
/// Each capability trait ships its WASI-backed methods as defaults, so an
/// empty `impl` per capability is the whole production wiring.
#[cfg(target_arch = "wasm32")]
pub struct Provider;

#[cfg(target_arch = "wasm32")]
impl Config for Provider {}
#[cfg(target_arch = "wasm32")]
impl HttpRequest for Provider {}
#[cfg(target_arch = "wasm32")]
impl Identity for Provider {}
#[cfg(target_arch = "wasm32")]
impl Publish for Provider {}
#[cfg(target_arch = "wasm32")]
impl StateStore for Provider {}

/// WASI HTTP export.
#[cfg(target_arch = "wasm32")]
pub struct Http;
#[cfg(target_arch = "wasm32")]
wasip3::http::service::export!(Http);

#[cfg(target_arch = "wasm32")]
impl Guest for Http {
    #[omnia_wasi_otel::instrument(name = "http_guest_handle", level = Level::INFO)]
    async fn handle(request: p3::Request) -> Result<p3::Response, p3::ErrorCode> {
        serve(router(Provider), request).await
    }
}

/// Build the HTTP router over one provider-owning [`Client`].
///
/// Omnia creates one WASI component instance per HTTP request, so the router
/// and client are constructed inside each `handle` call. The one route is
/// the connector's SOAP/XML ingress; every other path is answered as
/// unregistered.
pub fn router<P>(provider: P) -> axum::Router
where
    P: Config + HttpRequest + Identity + Publish + StateStore + Send + Sync + 'static,
{
    axum::Router::new()
        .route(
            routes::http::TRAIN_XML,
            handle_with(
                MethodFilter::POST,
                soap_connector::receive_message,
                |raw: RawRequest<'_>| decode_soap(raw.body),
                |reply| encode_soap(&reply),
            ),
        )
        .with_state(Client::new(OWNER, provider))
}

/// Pass the SOAP body through undecoded.
///
/// The handler parses the SOAP envelope itself so a malformed body is
/// answered with the vendor's XML `<Fault>` (via the handler error's
/// `HttpError` conversion). A decoder that failed here would instead reach
/// the client as the framework's JSON `invalid_request` 400.
#[allow(clippy::unnecessary_wraps, reason = "the route codec requires a fallible decoder")]
fn decode_soap(body: &[u8]) -> Result<SoapXml, DecodeError> {
    Ok(SoapXml(body.to_vec()))
}

/// Encode the SOAP acknowledgement in the vendor's XML shape.
fn encode_soap(reply: &SoapReply) -> Response {
    match reply.to_xml() {
        Ok(xml) => ([(CONTENT_TYPE, "text/xml")], xml).into_response(),
        Err(error) => HttpError::from(error).into_response(),
    }
}

/// WASI Messaging export.
#[cfg(target_arch = "wasm32")]
pub struct Messaging;
#[cfg(target_arch = "wasm32")]
omnia_wasi_messaging::export!(Messaging with_types_in omnia_wasi_messaging);

#[cfg(target_arch = "wasm32")]
impl omnia_wasi_messaging::incoming_handler::Guest for Messaging {
    #[omnia_wasi_otel::instrument(name = "messaging_guest_handle")]
    async fn handle(message: Message) -> Result<(), Error> {
        let router = messaging_router(Provider).await;
        messaging::handle(&router, message).await
    }
}

/// Build the exact-topic messaging router over one provider-owning [`Client`].
///
/// The topic is registered with its full `{env}-` qualified name, so a
/// topic from another environment is rejected as unhandled instead of
/// silently consumed. Router failures — including handler errors with their
/// structured codes — flow back as `error.other` with the full display
/// string, since the WIT contract only carries a string. Resolving the
/// environment reads `Config`, hence the `async`.
pub async fn messaging_router<P>(provider: P) -> messaging::Router<P>
where
    P: Config + HttpRequest + Identity + Publish + StateStore + Send + Sync + 'static,
{
    let env = config::env(&provider).await;
    messaging::Router::new(Client::new(OWNER, provider)).route(
        config::topic_for(&env, routes::topic::TRAIN),
        consume_with(position_adapter::train_update, decode_train_xml),
    )
}

/// Decode an inbound train update from its raw XML payload.
fn decode_train_xml(delivery: &Delivery) -> Result<TrainMessage, DecodeError> {
    TrainMessage::from_xml(&delivery.payload).map_err(|error| DecodeError::new(error.to_string()))
}
