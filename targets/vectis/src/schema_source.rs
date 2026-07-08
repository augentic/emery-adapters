//! Embedded schema sources shared between validate and the schema retrieval surface.

/// Canonical tool-owned `tokens.schema.json` (the tool-owned schema and catalog decisions D1).
pub const TOKENS_SCHEMA_SOURCE: &str = include_str!("../schemas/tokens.schema.json");

/// Canonical tool-owned `assets.schema.json` (the tool-owned schema and catalog decisions D1).
pub const ASSETS_SCHEMA_SOURCE: &str = include_str!("../schemas/assets.schema.json");

/// Canonical tool-owned `composition.schema.json` (the tool-owned schema and catalog decisions D1).
/// Shared between `layout` mode (unwired-subset runtime) and
/// `composition` mode (full lifecycle runtime).
pub const COMPOSITION_SCHEMA_SOURCE: &str = include_str!("../schemas/composition.schema.json");

/// Known schema names, in the CLI's historical listing order.
pub const SCHEMA_NAMES: [&str; 3] = ["tokens", "assets", "composition"];

/// Look up a tool-owned embedded schema source by name.
#[must_use]
pub fn schema(name: &str) -> Option<&'static str> {
    match name {
        "tokens" => Some(TOKENS_SCHEMA_SOURCE),
        "assets" => Some(ASSETS_SCHEMA_SOURCE),
        "composition" => Some(COMPOSITION_SCHEMA_SOURCE),
        _ => None,
    }
}
