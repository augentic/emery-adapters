//! Extracts claims from a tree of written documentation.
//!
//! A documentation source is a written tree — specifications, guides,
//! decision records. [`survey`] cuts it one top-level directory at a time,
//! and the guest hands the seams to `emery_sdk::mine`, which lends each its
//! own directory and joins the claims into one document. The guest exports
//! the `source-adapter` world on `wasm32` alone, so the survey is tested
//! natively.

#[cfg(target_arch = "wasm32")]
mod guest;
mod registry {
    emery_sdk::registry!();
}
mod survey;

use emery_sdk::SourceKind;

pub use self::registry::docs;
pub use self::survey::survey;

/// The kind of source the adapter reads.
pub const KIND: SourceKind = SourceKind::Documentation;
