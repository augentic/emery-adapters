//! Integration tests for iOS scaffold sync and drift detection.

use std::fs;

use specify_vectis::ios_scaffold::{
    DRIFT_FINDING_ID, REQUIRED_CARGO_SWIFT_XCFRAMEWORK_NAME, REQUIRED_SIM_DESTINATION,
    REQUIRED_SWIFT_TREAT_WARNINGS_AS_ERRORS, ios_scaffold_drift_findings, resolve_ios_app_name,
    sync_ios_scaffold_files,
};
use specify_vectis::prepare::{PrepareCommand, run};
use specify_vectis::scaffold::{ScaffoldPlan, Versions, parse_caps, plan_ios};
use tempfile::tempdir;

use crate::engine_support::{ProjectDirGuard, env_lock};

fn versions() -> Versions {
    Versions::embedded().expect("embedded versions parse")
}

fn plan(caps: Option<&str>) -> ScaffoldPlan {
    let caps = parse_caps(caps).expect("parse caps");
    plan_ios("Counter", "com.vectis.counter", &caps, &versions()).expect("plan ios")
}

fn project_yml_contents(plan: &ScaffoldPlan) -> &str {
    plan.files
        .iter()
        .find(|file| file.relative_path == "iOS/project.yml")
        .unwrap_or_else(|| panic!("iOS/project.yml missing from ios plan"))
        .contents
        .as_str()
}

fn assert_project_yml_strict_flags(plan: &ScaffoldPlan) {
    let contents = project_yml_contents(plan);
    assert!(
        contents.contains(REQUIRED_SWIFT_TREAT_WARNINGS_AS_ERRORS),
        "project.yml must set SWIFT_TREAT_WARNINGS_AS_ERRORS under settings.base:\n{contents}"
    );
    assert!(
        !contents.contains("OTHER_LDFLAGS"),
        "project.yml must not suppress linker warnings via OTHER_LDFLAGS:\n{contents}"
    );
    assert!(!contents.contains("-w"), "project.yml must not contain -w linker flag:\n{contents}");
}

fn makefile_contents(plan: &ScaffoldPlan) -> &str {
    plan.files
        .iter()
        .find(|file| file.relative_path == "iOS/Makefile")
        .unwrap_or_else(|| panic!("iOS/Makefile missing from ios plan"))
        .contents
        .as_str()
}

fn assert_makefile_xcframework_name(plan: &ScaffoldPlan) {
    let contents = makefile_contents(plan);
    assert!(
        contents.contains(REQUIRED_CARGO_SWIFT_XCFRAMEWORK_NAME),
        "iOS/Makefile package target must pass {REQUIRED_CARGO_SWIFT_XCFRAMEWORK_NAME}:\n{contents}"
    );
}

#[test]
fn ios_scaffold_project_yml_treats_warnings_as_errors_render_only() {
    assert_project_yml_strict_flags(&plan(None));
}

#[test]
fn ios_scaffold_makefile_sets_xcframework_name_render_only() {
    assert_makefile_xcframework_name(&plan(None));
}

#[test]
fn ios_scaffold_makefile_sets_xcframework_name_http() {
    assert_makefile_xcframework_name(&plan(Some("http")));
}

#[test]
fn ios_scaffold_project_yml_treats_warnings_as_errors_http() {
    assert_project_yml_strict_flags(&plan(Some("http")));
}

#[test]
fn sync_restores_drifted_project_yml_linker_suppression() {
    let dir = tempdir().unwrap();
    let ios = dir.path().join("iOS");
    fs::create_dir_all(ios.join("Counter")).expect("app dir");
    fs::write(ios.join("project.yml"), "name: Counter\n").expect("project yml");
    fs::write(
        ios.join("project.yml"),
        "name: Counter\nsettings:\n  configs:\n    Debug:\n      OTHER_LDFLAGS: [\"-w\"]\n",
    )
    .expect("drifted project yml");
    fs::write(ios.join("Counter/ContentView.swift"), "struct ContentView {}").expect("swift");

    let report = sync_ios_scaffold_files(dir.path()).expect("sync");
    assert!(report.synced.iter().any(|p| p == "iOS/project.yml"));

    let restored = fs::read_to_string(ios.join("project.yml")).expect("read project yml");
    assert!(restored.contains(REQUIRED_SWIFT_TREAT_WARNINGS_AS_ERRORS));
    assert!(!restored.contains("OTHER_LDFLAGS"));
    assert!(!restored.contains("-w"));
}

