//! Project composition inline `test_id` values into `ui-contract/test-ids.yaml`.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use crate::VectisError;
use crate::composition_manifests::effective_composition;
use crate::validate::engine::composition::{
    collect_test_id_values, is_kebab_test_id, kebab_to_maestro_key,
};

/// Relative path to the flat test-id registry consumed by exemplar codegen.
pub const REGISTRY_REL: &str = "ui-contract/test-ids.yaml";

type Entries = BTreeMap<String, String>;

/// Harvest `MAESTRO_*` → kebab entries from the effective composition document.
///
/// Pass `active_slice` during `emery build` so slice deltas merge onto baseline;
/// pass `None` for post-merge / desk verify (baseline only).
///
/// # Errors
///
/// Returns [`VectisError::InvalidProject`] when composition is unreadable, invalid,
/// or when the same `test_id` value appears on more than one element in the
/// effective (merged) composition document.
pub fn harvest_entries(
    project_root: &Path, active_slice: Option<&str>,
) -> Result<Entries, VectisError> {
    let document = effective_composition(project_root, active_slice)?;
    let mut entries = Entries::new();
    let mut errors = Vec::new();

    let mut collected: BTreeMap<String, String> = BTreeMap::new();
    collect_test_id_values(&document, "", &mut collected);

    // `kebab_to_maestro_key` is injective for schema kebab ids, so duplicate
    // `test_id` values on different elements collapse into one Maestro constant
    // unless we enforce document-wide uniqueness here (validate runs per-file only).
    let mut key_source: BTreeMap<String, String> = BTreeMap::new();

    for (json_path, value) in collected {
        if !is_kebab_test_id(&value) {
            errors.push(format!(
                "`{value}` at effective-composition:{json_path} must match \
                 `[a-z][a-z0-9]*(-[a-z0-9]+)*`"
            ));
            continue;
        }

        let key = kebab_to_maestro_key(&value);
        let source = format!("effective:{json_path}");
        if let Some(first_path) = key_source.get(&key) {
            errors.push(format!(
                "duplicate `test_id` `{value}` at {source} \
                 (also at {first_path}); test ids must be unique within the document"
            ));
        } else {
            key_source.insert(key.clone(), source);
            entries.insert(key, value);
        }
    }

    if errors.is_empty() {
        Ok(entries)
    } else {
        Err(VectisError::InvalidProject {
            message: format!("invalid composition test-id harvest:\n- {}", errors.join("\n- ")),
        })
    }
}

/// Write `ui-contract/test-ids.yaml` from the effective composition.
///
/// # Errors
///
/// Propagates [`harvest_entries`] failures and I/O errors while writing the derived file.
pub fn write_generated(project_root: &Path, active_slice: Option<&str>) -> Result<(), VectisError> {
    let entries = harvest_entries(project_root, active_slice)?;
    let path = project_root.join(REGISTRY_REL);
    let body = format_generated_yaml(&entries);

    if path.is_file() {
        let existing = fs::read_to_string(&path)?;
        if existing == body {
            return Ok(());
        }
    } else if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(path, body)?;
    Ok(())
}

/// Parse a flat `test_ids:` map from generated or ui-contract YAML.
///
/// # Errors
///
/// Returns [`VectisError::InvalidProject`] when the file is present but not a valid flat map.
pub fn parse_flat_file(path: &Path) -> Result<Entries, VectisError> {
    if !path.is_file() {
        return Ok(Entries::new());
    }

    let content = fs::read_to_string(path).map_err(|err| VectisError::InvalidProject {
        message: format!("{} not readable: {err}", path.display()),
    })?;

    parse_flat_yaml(&content).map_err(|message| VectisError::InvalidProject { message })
}

/// Format the canonical flat test-id registry YAML document.
///
/// Keys and values are emitted unquoted. Safe only while values remain
/// schema kebab-case and keys remain `MAESTRO_*` (no `:`, `#`, or whitespace).
#[must_use]
pub fn format_generated_yaml(entries: &Entries) -> String {
    let mut out = String::from(
        "# @generated from composition inline `test_id` by Vectis; do not edit.\n\
         #\n\
         # Refreshed during `emery build` after the composition validator gate passes.\n\
         # Consumed by `cargo make generate-bindings` (exemplar codegen).\n\n\
         test_ids:\n",
    );

    if entries.is_empty() {
        out.push_str("  {}\n");
    } else {
        for (key, value) in entries {
            writeln!(&mut out, "  {key}: {value}").expect("writing to a String cannot fail");
        }
    }

    out
}

fn parse_flat_yaml(content: &str) -> Result<Entries, String> {
    let document: BTreeMap<String, Entries> =
        serde_saphyr::from_str(content).map_err(|err| format!("invalid YAML: {err}"))?;

    let mut fields = document;
    let Some(entries) = fields.remove("test_ids") else {
        return Err("missing top-level `test_ids` map".into());
    };

    if let Some(unexpected) = fields.keys().next() {
        return Err(format!("unexpected top-level key `{unexpected}`"));
    }

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_flat_yaml() {
        let entries = Entries::from([
            ("MAESTRO_SPLASH_CTA".to_owned(), "splash-cta".to_owned()),
            ("MAESTRO_STUB_MESSAGE".to_owned(), "stub-message".to_owned()),
        ]);
        let yaml = format_generated_yaml(&entries);
        let parsed = parse_flat_yaml(&yaml).expect("parse");
        assert_eq!(parsed, entries);
    }

    #[test]
    fn empty_registry_serializes_as_empty_map() {
        let yaml = format_generated_yaml(&Entries::new());
        assert!(yaml.contains("test_ids:\n  {}\n"));
        assert!(parse_flat_yaml(&yaml).expect("parse").is_empty());
    }
}
