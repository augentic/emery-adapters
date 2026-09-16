//! Extracts claims from a TypeScript or JavaScript code tree.
//!
//! A code tree is mined one externally visible surface at a time — a route, a
//! command, a job, an exported API — with the whole tree in view. Which
//! modules serve one surface is no directory layout's to state, so
//! [`survey`] asks the model once under `prompts/survey.md`; and a handler's
//! behaviour runs through its imports and `tsconfig.json`, so each seam is
//! lent the root and told which files are its own. The guest hands the
//! seams to `emery_sdk::mine` and exports the `source-adapter` world on
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
pub const KIND: SourceKind = SourceKind::Behaviour;