#[test]
fn drift_findings_flag_linker_warning_suppression_in_project_yml() {
    let dir = tempdir().unwrap();
    let ios = dir.path().join("iOS");
    fs::create_dir_all(ios.join("Counter")).expect("app dir");
    fs::write(
        ios.join("project.yml"),
        "name: Counter\nsettings:\n  configs:\n    Debug:\n      OTHER_LDFLAGS: [\"-w\"]\n",
    )
    .expect("drifted project yml");

    let findings = ios_scaffold_drift_findings(dir.path());
    assert!(
        findings.iter().any(|f| {
            f["path"] == "iOS/project.yml"
                && f["message"].as_str().unwrap().contains("forbidden linker warning suppression")
        }),
        "expected linker suppression hint in project.yml finding: {findings:?}"
    );
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
    assert!(restored.contains(".vectis/sim-dev.sh"));
    assert!(restored.contains("sim-run"));
    assert!(restored.contains(REQUIRED_CARGO_SWIFT_XCFRAMEWORK_NAME));
    let script =
        fs::read_to_string(ios.join(".vectis/sim-build.sh")).expect("read sim-build script");
    assert!(script.contains(REQUIRED_SIM_DESTINATION));
    assert!(script.contains("-derivedDataPath"));
}

#[test]
fn ios_scaffold_plan_includes_sim_dev_script() {
    let plan = plan(None);
    let sim_dev = plan
        .files
        .iter()
        .find(|file| file.relative_path == "iOS/.vectis/sim-dev.sh")
        .unwrap_or_else(|| panic!("iOS/.vectis/sim-dev.sh missing from ios plan"));
    assert!(sim_dev.contents.contains("simctl install"));
    assert!(sim_dev.contents.contains("simctl launch"));
    assert!(sim_dev.contents.contains("DerivedData"));
}

#[test]
fn sync_restores_drifted_sim_dev_script() {
    let dir = tempdir().unwrap();
    let ios = dir.path().join("iOS");
    fs::create_dir_all(ios.join("TodoApp")).expect("app dir");
    fs::create_dir_all(ios.join(".vectis")).expect("vectis dir");
    fs::write(ios.join("project.yml"), "name: TodoApp\n").expect("project yml");
    fs::write(ios.join(".vectis/sim-dev.sh"), "#!/bin/bash\necho broken\n")
        .expect("drifted script");
    fs::write(ios.join("TodoApp/ContentView.swift"), "struct ContentView {}").expect("swift");

    let report = sync_ios_scaffold_files(dir.path()).expect("sync");
    assert!(report.synced.iter().any(|p| p == "iOS/.vectis/sim-dev.sh"));

    let restored = fs::read_to_string(ios.join(".vectis/sim-dev.sh")).expect("read script");
    assert!(restored.contains("simctl install"));
    assert!(restored.contains("simctl launch"));
}

#[test]
fn sync_restores_makefile_sim_run_targets() {
    let dir = tempdir().unwrap();
    let ios = dir.path().join("iOS");
    fs::create_dir_all(ios.join("TodoApp")).expect("app dir");
    fs::write(ios.join("project.yml"), "name: TodoApp\n").expect("project yml");
    fs::write(ios.join("Makefile"), "sim-build:\n\t@echo noop\n").expect("drifted makefile");
    fs::write(ios.join("TodoApp/ContentView.swift"), "struct ContentView {}").expect("swift");

    let report = sync_ios_scaffold_files(dir.path()).expect("sync");
    assert!(report.synced.iter().any(|p| p == "iOS/Makefile"));

    let restored = fs::read_to_string(ios.join("Makefile")).expect("read makefile");
    assert!(restored.contains("sim-run"));
    assert!(restored.contains("run: sim-run"));
    assert!(restored.contains(".vectis/sim-dev.sh"));
    assert!(restored.contains("DerivedData/"));
    assert!(restored.contains(REQUIRED_CARGO_SWIFT_XCFRAMEWORK_NAME));
}

