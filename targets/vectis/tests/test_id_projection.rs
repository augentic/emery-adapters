//! Projection of composition inline `test_id` values into `ui-contract/test-ids.yaml`.

use std::fs;
use std::path::Path;

use tempfile::tempdir;
use vectis::projections::test_id_registry::{self, REGISTRY_REL};

fn write(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(path, content).expect("write fixture");
}

#[test]
fn harvests_baseline_only_when_no_active_slice() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();

    write(
        &root.join(".emery/specs/composition.yaml"),
        "version: 1\nscreens:\n  splash:\n    name: Splash\n    body:\n      - button:\n          test_id: splash-cta\n",
    );
    write(
        &root.join(".emery/change/slices/follow-up/composition.yaml"),
        "version: 1\ndelta:\n  added:\n    stub:\n      name: Stub\n      body:\n        - text:\n            test_id: stub-message\n  modified: {}\n  removed: {}\n",
    );

    let entries = test_id_registry::harvest_entries(root, None).expect("harvest");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries.get("MAESTRO_SPLASH_CTA"), Some(&"splash-cta".to_string()));
    assert!(!entries.contains_key("MAESTRO_STUB_MESSAGE"));
}

#[test]
fn harvests_test_ids_from_merged_baseline_and_slice() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();

    write(
        &root.join(".emery/specs/composition.yaml"),
        "version: 1\nscreens:\n  splash:\n    name: Splash\n    body:\n      - button:\n          test_id: splash-cta\n",
    );
    write(
        &root.join(".emery/change/slices/follow-up/composition.yaml"),
        "version: 1\ndelta:\n  added:\n    stub:\n      name: Stub\n      body:\n        - text:\n            test_id: stub-message\n  modified: {}\n  removed: {}\n",
    );

    let slice = root.join(".emery/change/slices/follow-up/composition.yaml");
    let entries = test_id_registry::harvest_entries(root, Some(&slice)).expect("harvest");
    assert_eq!(entries.len(), 2);
    assert_eq!(entries.get("MAESTRO_SPLASH_CTA"), Some(&"splash-cta".to_string()));
    assert_eq!(entries.get("MAESTRO_STUB_MESSAGE"), Some(&"stub-message".to_string()));
}

#[test]
fn rejects_duplicate_test_id_across_merged_baseline_and_slice() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();

    write(
        &root.join(".emery/specs/composition.yaml"),
        "version: 1\nscreens:\n  splash:\n    name: Splash\n    body:\n      - button:\n          test_id: splash-cta\n",
    );
    write(
        &root.join(".emery/change/slices/follow-up/composition.yaml"),
        "version: 1\ndelta:\n  added:\n    stub:\n      name: Stub\n      body:\n        - text:\n            test_id: splash-cta\n  modified: {}\n  removed: {}\n",
    );

    let slice = root.join(".emery/change/slices/follow-up/composition.yaml");
    let err = test_id_registry::harvest_entries(root, Some(&slice)).unwrap_err();
    let message = format!("{err}");
    assert!(
        message.contains("duplicate `test_id` `splash-cta`"),
        "expected duplicate test_id error, got: {message}"
    );
}

#[test]
fn modified_screen_replaces_old_test_id() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();

    write(
        &root.join(".emery/specs/composition.yaml"),
        "version: 1\nscreens:\n  list:\n    name: List\n    body:\n      - button:\n          test_id: list-row\n",
    );
    write(
        &root.join(".emery/change/slices/rename/composition.yaml"),
        "version: 1\ndelta:\n  added: {}\n  modified:\n    list:\n      name: List\n      body:\n        - button:\n            test_id: list-row-updated\n  removed: {}\n",
    );

    let slice = root.join(".emery/change/slices/rename/composition.yaml");
    let entries = test_id_registry::harvest_entries(root, Some(&slice)).expect("harvest");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries.get("MAESTRO_LIST_ROW_UPDATED"), Some(&"list-row-updated".to_string()));
    assert!(!entries.contains_key("MAESTRO_LIST_ROW"));
}

#[test]
fn removed_screen_drops_test_ids_from_harvest() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();

    write(
        &root.join(".emery/specs/composition.yaml"),
        "version: 1\nscreens:\n  keep:\n    name: Keep\n    body:\n      - button:\n          test_id: keep-cta\n  drop:\n    name: Drop\n    body:\n      - button:\n          test_id: drop-cta\n",
    );
    write(
        &root.join(".emery/change/slices/prune/composition.yaml"),
        "version: 1\ndelta:\n  added: {}\n  modified: {}\n  removed:\n    drop:\n      reason: obsolete\n",
    );

    let slice = root.join(".emery/change/slices/prune/composition.yaml");
    let entries = test_id_registry::harvest_entries(root, Some(&slice)).expect("harvest");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries.get("MAESTRO_KEEP_CTA"), Some(&"keep-cta".to_string()));
    assert!(!entries.contains_key("MAESTRO_DROP_CTA"));
}

#[test]
fn write_generated_is_idempotent() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();

    write(
        &root.join(".emery/specs/composition.yaml"),
        "screens:\n  splash:\n    name: Splash\n    body:\n      - button:\n          test_id: splash-cta\n",
    );

    test_id_registry::write_generated(root, root, None).expect("first write");
    let path = root.join(REGISTRY_REL);
    let first = fs::read_to_string(&path).unwrap();

    test_id_registry::write_generated(root, root, None).expect("second write");
    let second = fs::read_to_string(&path).unwrap();
    assert_eq!(first, second);
    assert!(second.contains("MAESTRO_SPLASH_CTA: splash-cta"));
}
