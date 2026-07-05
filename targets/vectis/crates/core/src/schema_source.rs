//! Embedded schema sources shared between [`crate::validate`]
//! (lazy-compiled validators) and the extension's `schema` subcommand
//! (raw schema retrieval).
//!
//! The `.json` sources stay physically under
//! `targets/vectis/extension/schemas/` — the tool-owned location the
//! Vectis codex ([`rules/vectis.mdc`](../../../rules/vectis.mdc)) pins
//! — and are embedded here by relative path until RFC-61 Step 5
//! retires the extension crate and moves them.

/// Canonical tool-owned `tokens.schema.json` (the tool-owned schema and catalog decisions D1).
pub const TOKENS_SCHEMA_SOURCE: &str =
    include_str!("../../../extension/schemas/tokens.schema.json");

/// Canonical tool-owned `assets.schema.json` (the tool-owned schema and catalog decisions D1).
pub const ASSETS_SCHEMA_SOURCE: &str =
    include_str!("../../../extension/schemas/assets.schema.json");

/// Canonical tool-owned `composition.schema.json` (the tool-owned schema and catalog decisions D1).
/// Shared between `layout` mode (unwired-subset runtime) and
/// `composition` mode (full lifecycle runtime).
pub const COMPOSITION_SCHEMA_SOURCE: &str =
    include_str!("../../../extension/schemas/composition.schema.json");
