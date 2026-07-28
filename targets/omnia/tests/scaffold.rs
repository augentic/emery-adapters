//! Base-repo scaffold behavior: strict checkout validation plus fill-only
//! writes from a synthetic exemplar contract — no network, no sibling
//! checkout.

mod common;

use std::fs;
use std::path::Path;

use omnia::scaffold::{CHECKOUT_DIR, Error, ensure_missing};
use tempfile::TempDir;

/// A project tree carrying the valid synthetic checkout.
fn project() -> TempDir {
    let tmp = TempDir::new().unwrap();
    common::write_checkout(tmp.path());
    tmp
}

fn checkout_error(root: &Path) -> String {
    match ensure_missing(root) {
        Err(Error::Checkout(detail)) => detail,
        other => panic!("expected a checkout rejection, got {other:?}"),
    }
}

#[test]
fn empty_tree() {
    let tmp = project();

    let report = ensure_missing(tmp.path()).unwrap();

    assert_eq!(report.written, common::TARGETS, "every target written in manifest order");
    assert!(report.skipped.is_empty());
    for path in common::TARGETS {
        assert!(tmp.path().join(path).is_file(), "{path} exists");
    }

    // Bodies copied verbatim from the checkout; seed tokens stay unfilled.
    let makefile = fs::read_to_string(tmp.path().join("Makefile.toml")).unwrap();
    assert!(makefile.contains("[tasks.vet]"), "exact body copied from the checkout root");
    let publish = fs::read_to_string(tmp.path().join(".github/workflows/publish.yaml")).unwrap();
    assert_eq!(publish, common::PUBLISH_BODY, "seed body copied verbatim, tokens intact");
    assert!(
        fs::read_dir(tmp.path()).unwrap().all(|entry| {
            !entry.unwrap().file_name().to_string_lossy().ends_with(".scaffold-tmp")
        }),
        "no atomic-write temp files left behind"
    );
}

#[test]
fn idempotent() {
    let tmp = project();
    ensure_missing(tmp.path()).unwrap();
    let before = fs::read_to_string(tmp.path().join("deny.toml")).unwrap();

    let report = ensure_missing(tmp.path()).unwrap();

    assert!(report.written.is_empty(), "second pass writes nothing");
    assert_eq!(report.skipped, common::TARGETS);
    assert_eq!(fs::read_to_string(tmp.path().join("deny.toml")).unwrap(), before);
}

#[test]
fn fills_gaps_only() {
    let tmp = project();
    fs::write(tmp.path().join("deny.toml"), "# consumer-customized\n").unwrap();

    let report = ensure_missing(tmp.path()).unwrap();

    assert_eq!(report.skipped, ["deny.toml"], "existing file left untouched");
    assert_eq!(report.written.len(), common::TARGETS.len() - 1);
    assert_eq!(
        fs::read_to_string(tmp.path().join("deny.toml")).unwrap(),
        "# consumer-customized\n",
        "never overwritten"
    );
    assert!(tmp.path().join("Makefile.toml").is_file(), "missing siblings still written");
}

#[test]
fn tokens_from_manifest() {
    // The build prompts name these tokens in prose; the runtime list
    // derives from the checkout manifest's `tokens` map, not from Rust.
    let tmp = project();

    let report = ensure_missing(tmp.path()).unwrap();

    assert_eq!(report.tokens, ["<PACKAGE_NAME>", "<STORAGE_ACCOUNT>"]);
}

mod rejects {
    use super::*;

    #[test]
    fn missing_checkout() {
        let tmp = TempDir::new().unwrap();
        let detail = checkout_error(tmp.path());
        assert!(detail.contains("preparation leg"), "points at the missing preparation: {detail}");
    }

    #[test]
    fn wrong_exemplar_schema_version() {
        let tmp = project();
        let exemplar = tmp.path().join(CHECKOUT_DIR).join("exemplar.yaml");
        let body = common::EXEMPLAR_YAML.replace("schema-version: 1", "schema-version: 9");
        fs::write(exemplar, body).unwrap();

        let detail = checkout_error(tmp.path());
        assert!(detail.contains("schema-version 9"), "{detail}");
    }

    #[test]
    fn wrong_manifest_schema_version() {
        let tmp = project();
        let manifest = tmp.path().join(CHECKOUT_DIR).join("templates/guest/manifest.yaml");
        let body = common::MANIFEST_YAML.replace("schema-version: 3", "schema-version: 2");
        fs::write(manifest, body).unwrap();

        let detail = checkout_error(tmp.path());
        assert!(detail.contains("schema-version 2"), "{detail}");
    }

    #[test]
    fn unsafe_source_path() {
        let tmp = project();
        let manifest = tmp.path().join(CHECKOUT_DIR).join("templates/guest/manifest.yaml");
        let body = common::MANIFEST_YAML
            .replace("source: templates/guest/core/deny.toml", "source: ../outside/deny.toml");
        fs::write(manifest, body).unwrap();

        let detail = checkout_error(tmp.path());
        assert!(detail.contains("unsafe source path"), "{detail}");
    }

    #[test]
    fn exact_source_target_mismatch() {
        let tmp = project();
        let manifest = tmp.path().join(CHECKOUT_DIR).join("templates/guest/manifest.yaml");
        let body =
            common::MANIFEST_YAML.replace("- source: Makefile.toml", "- source: Makefile.core");
        fs::write(manifest, body).unwrap();

        let detail = checkout_error(tmp.path());
        assert!(detail.contains("source == target"), "{detail}");
    }

    #[test]
    fn missing_source_file() {
        let tmp = project();
        fs::remove_file(tmp.path().join(CHECKOUT_DIR).join("templates/guest/core/deny.toml"))
            .unwrap();

        let detail = checkout_error(tmp.path());
        assert!(detail.contains("unreadable"), "{detail}");
    }
}
