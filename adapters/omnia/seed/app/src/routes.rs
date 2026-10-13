//! The router list: every stem's routes on one router, one line per slice.

use omnia_sdk::api::Client;

use crate::{Capabilities, OWNER};

/// The router: every stem's routes over one [`Client`].
///
/// One line per slice after `Router::new()`, each added at the end.
pub fn router<P: Capabilities>(provider: P) -> axum::Router {
    let router = axum::Router::new();
    router.with_state(Client::new(OWNER, provider))
}
