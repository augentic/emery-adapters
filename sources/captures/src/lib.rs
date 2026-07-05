//! The captures adapter guest: `wasm32` shim over
//! `specify-captures-core`. See `specify_guest_kit::adapter` for the
//! shim contract.
#![cfg(target_arch = "wasm32")]

specify_guest_kit::source_adapter! {
    name: "captures",
    core: specify_captures_core,
}
