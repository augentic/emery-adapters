//! Extracts claims from an operator's written brief.
//!
//! An intent source is the operator's free-form brief, given inline or as a
//! one-file tree. It is never split: [`survey`] carries the brief verbatim
//! as one seam, and the guest hands it to `emery_sdk::mine`, whose document
//! carries one `intent` claim with the brief and one `requirement` claim per
//! directive it states. The guest exports the `source-adapter` world on
//! `wasm32` alone, so the survey is tested natively.

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
pub const KIND: SourceKind = SourceKind::Intent;
