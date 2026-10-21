//! The ui-ir behaviour contract, staged outside ui-ir.
//!
//! Three things every behaviour adapter shares and nothing else: the schema of
//! `behaviour.json` ([`SCHEMA`]), the hand-written mapping table
//! ([`MAPPINGS`], read through [`Mappings`]), and the node id rule
//! ([`node_id`]). Plugins depend on this crate only, never on each other.
//! After integration this module becomes part of the `ui-ir` crate and the
//! files under `ui-ir-staging/` move into the ui-ir repository.

use std::fmt::Write as _;

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

/// `schema/core/behaviour.schema.json`.
pub const SCHEMA: &str = include_str!("../../../schema/core/behaviour.schema.json");

/// `vocab/behaviour-mappings.json`.
pub const MAPPINGS: &str = include_str!("../../../vocab/behaviour-mappings.json");

/// The file a behaviour source adapter writes in an IR directory.
pub const BEHAVIOUR: &str = "behaviour.json";

/// The report written beside the IR directory: facts with no mapping row.
pub const UNMAPPED: &str = "behaviour.unmapped.json";

/// The composition node id of the element at `path` in `computed.json`.
///
/// `screen + "/n-" + first 16 hex digits of sha256(path)`; see
/// `ui-ir-staging/docs/node-id.md`.
#[must_use]
pub fn node_id(screen: &str, path: &str) -> String {
    let digest = Sha256::digest(path.as_bytes());
    let mut out = String::with_capacity(screen.len() + 19);
    out.push_str(screen);
    out.push_str("/n-");
    for byte in digest.iter().take(8) {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// Lower-case hex of the full sha256 of `bytes`.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for byte in digest {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// One row of the mapping table.
#[derive(Debug, Clone)]
pub struct Row {
    /// The source key (`jsx:a`, `call:sonner#toast.error`, ...); may end in `*`.
    pub source: String,
    /// The literal value the row is limited to.
    pub when: Option<String>,
    /// The converter applied.
    pub converter: String,
    /// `core`, `web`, `mobile`, `ios`, `android`, or `drop`.
    pub placement: String,
    /// The whole row, for converter parameters.
    pub raw: Map<String, Value>,
}

impl Row {
    /// A string parameter of the row.
    #[must_use]
    pub fn param(&self, name: &str) -> Option<&str> {
        self.raw.get(name).and_then(Value::as_str)
    }

    /// Whether `method` is one of the row's `methods`.
    #[must_use]
    pub fn has_method(&self, method: &str) -> bool {
        self.raw
            .get("methods")
            .and_then(Value::as_array)
            .is_some_and(|methods| methods.iter().any(|m| m.as_str() == Some(method)))
    }

    fn matches(&self, key: &str, value: Option<&str>) -> bool {
        let source_ok = self
            .source
            .strip_suffix('*')
            .map_or_else(|| self.source == key, |prefix| key.starts_with(prefix));
        source_ok && self.when.as_deref().is_none_or(|when| value == Some(when))
    }
}

/// The parsed mapping table.
#[derive(Debug, Clone)]
pub struct Mappings {
    rows: Vec<Row>,
}

impl Mappings {
    /// The embedded table.
    ///
    /// # Panics
    ///
    /// Never at runtime for the embedded table: its shape is checked by the
    /// crate's tests.
    #[must_use]
    pub fn embedded() -> Self {
        Self::parse(MAPPINGS)
            .unwrap_or_else(|reason| panic!("embedded behaviour mappings: {reason}"))
    }

    /// Parse a table.
    ///
    /// # Errors
    ///
    /// When the text is not a table of rows each with `source`, `converter`,
    /// and `placement`.
    pub fn parse(text: &str) -> Result<Self, String> {
        let doc: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let entries = doc
            .get("entries")
            .and_then(Value::as_array)
            .ok_or_else(|| "no `entries` array".to_string())?;
        let mut rows = Vec::with_capacity(entries.len());
        for (i, entry) in entries.iter().enumerate() {
            let raw = entry.as_object().ok_or_else(|| format!("entry {i} is not an object"))?;
            let field = |name: &str| {
                raw.get(name)
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .ok_or_else(|| format!("entry {i} has no `{name}`"))
            };
            rows.push(Row {
                source: field("source")?,
                when: raw.get("when").and_then(Value::as_str).map(str::to_string),
                converter: field("converter")?,
                placement: field("placement")?,
                raw: raw.clone(),
            });
        }
        Ok(Self { rows })
    }

    /// The first row for `key` (and the literal `value`, where the row has `when`).
    #[must_use]
    pub fn lookup(&self, key: &str, value: Option<&str>) -> Option<&Row> {
        self.rows.iter().find(|row| row.matches(key, value))
    }

    /// Every row.
    #[must_use]
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }
}

/// Check one behaviour document against [`SCHEMA`]: each violation as one line.
///
/// # Errors
///
/// When the schema itself does not compile.
#[cfg(feature = "validate")]
pub fn validate(doc: &Value) -> Result<Vec<String>, String> {
    let schema: Value = serde_json::from_str(SCHEMA).map_err(|e| e.to_string())?;
    let validator = jsonschema::validator_for(&schema).map_err(|e| e.to_string())?;
    let mut out: Vec<String> = validator
        .iter_errors(doc)
        .map(|error| format!("{}: {error}", error.instance_path))
        .collect();
    out.sort();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_id_vectors() {
        assert_eq!(node_id("login", "html"), "login/n-e5e239560bc125a2");
        assert_eq!(
            node_id(
                "login",
                "html > body > div > div > div:nth-of-type(2) > div:nth-of-type(1) > div > button"
            ),
            "login/n-cf2331bab5423b71"
        );
        assert_eq!(node_id("index", "html > body"), "index/n-b364c0b0dce72fd0");
    }

    #[test]
    fn embedded_table_parses_and_looks_up() {
        let table = Mappings::embedded();
        assert!(table.rows().len() > 40);
        let row = table.lookup("call:sonner#toast.error", None).map(|r| r.converter.as_str());
        assert_eq!(row, Some("show"));
        assert!(table.lookup("attr:target", Some("_self")).is_none());
        assert_eq!(
            table.lookup("attr:target", Some("_blank")).map(|r| r.converter.as_str()),
            Some("new_tab")
        );
        let wildcard = table.lookup("jsx:@radix-ui/react-dialog#Portal", None);
        assert_eq!(wildcard.map(|r| r.converter.as_str()), Some("passthrough"));
        let exact = table.lookup("jsx:@radix-ui/react-dialog#Trigger", None);
        assert_eq!(exact.map(|r| r.converter.as_str()), Some("overlay_trigger"));
    }

    #[test]
    fn every_row_names_a_declared_converter() {
        let doc: Value = serde_json::from_str(MAPPINGS).unwrap_or_default();
        let declared =
            doc.get("converters").and_then(Value::as_object).cloned().unwrap_or_default();
        for row in Mappings::embedded().rows() {
            assert!(declared.contains_key(&row.converter), "{} -> {}", row.source, row.converter);
        }
    }

    #[cfg(feature = "validate")]
    #[test]
    fn minimal_document_is_valid() {
        let doc = serde_json::json!({
            "version": 1, "screen": "login", "routes": [], "triggers": [], "guards": [],
            "state": {}, "validation": [], "transitions": []
        });
        assert_eq!(validate(&doc), Ok(Vec::new()));
        let bad = serde_json::json!({ "version": 1, "screen": "login" });
        assert_ne!(validate(&bad).unwrap_or_default(), [] as [std::string::String; 0]);
    }
}
