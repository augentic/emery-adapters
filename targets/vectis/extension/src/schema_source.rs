//! Embedded schema sources shared between `validate` (lazy-compiled
//! validators) and `schema` (raw schema retrieval).

/// Canonical tool-owned `tokens.schema.json` (the tool-owned schema and catalog decisions D1).
pub const TOKENS_SCHEMA_SOURCE: &str = include_str!("../schemas/tokens.schema.json");

/// Canonical tool-owned `assets.schema.json` (the tool-owned schema and catalog decisions D1).
pub const ASSETS_SCHEMA_SOURCE: &str = include_str!("../schemas/assets.schema.json");

/// Canonical tool-owned `composition.schema.json` (the tool-owned schema and catalog decisions D1).
/// Shared between `layout` mode (unwired-subset runtime) and
/// `composition` mode (full lifecycle runtime).
pub const COMPOSITION_SCHEMA_SOURCE: &str = include_str!("../schemas/composition.schema.json");
