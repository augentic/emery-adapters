//! Integration tests for iOS scaffold sync and drift detection.

use std::fs;
use std::sync::{Mutex, MutexGuard, OnceLock};

use specify_vectis::ios_scaffold::{
    DRIFT_FINDING_ID, REQUIRED_SIM_DESTINATION, ios_scaffold_drift_findings, resolve_ios_app_name,
    sync_ios_scaffold_files,
};
use specify_vectis::prepare::{PrepareCommand, run};
use tempfile::tempdir;

fn env_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(())).lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[test]
fn resolve_app_name_from_project_yml() {
    let dir = tempdir().unwrap();
    let ios = dir.path().join("iOS");
    fs::create_dir_all(&ios).expect("ios dir");
    fs::write(ios.join("project.yml"), "name: TodoApp\n").expect("project yml");

    assert_eq!(resolve_ios_app_name(dir.path()).expect("app name"), "TodoApp");
}

#[test]
fn resolve_app_name_falls_back_when_project_yml_name_invalid() {
    let dir = tempdir().unwrap();
    let ios = dir.path().join("iOS");
    fs::create_dir_all(ios.join("TodoApp")).expect("app dir");
    fs::write(ios.join("project.yml"), "name: not-valid\n").expect("project yml");
    fs::write(ios.join("TodoApp/ContentView.swift"), "struct ContentView {}").expect("swift");

    assert_eq!(resolve_ios_app_name(dir.path()).expect("app name"), "TodoApp");
}

#[test]
fn sync_restores_non_utf8_makefile() {
    let dir = tempdir().unwrap();
    let ios = dir.path().join("iOS");
    fs::create_dir_all(ios.join("TodoApp")).expect("app dir");
    fs::write(ios.join("project.yml"), "name: TodoApp\n").expect("project yml");
    fs::write(ios.join("Makefile"), [0xFF, 0xFE, 0x00]).expect("binary makefile");
    fs::write(ios.join("TodoApp/ContentView.swift"), "struct ContentView {}").expect("swift");

    let report = sync_ios_scaffold_files(dir.path()).expect("sync");
    assert!(report.synced.iter().any(|p| p == "iOS/Makefile"));

    let restored = fs::read_to_string(ios.join("Makefile")).expect("read makefile");
    assert!(restored.contains(".vectis/sim-build.sh"));
    let script = fs::read_to_string(ios.join(".vectis/sim-build.sh")).expect("read sim-build script");
    assert!(script.contains(REQUIRED_SIM_DESTINATION));
}

#[test]
fn sync_restores_drifted_makefile_destination() {
    let dir = tempdir().unwrap();
    let ios = dir.path().join("iOS");
    fs::create_dir_all(ios.join("TodoApp")).expect("app dir");
    fs::write(ios.join("project.yml"), "name: TodoApp\n").expect("project yml");
    // drift fixture — forbidden in real trees; sync must restore
    fs::write(ios.join("Makefile"), "-destination 'platform=iOS Simulator,name=iPhone 16'\n")
        .expect("makefile");
    fs::write(ios.join("TodoApp/ContentView.swift"), "struct ContentView {}").expect("swift");

    let report = sync_ios_scaffold_files(dir.path()).expect("sync");
    assert!(report.synced.iter().any(|p| p == "iOS/Makefile"));

    let restored = fs::read_to_string(ios.join("Makefile")).expect("read makefile");
    assert!(restored.contains(".vectis/sim-build.sh"));
    assert!(!restored.contains("iPhone 16"));
    let script = fs::read_to_string(ios.join(".vectis/sim-build.sh")).expect("read sim-build script");
    assert!(script.contains(REQUIRED_SIM_DESTINATION));
}

#[test]
fn sync_restores_drifted_sim_build_script() {
    let dir = tempdir().unwrap();
    let ios = dir.path().join("iOS");
    fs::create_dir_all(ios.join("TodoApp")).expect("app dir");
    fs::create_dir_all(ios.join(".vectis")).expect("vectis dir");
    fs::write(ios.join("project.yml"), "name: TodoApp\n").expect("project yml");
    // drift fixture — forbidden in real trees; sync must restore
    fs::write(
        ios.join(".vectis/sim-build.sh"),
        "DEST='platform=iOS Simulator,name=iPhone 16'\n",
    )
    .expect("drifted script");
    fs::write(ios.join("TodoApp/ContentView.swift"), "struct ContentView {}").expect("swift");

    let report = sync_ios_scaffold_files(dir.path()).expect("sync");
    assert!(report.synced.iter().any(|p| p == "iOS/.vectis/sim-build.sh"));

    let restored = fs::read_to_string(ios.join(".vectis/sim-build.sh")).expect("read script");
    assert!(restored.contains(REQUIRED_SIM_DESTINATION));
    assert!(!restored.contains("iPhone 16"));
}

