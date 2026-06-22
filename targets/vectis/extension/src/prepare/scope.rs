//! RFC §2.1 in-scope asset resolution for `vectis prepare build`.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::validate::engine::{
    app_icon_export_exists, collect_asset_references, conventional_export_exists,
    platform_pin_active,
};

const PROJECT_ASSETS_REL: &str = "design-system/assets.yaml";
const SLICE_ASSETS_NAME: &str = "assets.yaml";
const COMPOSITION_NAME: &str = "composition.yaml";

/// Resolved `assets.yaml` path and whether it lives in the slice tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveAssets {
    /// Absolute path to the effective inventory file.
    pub path: PathBuf,
    /// `true` when [`Self::path`] is `${SLICE_DIR}/assets.yaml`.
    pub slice_local: bool,
}

/// Asset ids that prepare should consider for materialization (RFC §2.1).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MaterializeScope {
    /// Sorted, deduplicated asset ids in scope for the active slice.
    pub asset_ids: BTreeSet<String>,
}

/// Resolve the effective `assets.yaml` with slice-local → project precedence.
#[must_use]
pub fn resolve_effective_assets(slice_dir: &Path, project_dir: &Path) -> Option<EffectiveAssets> {
    let slice_local = slice_dir.join(SLICE_ASSETS_NAME);
    if slice_local.is_file() {
        return Some(EffectiveAssets {
            path: slice_local,
            slice_local: true,
        });
    }
    let project = project_dir.join(PROJECT_ASSETS_REL);
    if project.is_file() {
        return Some(EffectiveAssets {
            path: project,
            slice_local: false,
        });
    }
    None
}

/// Whether any in-scope asset lacks on-disk exports for a declared shell platform.
#[must_use]
pub fn scope_needs_materialize(
    scope: &MaterializeScope, effective: &EffectiveAssets, shell_platforms: &[String],
) -> bool {
    if scope.asset_ids.is_empty() {
        return false;
    }
    let Ok(raw) = fs::read_to_string(&effective.path) else {
        return false;
    };
    let Ok(doc) = serde_saphyr::from_str::<Value>(&raw) else {
        return false;
    };
    let Some(assets) = doc.get("assets").and_then(Value::as_object) else {
        return false;
    };
    let assets_dir = effective.path.parent().unwrap_or_else(|| Path::new("."));

    scope.asset_ids.iter().any(|id| {
        assets
            .get(id)
            .is_some_and(|entry| asset_needs_materialize(entry, id, assets_dir, shell_platforms))
    })
}

/// Comma-separated platform tokens for declared UI shell platforms.
#[must_use]
pub fn materialize_platform_csv(shell_platforms: &[String]) -> String {
    shell_platforms.join(",")
}

/// Derive the RFC §2.1 materialization reference set for a slice build.
#[must_use]
pub fn resolve_materialize_scope(
    slice_dir: &Path, project_dir: &Path, ui_platforms: &[String], effective: &EffectiveAssets,
) -> MaterializeScope {
    let Ok(raw) = fs::read_to_string(&effective.path) else {
        return MaterializeScope::default();
    };
    let Ok(doc) = serde_saphyr::from_str::<Value>(&raw) else {
        return MaterializeScope::default();
    };
    let Some(assets) = doc.get("assets").and_then(Value::as_object) else {
        return MaterializeScope::default();
    };

    let assets_dir = effective.path.parent().unwrap_or(project_dir);

    let mut reference_ids = collect_reference_ids(slice_dir, assets);
    if effective.slice_local {
        reference_ids.extend(unpinned_source_inventory(assets, assets_dir, ui_platforms));
    }

    let mut asset_ids: BTreeSet<String> = reference_ids
        .into_iter()
        .filter(|id| assets.get(id).is_some_and(is_materializable_kind))
        .collect();

    append_app_icon(&mut asset_ids, ui_platforms, &doc, assets);

    MaterializeScope { asset_ids }
}

fn collect_reference_ids(
    slice_dir: &Path, assets: &serde_json::Map<String, Value>,
) -> BTreeSet<String> {
    let composition = slice_dir.join(COMPOSITION_NAME);
    if composition.is_file() {
        return collect_composition_asset_refs(&composition);
    }
    collect_artifact_asset_refs(slice_dir, assets)
}

