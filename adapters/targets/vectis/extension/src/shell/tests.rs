//! Unit tests for Crux shell presence heuristics.

use tempfile::tempdir;

use super::{SUPPORTED_SHELL_PLATFORMS, shell_present};

// Greenfield (empty) tree: every supported shell is absent. The `core`-absent
// branch is unit-only — `verify` fixtures always scaffold core. The positive
// and dir-without-source-file branches are covered end-to-end by
// `tests/engine/verify.rs`: `verify_all_present_exits_clean` (all present),
// `verify_missing_shell_exits_one` (ios absent), `web_desktop_emit_info_not_error`,
// `ios_dir_without_swift_files_is_not_present`,
// `android_dir_without_kt_files_is_not_present`.
#[test]
fn greenfield_all_supported_absent() {
    let tmp = tempdir().unwrap();
    assert!(!shell_present(tmp.path(), "core"));
    assert!(!shell_present(tmp.path(), "ios"));
    assert!(!shell_present(tmp.path(), "android"));
}

#[test]
fn supported_platforms_closed_set() {
    assert_eq!(SUPPORTED_SHELL_PLATFORMS, &["core", "ios", "android"]);
}
