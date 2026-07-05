//! Embedded schema sources, re-exported from `specify-vectis-core`
//! (which owns the lazy-compiled validators built from them). The
//! `.json` files live under the core crate's `schemas/` as of RFC-61
//! Step 5 Milestone A1.

pub use specify_vectis_core::schema_source::{
    ASSETS_SCHEMA_SOURCE, COMPOSITION_SCHEMA_SOURCE, TOKENS_SCHEMA_SOURCE,
};
