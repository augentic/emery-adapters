//! The vectis adapter guest: `wasm32` shim over `specify-vectis-core`.
//! See `specify_guest_kit::adapter` for the shim contract.
#![cfg(target_arch = "wasm32")]

specify_guest_kit::target_adapter! {
    name: "vectis",
    core: specify_vectis_core,
}
