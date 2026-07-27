//! YAML walker for top-level contract documents (`openapi:` / `asyncapi:` at root).

use std::path::{Path, PathBuf};

use serde_json::Value;

pub struct TopLevelDoc {
    pub(super) path: PathBuf,
    pub(super) value: Value,
}

// Parse errors are swallowed; the format verifier owns malformed YAML.
pub fn collect_top_level_docs(contracts_dir: &Path) -> Vec<TopLevelDoc> {
    let mut paths = Vec::new();
    collect_yaml_paths(contracts_dir, &mut paths);
    paths.sort();
    let mut out: Vec<TopLevelDoc> = Vec::new();
    for entry in paths {
        let Ok(content) = std::fs::read_to_string(&entry) else {
            continue;
        };
        let Ok(value) = serde_saphyr::from_str::<Value>(&content) else {
            continue;
        };
        if !is_top_level(&value) {
            continue;
        }
        out.push(TopLevelDoc { path: entry, value });
    }

    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

fn collect_yaml_paths(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            collect_yaml_paths(&path, out);
        } else if file_type.is_file() && path.extension().is_some_and(|ext| ext == "yaml") {
            out.push(path);
        }
    }
}

fn is_top_level(value: &Value) -> bool {
    let Some(obj) = value.as_object() else {
        return false;
    };
    obj.contains_key("openapi") || obj.contains_key("asyncapi")
}

pub fn version_str(info: Option<&Value>) -> Option<&str> {
    info?.get("version")?.as_str()
}

pub fn id_str(info: Option<&Value>) -> Option<&str> {
    info?.get("x-emery-id")?.as_str()
}

// Inlined from `RegistryProject::name` so the 64-character cap stays self-contained.
pub fn is_valid_emery_id(id: &str) -> bool {
    if id.is_empty() || id.len() > 64 {
        return false;
    }
    let bytes = id.as_bytes();
    if !bytes[0].is_ascii_lowercase() {
        return false;
    }
    let mut prev_dash = false;
    for &b in bytes {
        let lower = b.is_ascii_lowercase();
        let digit = b.is_ascii_digit();
        let dash = b == b'-';
        if !(lower || digit || dash) {
            return false;
        }
        if dash && prev_dash {
            return false;
        }
        prev_dash = dash;
    }
    if prev_dash {
        return false;
    }
    true
}
