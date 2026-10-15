//! Block Management API client.

use anyhow::{Context, Result};
use bytes::Bytes;
use http::Method;
use http::header::AUTHORIZATION;
use http_body_util::Empty;
use omnia_sdk::{Config, HttpRequest, Identity};

use crate::config;

/// Retrieves the identifiers of the vehicles allocated to the trip with the
/// given external reference.
///
/// # Errors
///
/// Returns an error when the block management API request fails or the
/// response cannot be deserialized.
pub async fn trip_allocations<P>(external_ref_id: &str, provider: &P) -> Result<Vec<String>>
where
    P: Config + HttpRequest + Identity,
{
    let url = Config::get(provider, config::BLOCK_MGT_URL).await?;
    let identity = Config::get(provider, config::AZURE_IDENTITY).await?;

    let token = Identity::access_token(provider, identity).await?;

    let request = http::Request::builder()
        .method(Method::GET)
        .uri(format!("{url}/allocations/trips?externalRefId={external_ref_id}"))
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .body(Empty::<Bytes>::new())
        .context("building trip allocations request")?;

    let response =
        HttpRequest::fetch(provider, request).await.context("fetching trip allocations")?;

    let body = response.into_body();
    let allocated: Vec<String> =
        serde_json::from_slice(&body).context("deserializing trip allocations response")?;

    Ok(allocated)
}
