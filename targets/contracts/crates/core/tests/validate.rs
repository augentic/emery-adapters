//! Matrix coverage for the absorbed baseline-contract validators.

use std::fs;
use std::path::PathBuf;

use specify_contracts_core::validate::{
    ContractFinding, RULE_ID_FORMAT, RULE_ID_UNIQUE, RULE_VERSION_IS_SEMVER, validate_baseline,
};
use tempfile::TempDir;

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
    format!("openapi: '3.1.0'\ninfo:\n  title: User API\n  version: 1.0.0\n  x-specify-id: {id}\n")
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
// shape, golden bytes) is owned black-box by the extension's
// `tests/cli.rs` and is deliberately not restated here.

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
                "asyncapi: '3.0.0'\ninfo:\n  title: Orders\n  version: 2024-01-15\n".to_string(),
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
                "$id: urn:specify:schemas/user\ntitle: User\ndescription: A user.\ntype: object\n"
                    .to_string(),
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
