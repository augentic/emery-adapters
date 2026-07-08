//! Tests for scaffold planning, writing, and the dispatcher.

use std::fs;

use sha2::{Digest, Sha256};
use tempfile::tempdir;
use vectis::scaffold::{
    CommonArgs, CoreArgs, ScaffoldCommand, ScaffoldError, ScaffoldPlan, Versions, parse_caps,
    plan_android, plan_core, plan_ios, run_at, write_plan,
};

const CORE_RENDER_ONLY_SHA256: &str =
    "f83be964272287a86228aefa3219e6f248f977b42880ecf3eeccc353f4a84b1e";
const IOS_RENDER_ONLY_SHA256: &str =
    "69bba9c2e5726b1355daf97d72ce98f55150694b32b890190af75025778035dc";
const ANDROID_RENDER_ONLY_SHA256: &str =
    "bd764b6f12aaa48fb70bfa447eef5d87234de96965e725cbe2aaa872f966333c";

// Template-registry entry counts, pinned by the build.rs manifest gate
// (`EXPECTED_COUNTS`).
const CORE_ENTRIES: usize = 13;
const IOS_ENTRIES: usize = 11;
const ANDROID_ENTRIES: usize = 23;

fn versions() -> Versions {
    Versions::embedded().expect("embedded versions parse")
}

fn digest_plan(plan: &ScaffoldPlan) -> String {
    let mut hasher = Sha256::new();
    hasher.update(plan.target.as_bytes());
    hasher.update([0]);
    for file in &plan.files {
        hasher.update(file.relative_path.as_bytes());
        hasher.update([0]);
        hasher.update(file.contents.as_bytes());
        hasher.update([0]);
    }
    hasher.finalize().iter().fold(String::with_capacity(64), |mut hex, byte| {
        use std::fmt::Write as _;
        let _ = write!(hex, "{byte:02x}");
        hex
    })
}

#[test]
fn golden_hashes_match_current_render_only_output() {
    let versions = versions();
    let core = plan_core("Counter", "com.vectis.counter", &[], &versions).unwrap();
    let ios = plan_ios("Counter", "com.vectis.counter", &[], &versions).unwrap();
    let android = plan_android("Counter", "com.vectis.counter", &[], &versions).unwrap();

    assert_eq!(digest_plan(&core), CORE_RENDER_ONLY_SHA256);
    assert_eq!(digest_plan(&ios), IOS_RENDER_ONLY_SHA256);
    assert_eq!(digest_plan(&android), ANDROID_RENDER_ONLY_SHA256);
}

