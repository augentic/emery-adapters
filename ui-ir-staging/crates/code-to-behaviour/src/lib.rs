//! `code-to-behaviour`: read a React + TypeScript source tree and write the
//! ui-ir behaviour layer (`behaviour.json`) with a `file#L` anchor on every
//! entry. Deterministic and model-free: a fact is written only when a row of
//! the hand-written mapping table (`ui-ir-staging/vocab/behaviour-mappings.json`)
//! says how to read it. [`evidence`] carries a behaviour document to Emery.

mod evidence;
pub mod extract;
pub mod syntax;

pub use evidence::evidence;
pub use extract::{Options, Output, extract};
