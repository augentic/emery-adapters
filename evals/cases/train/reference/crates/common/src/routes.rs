//! Canonical HTTP routes and messaging topics.
//!
//! The root-package guest builds its HTTP and messaging routers from these
//! tables, and domain crates publish to the same topic constants their
//! consumers subscribe to, so producers and consumers agree on the surface
//! by construction rather than by review.

/// HTTP ingress paths served by the guest.
///
/// The train position adapter serves none: its one ingress is the topic
/// [`topic::TRAIN`].
pub mod http {}

/// Environment-agnostic topic suffixes. Prefix with `{env}-` via
/// [`crate::config::topic`] before publishing or subscribing.
pub mod topic {
    /// Raw XML train updates (consumed by position-adapter).
    pub const TRAIN: &str = "realtime-train.v1";

    /// Vehicle location events (emitted by position-adapter).
    pub const TRAIN_POSITIONS: &str = "realtime-train-positions.v1";
}
