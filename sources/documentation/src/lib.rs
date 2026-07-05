//! The documentation adapter guest: `wasm32` shim over
//! `specify-documentation-core`. See `specify_guest_kit::adapter` for
//! the shim contract.
#![cfg(target_arch = "wasm32")]

specify_guest_kit::source_adapter! {
    name: "documentation",
    core: specify_documentation_core,
}
