//! Baseline-contract validation primitives. Owned by the contract
//! adapter's WASI carve-out: walks `contracts/` and enforces
//! `version-is-semver`, `id-format`, and `id-unique` against each
//! top-level `OpenAPI` / `AsyncAPI` document.
//!
//! Housing these primitives in the carve-out preserves the
//! adapter-extension invariant: an adapter's logic is reachable from the
//! host only through `specify extension run <name>`.

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

const RULE_VERSION_IS_SEMVER: &str = "contract.version-is-semver";
const RULE_ID_FORMAT: &str = "contract.id-format";
const RULE_ID_UNIQUE: &str = "contract.id-unique";

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

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    fn write_contract(tmp: &TempDir, rel: &str, body: &str) -> PathBuf {
        let path = tmp.path().join("contracts").join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, body).unwrap();
        path
    }

    fn contracts_dir(tmp: &TempDir) -> PathBuf {
        tmp.path().join("contracts")
    }

    fn finding_kinds(findings: &[ContractFinding]) -> Vec<&'static str> {
        findings.iter().map(|f| f.rule_id).collect()
    }

    /// One row of the `validate_baseline` projection matrix.
    struct Case {
        /// Failure label, surfaced on assertion so a red row is obvious.
        name: &'static str,
        /// Create the `contracts/` dir at all — `false` exercises the
        /// absent-directory early return.
        create_dir: bool,
        /// `(relative path, YAML body)` pairs written under `contracts/`.
        files: Vec<(&'static str, String)>,
        /// Expected `rule_id`s in `validate_baseline`'s sorted order.
        expect: Vec<&'static str>,
        /// Substrings each required somewhere in the findings' details.
        detail_contains: Vec<&'static str>,
    }

    /// `OpenAPI` doc carrying `version` and no `x-specify-id`.
    fn body_version(version: &str) -> String {
        format!("openapi: '3.1.0'\ninfo:\n  title: User API\n  version: {version}\n")
    }

    /// `OpenAPI` doc carrying a valid SemVer `version` and the given id.
    fn body_id(id: &str) -> String {
        format!(
            "openapi: '3.1.0'\ninfo:\n  title: User API\n  version: 1.0.0\n  x-specify-id: {id}\n"
        )
    }

    fn check(case: &Case) {
        let tmp = TempDir::new().unwrap();
        if case.create_dir {
            fs::create_dir_all(contracts_dir(&tmp)).unwrap();
        }
        for (rel, body) in &case.files {
            write_contract(&tmp, rel, body);
        }
        let findings = validate_baseline(&contracts_dir(&tmp));
        assert_eq!(finding_kinds(&findings), case.expect, "case: {}", case.name);
        for needle in &case.detail_contains {
            assert!(
                findings.iter().any(|f| f.detail.contains(*needle)),
                "case `{}` detail contains `{needle}`",
                case.name
            );
        }
    }

    // `validate_baseline` is a pure `(contracts tree → sorted findings)`
    // projection, so each rule's edge set collapses to a table — a new
    // case is a row, not a `fn`. The wire contract (exit codes, JSON
    // shape, golden bytes) is owned black-box by `tests/cli.rs` and is
    // deliberately not restated here.

    #[test]
    fn version_rule_matrix() {
        let cases = vec![
            Case {
                name: "semver passes",
                create_dir: true,
                files: vec![("http/user-api.yaml", body_version("1.0.0"))],
                expect: vec![],
                detail_contains: vec![],
            },
            Case {
                name: "semver prerelease passes",
                create_dir: true,
                files: vec![("http/user-api.yaml", body_version("1.0.0-draft.1"))],
                expect: vec![],
                detail_contains: vec![],
            },
            Case {
                name: "date-string version fails",
                create_dir: true,
                files: vec![("http/user-api.yaml", body_version("2024-01-15"))],
                expect: vec![RULE_VERSION_IS_SEMVER],
                detail_contains: vec!["2024-01-15"],
            },
            Case {
                name: "major-only version fails",
                create_dir: true,
                files: vec![("http/user-api.yaml", body_version("'1'"))],
                expect: vec![RULE_VERSION_IS_SEMVER],
                detail_contains: vec![],
            },
            Case {
                name: "missing version fails",
                create_dir: true,
                files: vec![(
                    "http/user-api.yaml",
                    "openapi: '3.1.0'\ninfo:\n  title: User API\n".to_string(),
                )],
                expect: vec![RULE_VERSION_IS_SEMVER],
                detail_contains: vec!["missing"],
            },
            Case {
                name: "asyncapi top-level is validated",
                create_dir: true,
                files: vec![(
                    "messages/orders.yaml",
                    "asyncapi: '3.0.0'\ninfo:\n  title: Orders\n  version: 2024-01-15\n"
                        .to_string(),
                )],
                expect: vec![RULE_VERSION_IS_SEMVER],
                detail_contains: vec![],
            },
        ];
        for case in &cases {
            check(case);
        }
    }

    #[test]
    fn id_format_matrix() {
        let too_long = "a".repeat(65);
        let cases = vec![
            Case {
                name: "id uppercase fails",
                create_dir: true,
                files: vec![("http/user-api.yaml", body_id("User-API"))],
                expect: vec![RULE_ID_FORMAT],
                detail_contains: vec![],
            },
            Case {
                name: "id leading-hyphen fails",
                create_dir: true,
                files: vec![("http/user-api.yaml", body_id("-leading"))],
                expect: vec![RULE_ID_FORMAT],
                detail_contains: vec![],
            },
            Case {
                name: "id too-long fails",
                create_dir: true,
                files: vec![("http/user-api.yaml", body_id(&too_long))],
                expect: vec![RULE_ID_FORMAT],
                detail_contains: vec![],
            },
            Case {
                name: "id kebab-case passes",
                create_dir: true,
                files: vec![("http/user-api.yaml", body_id("user-api"))],
                expect: vec![],
                detail_contains: vec![],
            },
        ];
        for case in &cases {
            check(case);
        }
    }

    #[test]
    fn skip_and_directory_matrix() {
        let cases = vec![
            Case {
                name: "absent dir returns no findings",
                create_dir: false,
                files: vec![],
                expect: vec![],
                detail_contains: vec![],
            },
            Case {
                name: "empty dir returns no findings",
                create_dir: true,
                files: vec![],
                expect: vec![],
                detail_contains: vec![],
            },
            Case {
                name: "json-schema file is skipped",
                create_dir: true,
                files: vec![(
                    "schemas/user.yaml",
                    "$id: urn:specify:schemas/user\ntitle: User\ndescription: A user.\ntype: object\n".to_string(),
                )],
                expect: vec![],
                detail_contains: vec![],
            },
            Case {
                name: "unparseable yaml is skipped",
                create_dir: true,
                files: vec![("http/broken.yaml", ":this is not yaml: [\n".to_string())],
                expect: vec![],
                detail_contains: vec![],
            },
            Case {
                name: "two docs without ids are not duplicates",
                create_dir: true,
                files: vec![
                    ("http/user-api.yaml", body_version("1.0.0")),
                    (
                        "http/billing-api.yaml",
                        "openapi: '3.1.0'\ninfo:\n  title: Billing API\n  version: 1.0.0\n".to_string(),
                    ),
                ],
                expect: vec![],
                detail_contains: vec![],
            },
        ];
        for case in &cases {
            check(case);
        }
    }

    #[test]
    fn id_duplicates_across_two_files_fail_both() {
        let tmp = TempDir::new().unwrap();
        write_contract(
            &tmp,
            "http/user-api.yaml",
            "openapi: '3.1.0'\ninfo:\n  title: User API\n  version: 1.0.0\n  x-specify-id: shared\n",
        );
        write_contract(
            &tmp,
            "http/billing-api.yaml",
            "openapi: '3.1.0'\ninfo:\n  title: Billing API\n  version: 1.0.0\n  x-specify-id: shared\n",
        );
        let findings = validate_baseline(&contracts_dir(&tmp));
        assert_eq!(findings.len(), 2);
        assert!(findings.iter().all(|f| f.rule_id == RULE_ID_UNIQUE));
        assert!(
            findings.iter().any(|f| f.path.ends_with("http/user-api.yaml")),
            "user-api.yaml flagged"
        );
        assert!(
            findings.iter().any(|f| f.path.ends_with("http/billing-api.yaml")),
            "billing-api.yaml flagged"
        );
    }
}
