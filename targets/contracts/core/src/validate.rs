//! Baseline-contract validation primitives: walk a `contracts/` tree and
//! enforce `version-is-semver`, `id-format`, and `id-unique` against each
//! top-level `OpenAPI` / `AsyncAPI` document.
//!
//! Owned by the wasm-free core so both consumers share one implementation:
//! the `specify-contract` extension binary (the `specify extension run
//! contract` surface, unchanged until RFC-61 Step 5) and the contracts
//! guest's validate-before-visible gate after each judgment answer lands.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

mod parse;

/// One validation finding produced by [`validate_baseline`].
///
/// `rule_id` is one of `contract.version-is-semver`,
/// `contract.id-format`, or `contract.id-unique`. `path` is the
/// absolute path to the offending YAML file, suitable to render
/// verbatim in the operator's terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractFinding {
    /// Absolute path to the contract file the finding refers to.
    pub path: PathBuf,
    /// Stable rule identifier (`contract.<rule>`).
    pub rule_id: &'static str,
    /// Human-readable failure detail (file-name-aware).
    pub detail: String,
}

/// Rule id: `info.version` must parse as SemVer.
pub const RULE_VERSION_IS_SEMVER: &str = "contract.version-is-semver";
/// Rule id: `info.x-specify-id` must be kebab-case and ≤ 64 characters.
pub const RULE_ID_FORMAT: &str = "contract.id-format";
/// Rule id: every `info.x-specify-id` must be unique across the tree.
pub const RULE_ID_UNIQUE: &str = "contract.id-unique";

/// Run the baseline-contract validation checks across `contracts_dir`.
///
/// Returns an empty vector when the directory does not exist, when it
/// is empty, or when every walked file is well-formed. The order of
/// findings is deterministic: rules within a file appear in the order
/// listed in the module docs, and files appear in lexicographic path
/// order.
#[must_use]
pub fn validate_baseline(contracts_dir: &Path) -> Vec<ContractFinding> {
    if std::fs::read_dir(contracts_dir).is_err() {
        return Vec::new();
    }

    let docs = parse::collect_top_level_docs(contracts_dir);

    let mut findings: Vec<ContractFinding> = Vec::new();
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
            if parse::is_valid_specify_id(id) {
                id_to_paths.entry(id.to_string()).or_default().push(doc.path.clone());
            } else {
                findings.push(ContractFinding {
                    path: doc.path.clone(),
                    rule_id: RULE_ID_FORMAT,
                    detail: format!(
                        "info.x-specify-id `{id}` must match `^[a-z][a-z0-9-]*$` \
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
                    "info.x-specify-id `{id}` is declared by multiple top-level contracts: {}",
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
