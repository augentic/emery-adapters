//! Canonical HTTP routes and messaging topics.
//!
//! The root-package guest builds its HTTP and messaging routers from these
//! tables, and domain crates publish to the same topic constants their
//! consumers subscribe to, so producers and consumers agree on the surface
//! by construction rather than by review.

/// HTTP ingress paths served by the guest.
pub mod http {
    /// Train SOAP/XML position ingress (served by soap-connector).
    pub const TRAIN_XML: &str = "/inbound/xml";
}

/// Environment-agnostic topic suffixes. Prefix with `{env}-` via
/// [`crate::config::topic`] before publishing or subscribing.
pub mod topic {
    /// Raw XML train updates (soap-connector to position-adapter).
    pub const TRAIN: &str = "realtime-train.v1";

    /// Vehicle location events (emitted by position-adapter).
    pub const TRAIN_POSITIONS: &str = "realtime-train-positions.v1";
}
