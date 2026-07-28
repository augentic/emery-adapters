//! Synthetic exemplar checkout for scaffold and operations tests.
//!
//! Deliberately not a copy of the upstream contract: small bodies, the
//! same shape. Tests run without any network or sibling checkout.

#![allow(dead_code, reason = "shared across test binaries; each uses a subset")]

use std::fs;
use std::path::Path;

/// Synthetic `exemplar.yaml` body.
pub const EXEMPLAR_YAML: &str = "\
schema-version: 1

omnia:
  version: 0.0.1
  repository: https://example.invalid/omnia
  rev: 0123456789abcdef0123456789abcdef01234567

templates:
  manifest: templates/guest/manifest.yaml
";

/// Synthetic manifest: exact entries reference checkout-root files in
/// place; seed bodies live under `templates/guest/core/`. The order is
/// deliberately not alphabetical so ordering assertions mean something.
pub const MANIFEST_YAML: &str = "\
schema-version: 3

tokens:
  PACKAGE_NAME: Deployable guest package name.
  STORAGE_ACCOUNT: Deploy target storage account.

assemblies:
  core:
    path-mode: content-only
    files:
      - source: Makefile.toml
        target: Makefile.toml
        proof: exact
      - source: templates/guest/core/deny.toml
        target: deny.toml
        proof: seed
      - source: templates/guest/core/supply-chain-config.toml
        target: supply-chain/config.toml
        proof: seed
      - source: .github/workflows/ci.yaml
        target: .github/workflows/ci.yaml
        proof: exact
      - source: templates/guest/core/workflow-publish.yaml
        target: .github/workflows/publish.yaml
        proof: seed
";

/// Target paths the synthetic manifest declares, in manifest order.
pub const TARGETS: &[&str] = &[
    "Makefile.toml",
    "deny.toml",
    "supply-chain/config.toml",
    ".github/workflows/ci.yaml",
    ".github/workflows/publish.yaml",
];

/// Body of the synthetic publish seed; carries both declared tokens.
pub const PUBLISH_BODY: &str =
    "name: Publish\nenv:\n  package: <PACKAGE_NAME>\n  account: <STORAGE_ACCOUNT>\n";

/// Write a valid synthetic exemplar checkout under
/// `<project_root>/target/omnia-exemplar/`.
pub fn write_checkout(project_root: &Path) {
    let checkout = project_root.join("target/omnia-exemplar");
    let files: &[(&str, &str)] = &[
        ("exemplar.yaml", EXEMPLAR_YAML),
        ("templates/guest/manifest.yaml", MANIFEST_YAML),
        ("Makefile.toml", "[tasks.vet]\ncommand = \"cargo\"\n"),
        (".github/workflows/ci.yaml", "name: CI\non: [push]\n"),
        ("templates/guest/core/deny.toml", "[licenses]\nallow = [\"MIT\"]\n"),
        (
            "templates/guest/core/supply-chain-config.toml",
            "[imports.bytecode-alliance]\nurl = \"https://example.invalid/audits.toml\"\n",
        ),
        ("templates/guest/core/workflow-publish.yaml", PUBLISH_BODY),
    ];
    for (path, body) in files {
        let path = checkout.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    }
}
