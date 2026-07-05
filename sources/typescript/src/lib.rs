//! The TypeScript / JavaScript adapter guest: `wasm32` shim over
//! `specify-typescript-core`. See `specify_guest_kit::adapter` for the
//! shim contract.
#![cfg(target_arch = "wasm32")]

specify_guest_kit::source_adapter! {
    name: "typescript",
    core: specify_typescript_core,
}
