//! YAML walker for top-level contract documents (`openapi:` / `asyncapi:` at root).

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::{ContractFinding, RULE_TREE_READABLE, RULE_YAML_WELL_FORMED};

pub struct TopLevelDoc {
    pub(super) path: PathBuf,
    pub(super) value: Value,
}

/// Walk `contracts_dir` into the top-level contract set, failing closed
/// (A4): traversal failures and unreadable or unparseable YAML become
/// blocking findings instead of silently shrinking the document set. A
/// parsed document without a root `openapi:` / `asyncapi:` marker is a
/// fragment (shared components, JSON Schema), not a contract — skipped
/// by identification, not by error.
pub fn collect_top_level_docs(contracts_dir: &Path) -> (Vec<TopLevelDoc>, Vec<ContractFinding>) {
    let mut paths = Vec::new();
    let mut findings = Vec::new();
    collect_yaml_paths(contracts_dir, &mut paths, &mut findings);
    paths.sort();
    let mut out: Vec<TopLevelDoc> = Vec::new();
    for entry in paths {
        let content = match std::fs::read_to_string(&entry) {
            Ok(content) => content,
            Err(err) => {
                findings.push(ContractFinding {
                    path: entry,
                    rule_id: RULE_YAML_WELL_FORMED,
                    detail: format!("contract file is unreadable: {err}"),
                });
                continue;
            }
        };
        let value = match serde_saphyr::from_str::<Value>(&content) {
            Ok(value) => value,
            Err(err) => {
                findings.push(ContractFinding {
                    path: entry,
                    rule_id: RULE_YAML_WELL_FORMED,
                    detail: format!("contract file is not valid YAML: {err}"),
                });
                continue;
            }
        };
        if !is_top_level(&value) {
            continue;
        }
        out.push(TopLevelDoc { path: entry, value });
    }

    out.sort_by(|a, b| a.path.cmp(&b.path));
    (out, findings)
}

fn collect_yaml_paths(dir: &Path, out: &mut Vec<PathBuf>, findings: &mut Vec<ContractFinding>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) => {
            findings.push(ContractFinding {
                path: dir.to_path_buf(),
                rule_id: RULE_TREE_READABLE,
                detail: format!("contracts directory is unreadable: {err}"),
            });
            return;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                findings.push(ContractFinding {
                    path: dir.to_path_buf(),
                    rule_id: RULE_TREE_READABLE,
                    detail: format!("directory entry is unreadable: {err}"),
                });
                continue;
            }
        };
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(err) => {
                findings.push(ContractFinding {
                    path,
                    rule_id: RULE_TREE_READABLE,
                    detail: format!("entry metadata is unreadable: {err}"),
                });
                continue;
            }
        };
        if file_type.is_dir() {
            collect_yaml_paths(&path, out, findings);
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
