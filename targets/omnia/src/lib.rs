//! The omnia adapter guest: `wasm32` shim over `specify-omnia-core`.
//! See `specify_guest_kit::adapter` for the shim contract.
#![cfg(target_arch = "wasm32")]

specify_guest_kit::target_adapter! {
    name: "omnia",
    core: specify_omnia_core,
}
