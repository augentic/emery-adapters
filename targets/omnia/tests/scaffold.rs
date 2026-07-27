//! Base-repo scaffold behavior: fill-only writes from the embedded templates.

use std::fs;

use omnia::scaffold::ensure_missing;
use tempfile::TempDir;

/// Every target path the core assembly declares, in manifest order.
const EXPECTED: &[&str] = &[
    "Makefile",
    "Makefile.toml",
    "deny.toml",
    "rust-toolchain.toml",
    "rustfmt.toml",
    "clippy.toml",
    "taplo.toml",
    ".gitignore",
    ".cargo/config.toml",
    ".vscode/settings.json",
    "supply-chain/README.md",
    "supply-chain/config.toml",
    "supply-chain/audits.toml",
    ".github/workflows/audit.yaml",
    ".github/workflows/ci.yaml",
    ".github/workflows/patch.yaml",
    ".github/workflows/release.yaml",
    ".github/workflows/publish.yaml",
];

#[test]
fn empty_tree() {
    let tmp = TempDir::new().unwrap();

    let report = ensure_missing(tmp.path()).unwrap();

    assert_eq!(report.written, EXPECTED, "every template written in manifest order");
    assert!(report.skipped.is_empty());
    for path in EXPECTED {
        assert!(tmp.path().join(path).is_file(), "{path} exists");
    }

    // Spot-check rendered contents against the embedded template bodies.
    let makefile = fs::read_to_string(tmp.path().join("Makefile.toml")).unwrap();
    assert!(makefile.contains("[tasks.vet]"), "cargo-vet task present");
    assert!(makefile.contains(r#""-D", "warnings""#), "clippy denies warnings");
    let vet_config = fs::read_to_string(tmp.path().join("supply-chain/config.toml")).unwrap();
    assert!(vet_config.contains("[imports.bytecode-alliance]"), "standard vet imports");
    assert!(
        !tmp.path().join("supply-chain/imports.lock").exists(),
        "imports.lock is cargo-vet output, never scaffolded"
    );
    let toolchain = fs::read_to_string(tmp.path().join("rust-toolchain.toml")).unwrap();
    assert!(toolchain.contains("wasm32-wasip2"), "wasm component target pinned");
}

#[test]
fn idempotent() {
    let tmp = TempDir::new().unwrap();
    ensure_missing(tmp.path()).unwrap();
    let before = fs::read_to_string(tmp.path().join("deny.toml")).unwrap();

    let report = ensure_missing(tmp.path()).unwrap();

    assert!(report.written.is_empty(), "second pass writes nothing");
    assert_eq!(report.skipped, EXPECTED);
    assert_eq!(fs::read_to_string(tmp.path().join("deny.toml")).unwrap(), before);
}

#[test]
fn fills_gaps_only() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("deny.toml"), "# consumer-customized\n").unwrap();

    let report = ensure_missing(tmp.path()).unwrap();

    assert_eq!(report.skipped, ["deny.toml"], "existing file left untouched");
    assert_eq!(report.written.len(), EXPECTED.len() - 1);
    assert_eq!(
        fs::read_to_string(tmp.path().join("deny.toml")).unwrap(),
        "# consumer-customized\n",
        "never overwritten"
    );
    assert!(tmp.path().join("Makefile.toml").is_file(), "missing siblings still written");
}
