//! The contracts adapter guest: `wasm32` shim over
//! `specify-contracts-core`. See `specify_guest_kit::adapter` for the
//! shim contract.
#![cfg(target_arch = "wasm32")]

specify_guest_kit::target_adapter! {
    name: "contracts",
    core: specify_contracts_core,
}
