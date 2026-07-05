//! The intent adapter guest: `wasm32` shim over `specify-intent-core`.
//! See `specify_guest_kit::adapter` for the shim contract.
#![cfg(target_arch = "wasm32")]

specify_guest_kit::source_adapter! {
    name: "intent",
    core: specify_intent_core,
}
