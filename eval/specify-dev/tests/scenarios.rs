//! Model-free scenario runner gates: config, artifacts, run dirs, outcomes.

use std::fs;
use std::path::{Path, PathBuf};

use eval::scenario;
use project::seam::wire::{BUILD_VERSION, BuildReport, BuildStatus};
use tempfile::TempDir;

fn report(status: BuildStatus) -> BuildReport {
    BuildReport {
        version: BUILD_VERSION,
        slice: "demo".to_string(),
        target: "contracts".to_string(),
        status,
        findings: Vec::new(),
        outputs: Vec::new(),
        ui_surface: None,
    }
}

#[test]
fn wiring() {
    let root = scenario::scenarios_dir();
    let mut seen = 0;
    for dir in scenario_dirs(&root) {
        let id = dir.strip_prefix(&root).expect("scenario under the scenarios root");
        let config =
            scenario::load(&dir).unwrap_or_else(|err| panic!("{}: {err:#}", id.display()));

        let inputs: Vec<PathBuf> = fs::read_dir(dir.join("inputs"))
            .unwrap_or_else(|err| panic!("{}: reading inputs/: {err}", id.display()))
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
            .collect();
        assert!(!inputs.is_empty(), "{}: no `inputs/*.md`", id.display());
        assert!(!config.slice.trim().is_empty(), "{}: empty slice name", id.display());
        seen += 1;
    }
    assert!(seen >= 6, "expected the committed scenario set, found {seen}");
}

fn scenario_dirs(root: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for adapter in fs::read_dir(root).expect("scenarios root") {
        let adapter = adapter.expect("scenarios entry").path();
        if !adapter.is_dir() {
            continue;
        }
        for scenario in fs::read_dir(&adapter).expect("adapter scenarios") {
            let scenario = scenario.expect("scenario entry").path();
            if scenario.join("scenario.toml").is_file() {
                dirs.push(scenario);
            }
        }
    }
    dirs.sort();
    dirs
}

mod config {
    use super::*;

    fn stage(body: &str) -> (TempDir, PathBuf) {
        let tmp = TempDir::new().expect("tempdir");
        let dir = tmp.path().join("scenario");
        fs::create_dir_all(&dir).expect("mkdir");
        fs::write(dir.join("scenario.toml"), body).expect("write scenario.toml");
        (tmp, dir)
    }

    #[test]
    fn unlinked_adapter() {
        let (_tmp, dir) = stage(
            "adapter = \"target:unknown\"\noperation = \"build\"\nslice = \"demo\"\n\
             expect = [\"contracts\"]\n",
        );
        let err = scenario::load(&dir).expect_err("unlinked adapter refuses");
        assert!(format!("{err:#}").contains("not linked"), "{err:#}");
    }

    #[test]
    fn build_requires_expect() {
        let (_tmp, dir) =
            stage("adapter = \"target:contracts\"\noperation = \"build\"\nslice = \"demo\"\n");
        let err = scenario::load(&dir).expect_err("build without expect refuses");
        assert!(format!("{err:#}").contains("at least one `expect`"), "{err:#}");
    }

    #[test]
    fn merge_gate_allows_empty_expect() {
        let (_tmp, dir) = stage(
            "adapter = \"target:contracts\"\noperation = \"merge-preflight\"\nslice = \"demo\"\n",
        );
        scenario::load(&dir).expect("merge gates carry no mandatory expect");
    }

    #[test]
    fn absolute_expect_refused() {
        let (_tmp, dir) = stage(
            "adapter = \"target:contracts\"\noperation = \"build\"\nslice = \"demo\"\n\
             expect = [\"/etc/passwd\"]\n",
        );
        let err = scenario::load(&dir).expect_err("absolute expect refuses");
        assert!(format!("{err:#}").contains("absolute"), "{err:#}");
    }

    #[test]
    fn traversing_expect_refused() {
        let (_tmp, dir) = stage(
            "adapter = \"target:contracts\"\noperation = \"build\"\nslice = \"demo\"\n\
             expect = [\"../outside\"]\n",
        );
        let err = scenario::load(&dir).expect_err("parent-traversing expect refuses");
        assert!(format!("{err:#}").contains("plain names"), "{err:#}");
    }

    #[test]
    fn empty_expect_entry_refused() {
        let (_tmp, dir) = stage(
            "adapter = \"target:contracts\"\noperation = \"build\"\nslice = \"demo\"\n\
             expect = [\"  \"]\n",
        );
        let err = scenario::load(&dir).expect_err("blank expect entry refuses");
        assert!(format!("{err:#}").contains("empty expect entry"), "{err:#}");
    }

    #[test]
    fn unknown_key_refused() {
        let (_tmp, dir) = stage(
            "adapter = \"target:contracts\"\noperation = \"build\"\nslice = \"demo\"\n\
             expect = [\"contracts\"]\nsurprise = true\n",
        );
        let err = scenario::load(&dir).expect_err("unknown keys refuse");
        assert!(format!("{err:#}").contains("surprise"), "{err:#}");
    }
}

