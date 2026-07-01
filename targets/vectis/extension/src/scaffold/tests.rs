//! Tests for `vectis scaffold` planning, writing, and the dispatcher. The five
//! plan-shape unit tests the document flagged for collapse are folded into
//! three table-style tests (`golden_hashes_*`, `plan_substitutions_and_caps`,
//! `write_plan_refuses_existing_roots`); the `.gitignore` merge and the `run`
//! dispatcher (which mutates `PROJECT_DIR`) stay distinct because they reach
//! `runtime::merge_gitignore` and the env-bound dispatch path. They remain
//! in-crate so the collapse stays coverage-neutral over `runtime.rs`.

use std::fs;
use std::sync::{Mutex, MutexGuard, OnceLock};

use sha2::{Digest, Sha256};
use tempfile::tempdir;

use super::templates::registry::{android, core, ios};
use super::*;

const CORE_RENDER_ONLY_SHA256: &str =
    "be4d2c16b0736c7be8f137990e7039055c66c228957ddd1b13d54caa8433b7b0";
const IOS_RENDER_ONLY_SHA256: &str =
    "c7e71e7e1c512bb98ecf591d5d57cbd127479631236b01130688e113a9cd1565";
const ANDROID_RENDER_ONLY_SHA256: &str =
    "6d6888bc19e0951063b1e90f9129b9bdd70fe3b2c2fb479cba209bc8df1e0c0b";

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

fn env_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(())).lock().unwrap_or_else(std::sync::PoisonError::into_inner)
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

// Per-target substitutions and capability gating: the core plan preserves
// template order and substitutes the app struct + android package; the ios
// plan renders the `http` capability block; the android plan omits the
// network-security config without a network cap and writes it under `http`. An
// unknown capability is rejected.
#[test]
fn plan_substitutions_and_caps() {
    let versions = versions();

    let plan = plan_core("Counter", "com.example.counter", &[], &versions).unwrap();
    assert_eq!(plan.files.len(), core::ENTRIES.len());
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
    assert_eq!(ios.files.len(), ios::ENTRIES.len());
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
    assert_eq!(android_bare.files.len(), android::ENTRIES.len() - 1);
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
    assert_eq!(android_http.files.len(), android::ENTRIES.len());
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

// `write_plan` refuses to clobber an existing core file or an existing ios /
// android shell root, leaving the prior tree untouched.
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
    // `specify init` writes a root `.gitignore` in every project, so
    // the bootstrap path scaffolds into an initialised repo: the
    // template's missing lines append; operator content survives.
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

    // Idempotent: a second merge pass appends nothing.
    let plan_again = plan_core("Counter", "com.vectis.counter", &[], &versions()).unwrap();
    let gitignore_template = plan_again
        .files
        .iter()
        .find(|file| file.relative_path == ".gitignore")
        .expect("core plan carries .gitignore");
    runtime::merge_gitignore(&dir.path().join(".gitignore"), &gitignore_template.contents)
        .expect("re-merge succeeds");
    let remerged = fs::read_to_string(dir.path().join(".gitignore")).unwrap();
    assert_eq!(merged, remerged, "second merge is a no-op");
}

#[test]
fn run_writes_under_project_dir() {
    let _guard = env_lock();
    let dir = tempdir().unwrap();
    let previous = std::env::var_os("PROJECT_DIR");
    #[expect(unsafe_code, reason = "edition-2024 set_var is unsafe; env_lock serializes access")]
    // SAFETY: this test serializes PROJECT_DIR mutation with `env_lock`.
    let () = unsafe { std::env::set_var("PROJECT_DIR", dir.path()) };

    let command = ScaffoldCommand::Core(CoreArgs {
        common: CommonArgs {
            app_name: "Counter".into(),
            caps: None,
            version_file: None,
        },
        android_package: None,
    });
    let value = run(&command).expect("run succeeds");
    assert_eq!(value["target"], "core");
    assert!(dir.path().join("shared/src/app.rs").is_file());

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
}
