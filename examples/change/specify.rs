//! The specify workflow guest: identical to the engine's root cdylib.
//!
//! The `guest` crate (in `augentic/specify`) owns the `workflow`-world
//! WIT bindings, the WIT-backed provider, and the transport wiring;
//! this example is the same single macro invocation the engine's
//! `src/lib.rs` makes.
#![cfg(target_arch = "wasm32")]

guest::export!();