fn collect_composition_asset_refs(path: &Path) -> BTreeSet<String> {
    let Ok(text) = fs::read_to_string(path) else {
        return BTreeSet::new();
    };
    let Ok(doc) = serde_saphyr::from_str::<Value>(&text) else {
        return BTreeSet::new();
    };
    collect_asset_references(&doc).into_iter().map(|asset_ref| asset_ref.id).collect()
}

fn collect_artifact_asset_refs(
    slice_dir: &Path, assets: &serde_json::Map<String, Value>,
) -> BTreeSet<String> {
    let mut corpus = String::new();
    append_artifact_text(slice_dir.join("design.md"), &mut corpus);
    let specs_dir = slice_dir.join("specs");
    if specs_dir.is_dir()
        && let Ok(entries) = fs::read_dir(&specs_dir)
    {
        for entry in entries.flatten() {
            let domain = entry.path();
            if domain.is_dir() {
                append_artifact_text(domain.join("spec.md"), &mut corpus);
            }
        }
    }
    if corpus.is_empty() {
        return BTreeSet::new();
    }

    assets.keys().filter(|id| text_references_asset(&corpus, id)).cloned().collect()
}

fn append_artifact_text(path: PathBuf, corpus: &mut String) {
    if let Ok(text) = fs::read_to_string(path) {
        corpus.push_str(&text);
        corpus.push('\n');
    }
}

fn text_references_asset(text: &str, asset_id: &str) -> bool {
    if text.contains(&format!("`{asset_id}`")) {
        return true;
    }
    if text.contains(&format!("assets.{asset_id}")) {
        return true;
    }
    text.match_indices(asset_id).any(|(start, _)| {
        let before_ok = start == 0 || !is_id_char(text.as_bytes()[start - 1]);
        let after_idx = start + asset_id.len();
        let after_ok = after_idx >= text.len() || !is_id_char(text.as_bytes()[after_idx]);
        before_ok && after_ok
    })
}

const fn is_id_char(byte: u8) -> bool {
    byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'
}

fn unpinned_source_inventory(
    assets: &serde_json::Map<String, Value>, assets_dir: &Path, shell_platforms: &[String],
) -> BTreeSet<String> {
    assets
        .iter()
        .filter(|(_, entry)| entry.get("source").and_then(Value::as_str).is_some())
        .filter(|(_, entry)| entry_lacks_satisfiable_pin(entry, assets_dir, shell_platforms))
        .map(|(id, _)| id.clone())
        .collect()
}

fn entry_lacks_satisfiable_pin(
    entry: &Value, assets_dir: &Path, shell_platforms: &[String],
) -> bool {
    if entry.get("source").and_then(Value::as_str).is_none() {
        return false;
    }
    shell_platforms.iter().any(|platform| !platform_pin_active(entry, platform, assets_dir))
}

fn is_materializable_kind(entry: &Value) -> bool {
    matches!(entry.get("kind").and_then(Value::as_str), Some("vector" | "raster"))
}

fn append_app_icon(
    asset_ids: &mut BTreeSet<String>, ui_platforms: &[String], doc: &Value,
    assets: &serde_json::Map<String, Value>,
) {
    if ui_platforms.is_empty() {
        return;
    }
    let Some(pointer) = doc.get("app-icon").and_then(Value::as_str) else {
        return;
    };
    let Some(entry) = assets.get(pointer) else {
        return;
    };
    if entry.get("role").and_then(Value::as_str) == Some("app-icon") {
        asset_ids.insert(pointer.to_string());
    }
}

fn asset_needs_materialize(
    entry: &Value, id: &str, assets_dir: &Path, shell_platforms: &[String],
) -> bool {
    if entry.get("role").and_then(Value::as_str) == Some("app-icon") {
        return shell_platforms.iter().any(|platform| {
            !platform_pin_active(entry, platform, assets_dir)
                && !app_icon_export_exists(assets_dir, platform)
        });
    }
    let Some(kind) = entry.get("kind").and_then(Value::as_str) else {
        return false;
    };
    shell_platforms.iter().any(|platform| {
        if platform_pin_active(entry, platform, assets_dir) {
            return false;
        }
        if conventional_export_exists(assets_dir, id, kind, platform, entry) {
            return false;
        }
        entry.get("source").and_then(Value::as_str).is_some()
    })
}
