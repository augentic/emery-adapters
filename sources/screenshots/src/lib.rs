//! The screenshots adapter guest: `wasm32` shim over
//! `specify-screenshots-core`. See `specify_guest_kit::adapter` for
//! the shim contract.
#![cfg(target_arch = "wasm32")]

specify_guest_kit::source_adapter! {
    name: "screenshots",
    core: specify_screenshots_core,
}
