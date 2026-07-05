//! Embedded schema sources, re-exported from `specify-vectis-core`
//! (which owns the lazy-compiled validators built from them). The
//! `.json` files stay under this crate's `schemas/` — the tool-owned
//! location the Vectis codex pins — and the core embeds them by
//! relative path.

pub use specify_vectis_core::schema_source::{
    ASSETS_SCHEMA_SOURCE, COMPOSITION_SCHEMA_SOURCE, TOKENS_SCHEMA_SOURCE,
};
