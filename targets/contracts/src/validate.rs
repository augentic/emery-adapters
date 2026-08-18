//! Baseline-contract validation: SemVer, id format, and id uniqueness
//! for top-level `OpenAPI` / `AsyncAPI` documents under `contracts/`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

mod parse;

/// One validation finding from [`validate_baseline`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractFinding {
    /// Contract file path.
    pub path: PathBuf,
    /// Stable rule id (`contract.<rule>`).
    pub rule_id: &'static str,
    /// Human-readable failure detail.
    pub detail: String,
}

/// Rule id: `info.version` must parse as SemVer.
pub const RULE_VERSION_IS_SEMVER: &str = "contract.version-is-semver";
/// Rule id: `info.x-emery-id` must be kebab-case and ≤ 64 characters.
pub const RULE_ID_FORMAT: &str = "contract.id-format";
/// Rule id: every `info.x-emery-id` must be unique across the tree.
pub const RULE_ID_UNIQUE: &str = "contract.id-unique";
/// Rule id: every directory and entry under `contracts/` must be
/// traversable — an unreadable subtree could hide contracts (A4).
pub const RULE_TREE_READABLE: &str = "contract.tree-readable";
/// Rule id: every `.yaml` file under `contracts/` must read and parse —
/// a malformed contract must not vanish from the document set (A4).
pub const RULE_YAML_WELL_FORMED: &str = "contract.yaml-well-formed";

/// Run baseline-contract validation across `contracts_dir`.
///
/// Returns an empty vector when the directory does not exist (a project
/// without contracts) or every file is well-formed. Fails closed on
/// everything else (A4): an unreadable directory, entry, or file and
/// unparseable YAML are blocking findings, never silent skips.
#[must_use]
pub fn validate_baseline(contracts_dir: &Path) -> Vec<ContractFinding> {
    match std::fs::metadata(contracts_dir) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(err) => {
            return vec![ContractFinding {
                path: contracts_dir.to_path_buf(),
                rule_id: RULE_TREE_READABLE,
                detail: format!("contracts directory is unreadable: {err}"),
            }];
        }
        Ok(_) => {}
    }

    let (docs, mut findings) = parse::collect_top_level_docs(contracts_dir);

    let mut id_to_paths: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();

    for doc in &docs {
        let info = doc.value.get("info");

        match parse::version_str(info) {
            Some(v) if semver::Version::parse(v).is_ok() => {}
            Some(v) => findings.push(ContractFinding {
                path: doc.path.clone(),
                rule_id: RULE_VERSION_IS_SEMVER,
                detail: format!(
                    "info.version `{v}` is not valid SemVer (must parse per semver.org, \
                     including optional prerelease labels)"
                ),
            }),
            None => findings.push(ContractFinding {
                path: doc.path.clone(),
                rule_id: RULE_VERSION_IS_SEMVER,
                detail: "info.version is missing or not a string; \
                         every top-level OpenAPI / AsyncAPI document must \
                         set a SemVer info.version"
                    .to_string(),
            }),
        }

        if let Some(id) = parse::id_str(info) {
            if parse::is_valid_emery_id(id) {
                id_to_paths.entry(id.to_string()).or_default().push(doc.path.clone());
            } else {
                findings.push(ContractFinding {
                    path: doc.path.clone(),
                    rule_id: RULE_ID_FORMAT,
                    detail: format!(
                        "info.x-emery-id `{id}` must match `^[a-z][a-z0-9-]*$` \
                         and be ≤ 64 characters"
                    ),
                });
            }
        }
    }

    for (id, paths) in &id_to_paths {
        if paths.len() < 2 {
            continue;
        }
        let listed: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
        for path in paths {
            findings.push(ContractFinding {
                path: path.clone(),
                rule_id: RULE_ID_UNIQUE,
                detail: format!(
                    "info.x-emery-id `{id}` is declared by multiple top-level contracts: {}",
                    listed.join(", ")
                ),
            });
        }
    }

    findings.sort_by(|a, b| {
        a.path
            .as_os_str()
            .cmp(b.path.as_os_str())
            .then_with(|| a.rule_id.cmp(b.rule_id))
            .then_with(|| a.detail.cmp(&b.detail))
    });

    findings
}