#[test]
fn sync_is_noop_when_makefile_matches_template() {
    let dir = tempdir().unwrap();
    sync_ios_scaffold_files(dir.path()).expect("noop sync without ios dir");

    let ios = dir.path().join("iOS");
    fs::create_dir_all(ios.join("Counter")).expect("app dir");
    fs::write(ios.join("project.yml"), "name: Counter\n").expect("project yml");
    fs::write(ios.join("Counter/ContentView.swift"), "struct ContentView {}").expect("swift");

    let first = sync_ios_scaffold_files(dir.path()).expect("first sync");
    assert!(first.synced.iter().any(|p| p == "iOS/Makefile"));

    let makefile = fs::read_to_string(ios.join("Makefile")).expect("read makefile");
    let second = sync_ios_scaffold_files(dir.path()).expect("second sync");
    assert!(second.synced.is_empty());
    assert!(second.unchanged.iter().any(|p| p == "iOS/Makefile"));
    assert_eq!(fs::read_to_string(ios.join("Makefile")).expect("read makefile"), makefile);
}

#[test]
fn drift_findings_flag_missing_makefile() {
    let dir = tempdir().unwrap();
    let ios = dir.path().join("iOS");
    fs::create_dir_all(ios.join("Counter")).expect("app dir");
    fs::write(ios.join("project.yml"), "name: Counter\n").expect("project yml");

    let findings = ios_scaffold_drift_findings(dir.path());
    assert!(
        findings.iter().any(|f| f["path"] == "iOS/Makefile"),
        "expected missing Makefile finding: {findings:?}"
    );
}

#[test]
fn drift_findings_flag_named_simulator() {
    let dir = tempdir().unwrap();
    let ios = dir.path().join("iOS");
    fs::create_dir_all(ios.join("Counter")).expect("app dir");
    fs::write(ios.join("project.yml"), "name: Counter\n").expect("project yml");
    fs::write(ios.join("Makefile"), "-destination 'platform=iOS Simulator,name=iPhone 16'\n")
        .expect("makefile");

    let findings = ios_scaffold_drift_findings(dir.path());
    assert_eq!(findings.len(), 3);
    assert!(findings.iter().all(|f| f["id"] == DRIFT_FINDING_ID));
    assert!(
        findings.iter().any(|f| {
            f["path"] == "iOS/Makefile"
                && f["message"].as_str().unwrap().contains("forbidden named simulator")
        }),
        "expected named simulator hint in Makefile finding: {findings:?}"
    );
    assert!(
        findings.iter().any(|f| f["path"] == "iOS/.vectis/sim-build.sh"),
        "expected missing sim-build.sh finding: {findings:?}"
    );
    assert!(
        findings.iter().any(|f| f["path"] == "iOS/project.yml"),
        "expected drifted project.yml finding: {findings:?}"
    );
}

#[test]
fn drift_findings_flag_named_simulator_in_script() {
    let dir = tempdir().unwrap();
    let ios = dir.path().join("iOS");
    fs::create_dir_all(ios.join(".vectis")).expect("vectis dir");
    fs::write(ios.join("project.yml"), "name: Counter\n").expect("project yml");
    // drift fixture — forbidden in real trees; sync must restore
    fs::write(
        ios.join(".vectis/sim-build.sh"),
        "DEST='platform=iOS Simulator,name=iPhone 16'\n",
    )
    .expect("drifted script");

    let findings = ios_scaffold_drift_findings(dir.path());
    assert!(
        findings.iter().any(|f| {
            f["path"] == "iOS/.vectis/sim-build.sh"
                && f["message"].as_str().unwrap().contains("forbidden named simulator")
        }),
        "expected named simulator hint in sim-build.sh finding: {findings:?}"
    );
}