#[test]
fn drift_findings_flag_missing_xcframework_name_in_makefile() {
    let dir = tempdir().unwrap();
    let ios = dir.path().join("iOS");
    fs::create_dir_all(ios.join("Counter")).expect("app dir");
    fs::write(ios.join("project.yml"), "name: Counter\n").expect("project yml");
    fs::write(
        ios.join("Makefile"),
        "package:\n\tcargo swift package --name Shared --platforms ios --lib-type static --features uniffi\n",
    )
    .expect("drifted makefile");

    let findings = ios_scaffold_drift_findings(dir.path());
    assert!(
        findings.iter().any(|f| {
            f["path"] == "iOS/Makefile"
                && f["message"]
                    .as_str()
                    .unwrap()
                    .contains("Makefile cargo swift package must pass --xcframework-name sharedFFI")
        }),
        "expected xcframework-name hint in Makefile finding: {findings:?}"
    );
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
    let script =
        fs::read_to_string(ios.join(".vectis/sim-build.sh")).expect("read sim-build script");
    assert!(script.contains(REQUIRED_SIM_DESTINATION));
    assert!(script.contains("-derivedDataPath"));
}

#[test]
fn sync_restores_drifted_sim_build_script() {
    let dir = tempdir().unwrap();
    let ios = dir.path().join("iOS");
    fs::create_dir_all(ios.join("TodoApp")).expect("app dir");
    fs::create_dir_all(ios.join(".vectis")).expect("vectis dir");
    fs::write(ios.join("project.yml"), "name: TodoApp\n").expect("project yml");
    // drift fixture — forbidden in real trees; sync must restore
    fs::write(ios.join(".vectis/sim-build.sh"), "DEST='platform=iOS Simulator,name=iPhone 16'\n")
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
    assert_eq!(findings.len(), 4);
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
        findings.iter().any(|f| f["path"] == "iOS/.vectis/sim-dev.sh"),
        "expected missing sim-dev.sh finding: {findings:?}"
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
    fs::write(ios.join(".vectis/sim-build.sh"), "DEST='platform=iOS Simulator,name=iPhone 16'\n")
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

    let _project_dir = ProjectDirGuard::set(&project);

    let outcome = run(&PrepareCommand::Build(specify_vectis::prepare::BuildArgs {
        slice_dir: slice.strip_prefix(&project).unwrap().to_path_buf(),
    }))
    .expect("prepare build");

    let synced = outcome["scaffold_sync"]["ios"]["synced"].as_array().expect("synced array");
    assert!(synced.iter().any(|v| v == "iOS/Makefile"));

    let restored = fs::read_to_string(ios.join("Makefile")).expect("read makefile");
    assert!(restored.contains(".vectis/sim-build.sh"));
    assert!(!restored.contains("iPhone 16"));
    let script =
        fs::read_to_string(ios.join(".vectis/sim-build.sh")).expect("read sim-build script");
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

    let _project_dir = ProjectDirGuard::set(&project);

    let outcome = specify_vectis::sync::run(&specify_vectis::sync::SyncCommand::IosScaffold(
        specify_vectis::sync::IosScaffoldArgs { path: None },
    ))
    .expect("sync ios-scaffold");

    let synced = outcome["scaffold_sync"]["ios"]["synced"].as_array().expect("synced array");
    assert!(synced.iter().any(|v| v == "iOS/Makefile"));

    let restored = fs::read_to_string(ios.join("Makefile")).expect("read makefile");
    assert!(restored.contains(".vectis/sim-build.sh"));
    assert!(!restored.contains("iPhone 16"));
    let script =
        fs::read_to_string(ios.join(".vectis/sim-build.sh")).expect("read sim-build script");
    assert!(script.contains(REQUIRED_SIM_DESTINATION));
}