#[test]
fn plan_substitutions_and_caps() {
    let versions = versions();

    let plan = plan_core("Counter", "com.example.counter", &[], &versions).unwrap();
    assert_eq!(plan.files.len(), CORE_ENTRIES);
    assert_eq!(plan.files[0].relative_path, "Cargo.toml");
    assert_eq!(plan.files[8].relative_path, "shared/src/bin/codegen.rs");
    let app = plan.files.iter().find(|file| file.relative_path == "shared/src/app.rs").unwrap();
    assert!(app.contents.contains("Hello from Counter"));
    assert!(!app.contents.contains("__APP_STRUCT__"));
    let codegen =
        plan.files.iter().find(|file| file.relative_path == "shared/src/bin/codegen.rs").unwrap();
    assert!(codegen.contents.contains("com.example.counter"));
    assert!(!codegen.contents.contains("__ANDROID_PACKAGE__"));

    let ios_caps = parse_caps(Some("http")).unwrap();
    let ios = plan_ios("Counter", "com.vectis.counter", &ios_caps, &versions).unwrap();
    assert_eq!(ios.files.len(), IOS_ENTRIES);
    assert!(ios.files.iter().any(|file| file.relative_path == "iOS/Counter/CounterApp.swift"));
    assert!(ios.files.iter().any(|file| {
        file.relative_path
            == "iOS/Counter/Resources/Assets.xcassets/AppIcon.appiconset/Contents.json"
    }));
    let project_yml =
        ios.files.iter().find(|file| file.relative_path == "iOS/project.yml").unwrap();
    assert!(project_yml.contents.contains("Resources"));
    let core_swift =
        ios.files.iter().find(|file| file.relative_path == "iOS/Counter/Core.swift").unwrap();
    assert!(core_swift.contents.contains("case .http"));
    assert!(core_swift.contents.contains("performHttpRequest"));
    assert!(!core_swift.contents.contains("<<<CAP:"));

    let android_bare = plan_android("Counter", "com.vectis.counter", &[], &versions).unwrap();
    assert_eq!(android_bare.files.len(), ANDROID_ENTRIES - 1);
    assert!(
        !android_bare
            .files
            .iter()
            .any(|file| file.relative_path.ends_with("network_security_config.xml"))
    );
    assert!(android_bare.files.iter().any(|file| {
        file.relative_path == "Android/app/src/main/java/com/vectis/counter/CounterApplication.kt"
    }));
    assert!(android_bare.files.iter().any(|file| {
        file.relative_path == "Android/app/src/main/res/mipmap-anydpi-v26/ic_launcher.xml"
    }));
    assert!(android_bare.files.iter().any(|file| {
        file.relative_path == "Android/app/src/main/res/mipmap-anydpi-v26/ic_launcher_round.xml"
    }));
    let manifest = android_bare
        .files
        .iter()
        .find(|file| file.relative_path == "Android/app/src/main/AndroidManifest.xml")
        .unwrap();
    assert!(manifest.contents.contains(r#"android:icon="@mipmap/ic_launcher""#));
    assert!(
        !android_bare.files.iter().any(|file| file.relative_path == "Android/local.properties"),
        "host-derived local.properties is outside the WASI renderer"
    );

    let android_caps = parse_caps(Some("http")).unwrap();
    let android_http =
        plan_android("Counter", "com.vectis.counter", &android_caps, &versions).unwrap();
    assert_eq!(android_http.files.len(), ANDROID_ENTRIES);
    assert!(
        android_http
            .files
            .iter()
            .any(|file| file.relative_path.ends_with("network_security_config.xml"))
    );

    match parse_caps(Some("http,bogus")).expect_err("unknown cap must fail") {
        ScaffoldError::InvalidProject { message } => {
            assert!(message.contains("\"bogus\""));
            assert!(message.contains("http"));
            assert!(message.contains("sse"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn write_plan_refuses_existing_roots() {
    let versions = versions();

    let core_dir = tempdir().unwrap();
    fs::write(core_dir.path().join("Cargo.toml"), "pre-existing").unwrap();
    let core = plan_core("Counter", "com.vectis.counter", &[], &versions).unwrap();
    match write_plan(core_dir.path(), &core).expect_err("must refuse overwrite") {
        ScaffoldError::InvalidProject { message } => {
            assert!(message.contains("refusing to overwrite existing file"));
            assert!(message.contains("Cargo.toml"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert!(!core_dir.path().join("shared/src/app.rs").exists());
    assert_eq!(fs::read_to_string(core_dir.path().join("Cargo.toml")).unwrap(), "pre-existing");

    let ios_dir = tempdir().unwrap();
    fs::create_dir_all(ios_dir.path().join("iOS")).unwrap();
    let ios = plan_ios("Counter", "com.vectis.counter", &[], &versions).unwrap();
    match write_plan(ios_dir.path(), &ios).expect_err("must refuse iOS root") {
        ScaffoldError::InvalidProject { message } => {
            assert!(message.contains("refusing to overwrite existing iOS shell"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert!(!ios_dir.path().join("iOS/project.yml").exists());

    let android_dir = tempdir().unwrap();
    fs::create_dir_all(android_dir.path().join("Android")).unwrap();
    let android = plan_android("Counter", "com.vectis.counter", &[], &versions).unwrap();
    match write_plan(android_dir.path(), &android).expect_err("must refuse Android root") {
        ScaffoldError::InvalidProject { message } => {
            assert!(message.contains("refusing to overwrite existing Android shell"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert!(!android_dir.path().join("Android/Makefile").exists());
}

#[test]
fn write_plan_merges_existing_gitignore() {
    // `specify init` writes a root `.gitignore` in every project, so the
    // bootstrap path always scaffolds into an initialised repo.
    let dir = tempdir().unwrap();
    fs::write(dir.path().join(".gitignore"), ".specify/cache/\n/target\n").unwrap();
    let plan = plan_core("Counter", "com.vectis.counter", &[], &versions()).unwrap();
    write_plan(dir.path(), &plan).expect("gitignore collision merges");

    let merged = fs::read_to_string(dir.path().join(".gitignore")).unwrap();
    assert!(merged.starts_with(".specify/cache/\n/target\n"), "operator content survives");
    assert!(merged.contains("# Vectis scaffold"));
    assert!(merged.contains(".DS_Store"), "template lines appended");
    assert_eq!(merged.matches("/target").count(), 1, "duplicate lines are not re-appended");
    assert!(dir.path().join("shared/src/app.rs").exists(), "rest of the plan writes normally");

    // Idempotent: a merge pass over an already-merged .gitignore appends
    // nothing (fresh dir so the rest of the plan writes cleanly).
    let again = tempdir().unwrap();
    fs::write(again.path().join(".gitignore"), &merged).unwrap();
    let plan_again = plan_core("Counter", "com.vectis.counter", &[], &versions()).unwrap();
    write_plan(again.path(), &plan_again).expect("re-merge succeeds");
    let remerged = fs::read_to_string(again.path().join(".gitignore")).unwrap();
    assert_eq!(merged, remerged, "second merge is a no-op");
}

#[test]
fn run_at_writes_under_project_dir() {
    let dir = tempdir().unwrap();
    let command = ScaffoldCommand::Core(CoreArgs {
        common: CommonArgs::for_app("Counter".into()),
        android_package: None,
    });
    let value = run_at(dir.path(), &command).expect("run_at succeeds");
    assert_eq!(value["target"], "core");
    assert!(dir.path().join("shared/src/app.rs").is_file());
}