mod expected {
    use super::*;

    #[test]
    fn missing_artifact_fails() {
        let tmp = TempDir::new().expect("tempdir");
        let err = scenario::enforce_expected(
            "demo/one",
            tmp.path(),
            &["contracts/api.yaml".to_string()],
        )
        .expect_err("a missing artifact fails the gate");
        assert!(format!("{err:#}").contains("contracts/api.yaml"), "{err:#}");
    }

    #[test]
    fn file_and_populated_dir_pass() {
        let tmp = TempDir::new().expect("tempdir");
        fs::create_dir_all(tmp.path().join("contracts/nested")).expect("mkdir");
        fs::write(tmp.path().join("contracts/nested/api.yaml"), "openapi: 3.1.0\n")
            .expect("write");
        scenario::enforce_expected(
            "demo/one",
            tmp.path(),
            &["contracts".to_string(), "contracts/nested/api.yaml".to_string()],
        )
        .expect("a populated directory and an existing file both satisfy the gate");
    }

    #[test]
    fn empty_dir_fails() {
        let tmp = TempDir::new().expect("tempdir");
        fs::create_dir_all(tmp.path().join("contracts")).expect("mkdir");
        let err = scenario::enforce_expected("demo/one", tmp.path(), &["contracts".to_string()])
            .expect_err("an empty directory is a silent no-op");
        assert!(format!("{err:#}").contains("contracts"), "{err:#}");
    }

    #[cfg(unix)]
    #[test]
    fn symlink_cycle_terminates() {
        let tmp = TempDir::new().expect("tempdir");
        let dir = tmp.path().join("contracts/loop");
        fs::create_dir_all(&dir).expect("mkdir");
        std::os::unix::fs::symlink(tmp.path().join("contracts"), dir.join("back"))
            .expect("cycle link");
        let err = scenario::enforce_expected("demo/one", tmp.path(), &["contracts".to_string()])
            .expect_err("a cyclic, file-free tree fails rather than hanging");
        assert!(format!("{err:#}").contains("contracts"), "{err:#}");
    }

    #[cfg(unix)]
    #[test]
    fn escaping_symlink_never_satisfies() {
        let outer = TempDir::new().expect("outer tempdir");
        fs::write(outer.path().join("secret.yaml"), "outside\n").expect("write outside");
        let tmp = TempDir::new().expect("tempdir");
        std::os::unix::fs::symlink(outer.path().join("secret.yaml"), tmp.path().join("api.yaml"))
            .expect("escape link");
        let err = scenario::enforce_expected("demo/one", tmp.path(), &["api.yaml".to_string()])
            .expect_err("a symlink escaping the scratch root never satisfies the gate");
        assert!(format!("{err:#}").contains("api.yaml"), "{err:#}");
    }
}

#[test]
fn run_dirs_unique() {
    let tmp = TempDir::new().expect("tempdir");
    let first = scenario::allocate_run_dir(tmp.path()).expect("first run dir");
    let second = scenario::allocate_run_dir(tmp.path()).expect("second run dir");
    assert_ne!(first, second, "same-second runs must not share a directory");
    assert!(first.is_dir() && second.is_dir());
}

mod outcome {
    use super::*;

    fn persisted(scratch: &Path) -> serde_json::Value {
        let body = fs::read_to_string(scratch.join("report.json")).expect("report.json");
        serde_json::from_str(&body).expect("report.json is JSON")
    }

    #[test]
    fn pass_after_both_gates() {
        let tmp = TempDir::new().expect("tempdir");
        fs::write(tmp.path().join("api.yaml"), "openapi: 3.1.0\n").expect("write");
        scenario::conclude(
            "demo/one",
            tmp.path(),
            &report(BuildStatus::Success),
            &["api.yaml".to_string()],
        )
        .expect("both gates pass");
        assert_eq!(persisted(tmp.path())["outcome"], "pass");
    }

    #[test]
    fn missing_artifact_persists_fail() {
        let tmp = TempDir::new().expect("tempdir");
        scenario::conclude(
            "demo/one",
            tmp.path(),
            &report(BuildStatus::Success),
            &["api.yaml".to_string()],
        )
        .expect_err("a success report without its artifact fails");
        assert_eq!(persisted(tmp.path())["outcome"], "fail");
    }

    #[test]
    fn failing_report_persists_fail() {
        let tmp = TempDir::new().expect("tempdir");
        fs::write(tmp.path().join("api.yaml"), "openapi: 3.1.0\n").expect("write");
        scenario::conclude(
            "demo/one",
            tmp.path(),
            &report(BuildStatus::Failure),
            &["api.yaml".to_string()],
        )
        .expect_err("a failing report fails regardless of artifacts");
        assert_eq!(persisted(tmp.path())["outcome"], "fail");
    }
}
