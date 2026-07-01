//! Integration tests for the vectis validation engine, split by concern
//! (paths, layout, tokens, assets, composition). Shared fixtures and
//! assertion helpers live in [`engine_support`].

mod engine_support;

#[path = "engine/android_scaffold.rs"]
mod android_scaffold;
#[path = "engine/android_setup.rs"]
mod android_setup;
#[path = "engine/assets.rs"]
mod assets;
#[path = "engine/composition.rs"]
mod composition;
#[path = "engine/core_scaffold.rs"]
mod core_scaffold;
#[path = "engine/infer.rs"]
mod infer;
#[path = "engine/ios_scaffold.rs"]
mod ios_scaffold;
#[path = "engine/layout.rs"]
mod layout;
#[path = "engine/materialize.rs"]
mod materialize;
#[path = "engine/materialize_acceptance_fixture.rs"]
mod materialize_acceptance_fixture;
#[path = "engine/materialize_app_icon.rs"]
mod materialize_app_icon;
#[path = "engine/materialize_illustrations.rs"]
mod materialize_illustrations;
#[path = "engine/paths.rs"]
mod paths;
#[path = "engine/prepare_scope.rs"]
mod prepare_scope;
#[path = "engine/suppression_scan.rs"]
mod suppression_scan;
#[path = "engine/svg_normalize.rs"]
mod svg_normalize;
#[path = "engine/tokens.rs"]
mod tokens;
#[path = "engine/verify.rs"]
mod verify;
