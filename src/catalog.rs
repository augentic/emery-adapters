//! The first-party adapter catalog declaration.
//!
//! Every first-party source and target adapter, linked once on its
//! axis, validated by [`native::Catalog::builder`]. Consumed by the
//! `eval` composition example at `examples/eval/`. The inventory check
//! in `tests/catalog.rs` pins the expected entries and the global
//! published-name uniqueness across axes.

/// The validated first-party catalog.
///
/// # Errors
///
/// Returns [`native::Error::Catalog`] when an adapter identity is
/// malformed or registered twice on one axis.
pub fn catalog() -> Result<native::Catalog, native::Error> {
    native::Catalog::builder()
        .source::<captures::Adapter>()
        .source::<documentation::Adapter>()
        .source::<intent::Adapter>()
        .source::<screenshots::Adapter>()
        .source::<typescript::Adapter>()
        .target::<contracts::Adapter>()
        .target::<omnia_target::Adapter>()
        .target::<vectis::Adapter>()
        .build()
}