#[test]
fn prepare_build_syncs_drifted_ios_makefile() {
    let _guard = env_lock();
    let tmp = tempdir().unwrap();
    let project = tmp.path().join("project");
    let slice = project.join(".specify/slices/feature");
    fs::create_dir_all(&slice).expect("slice dir");
    fs::create_dir_all(project.join(".specify")).expect("specify dir");
    fs::write(project.join(".specify/project.yaml"), "platforms:\n  - core\n  - ios\n")
        .expect("project yaml");

    let ios = project.join("iOS");
    fs::create_dir_all(ios.join("Counter")).expect("app dir");
    fs::write(ios.join("project.yml"), "name: Counter\n").expect("project yml");
    fs::write(ios.join("Makefile"), "-destination 'platform=iOS Simulator,name=iPhone 16'\n")
        .expect("makefile");
    fs::write(ios.join("Counter/ContentView.swift"), "struct ContentView {}").expect("swift");

    let previous = std::env::var_os("PROJECT_DIR");
    #[expect(unsafe_code, reason = "edition-2024 set_var is unsafe; env_lock serializes access")]
    // SAFETY: this test serializes PROJECT_DIR mutation with `env_lock`.
    let () = unsafe { std::env::set_var("PROJECT_DIR", &project) };

    let outcome = run(&PrepareCommand::Build(specify_vectis::prepare::BuildArgs {
        slice_dir: slice.strip_prefix(&project).unwrap().to_path_buf(),
    }))
    .expect("prepare build");

    #[expect(
        unsafe_code,
        reason = "edition-2024 set_var/remove_var are unsafe; env_lock serializes access"
    )]
    // SAFETY: this test serializes PROJECT_DIR mutation with `env_lock`.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("PROJECT_DIR", value),
            None => std::env::remove_var("PROJECT_DIR"),
        }
    }

    let synced = outcome["scaffold_sync"]["ios"]["synced"].as_array().expect("synced array");
    assert!(synced.iter().any(|v| v == "iOS/Makefile"));

    let restored = fs::read_to_string(ios.join("Makefile")).expect("read makefile");
    assert!(restored.contains(".vectis/sim-build.sh"));
    assert!(!restored.contains("iPhone 16"));
    let script = fs::read_to_string(ios.join(".vectis/sim-build.sh")).expect("read sim-build script");
    assert!(script.contains(REQUIRED_SIM_DESTINATION));
}

#[test]
fn sync_ios_scaffold_command_restores_drifted_makefile() {
    let _guard = env_lock();
    let tmp = tempdir().unwrap();
    let project = tmp.path().join("project");
    fs::create_dir_all(project.join(".specify")).expect("specify dir");
    fs::write(project.join(".specify/project.yaml"), "platforms:\n  - core\n  - ios\n")
        .expect("project yaml");

    let ios = project.join("iOS");
    fs::create_dir_all(ios.join("Counter")).expect("app dir");
    fs::write(ios.join("project.yml"), "name: Counter\n").expect("project yml");
    fs::write(ios.join("Makefile"), "-destination 'platform=iOS Simulator,name=iPhone 16'\n")
        .expect("makefile");
    fs::write(ios.join("Counter/ContentView.swift"), "struct ContentView {}").expect("swift");

    let previous = std::env::var_os("PROJECT_DIR");
    #[expect(unsafe_code, reason = "edition-2024 set_var is unsafe; env_lock serializes access")]
    // SAFETY: this test serializes PROJECT_DIR mutation with `env_lock`.
    let () = unsafe { std::env::set_var("PROJECT_DIR", &project) };

    let outcome = specify_vectis::sync::run(&specify_vectis::sync::SyncCommand::IosScaffold(
        specify_vectis::sync::IosScaffoldArgs { path: None },
    ))
    .expect("sync ios-scaffold");

    #[expect(
        unsafe_code,
        reason = "edition-2024 set_var/remove_var are unsafe; env_lock serializes access"
    )]
    // SAFETY: this test serializes PROJECT_DIR mutation with `env_lock`.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("PROJECT_DIR", value),
            None => std::env::remove_var("PROJECT_DIR"),
        }
    }

    let synced = outcome["scaffold_sync"]["ios"]["synced"].as_array().expect("synced array");
    assert!(synced.iter().any(|v| v == "iOS/Makefile"));

    let restored = fs::read_to_string(ios.join("Makefile")).expect("read makefile");
    assert!(restored.contains(".vectis/sim-build.sh"));
    assert!(!restored.contains("iPhone 16"));
    let script = fs::read_to_string(ios.join(".vectis/sim-build.sh")).expect("read sim-build script");
    assert!(script.contains(REQUIRED_SIM_DESTINATION));
}
