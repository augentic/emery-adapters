//! Configuration keys and helpers.
//!
//! Every configuration key read by the guest is declared here so there is a
//! single catalog of what a deployment must provide. `.env.example` carries
//! sample values for each key.

use omnia_sdk::Config;

/// Deployment environment (e.g. `dev`, `prod`). Prefixes every messaging
/// topic — see [`topic`].
pub const ENV: &str = "ENV";

/// Base URL of the Block Management API.
pub const BLOCK_MGT_URL: &str = "BLOCK_MGT_URL";

/// Identity used to acquire access tokens for the upstream APIs.
pub const AZURE_IDENTITY: &str = "AZURE_IDENTITY";

/// Base URL of the CC Static (static GTFS) API.
pub const CC_STATIC_URL: &str = "CC_STATIC_URL";

/// Resolve the deployment environment.
///
/// Falls back to `dev` — with a warning — when [`ENV`] is not configured, so
/// the guest remains runnable out of the box. Real deployments should always
/// set `ENV` explicitly; the warning makes a missing key visible instead of
/// silently mis-prefixing every topic.
pub async fn env(provider: &impl Config) -> String {
    match Config::get(provider, ENV).await {
        Ok(env) => env,
        Err(error) => {
            tracing::warn!("`ENV` is not configured, defaulting to `dev`: {error}");
            "dev".to_string()
        }
    }
}

/// Build a fully-qualified messaging topic from an environment-agnostic
/// suffix (see [`crate::routes::topic`]), e.g. `realtime-train.v1` becomes
/// `dev-realtime-train.v1`.
pub async fn topic(provider: &impl Config, suffix: &str) -> String {
    topic_for(&env(provider).await, suffix)
}

/// Build a fully-qualified messaging topic from an already-resolved
/// environment. Prefer this over [`topic`] when qualifying several topics so
/// the environment is only resolved once.
#[must_use]
pub fn topic_for(env: &str, suffix: &str) -> String {
    format!("{env}-{suffix}")
}
