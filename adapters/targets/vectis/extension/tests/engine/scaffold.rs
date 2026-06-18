//! Re-homed from `src/scaffold/tests.rs`: `vectis scaffold` plan rendering,
//! per-target substitution, capability gating, and overwrite refusal, driven
//! through the public `scaffold::{plan_core, plan_ios, plan_android,
//! write_plan, parse_caps, Versions}` surface. Golden byte hashes pin the full
//! render-only output. The `.gitignore` merge (pub(super) `merge_gitignore`)
//! and the `run` dispatcher env-mutation test stay as `src` units because they
//! reach internal items the public API does not expose.

use std::fs;

use sha2::{Digest, Sha256};
use specify_vectis::scaffold::{
    ScaffoldError, ScaffoldPlan, Versions, parse_caps, plan_android, plan_core, plan_ios,
    write_plan,
};
use tempfile::tempdir;

const CORE_RENDER_ONLY_SHA256: &str =
    "3db14983887828dff03d604ad449f1eb098e1008db4b83df525af6cbb64abeff";
const IOS_RENDER_ONLY_SHA256: &str =
    "49b83d8ec33b826099f6d79b646ba9dbe1689c17bdfd8e51820b85651d10abec";
const ANDROID_RENDER_ONLY_SHA256: &str =
    "31385d870d8cfddd0af7676ac9a7567f5cea979dedc792b4dcabc45290c0ee1f";

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

// Render-only (no-cap) core / ios / android plans hash to their golden digests
// — a byte-exact pin over template declaration order plus substitutions.
#[test]
fn scaffold_plans_render_deterministic_golden() {
    let versions = versions();
    let core = plan_core("Counter", "com.vectis.counter", &[], &versions).unwrap();
    let ios = plan_ios("Counter", "com.vectis.counter", &[], &versions).unwrap();
    let android = plan_android("Counter", "com.vectis.counter", &[], &versions).unwrap();

    assert_eq!(digest_plan(&core), CORE_RENDER_ONLY_SHA256);
    assert_eq!(digest_plan(&ios), IOS_RENDER_ONLY_SHA256);
    assert_eq!(digest_plan(&android), ANDROID_RENDER_ONLY_SHA256);
}

// Substitutions land per target and capability blocks gate correctly: the core
// plan preserves template order and substitutes the app struct + android
// package; the ios plan renders the `http` capability block (no leftover
// `<<<CAP:` markers); the android plan omits the network-security config
// without a network cap and adds exactly that one file under `http`. An
// unknown capability is rejected. (The render-only `files.len() ==
// ENTRIES.len()` equalities the src units carried are subsumed by the golden
// hashes above, which pin the full file set; `ENTRIES` is module-private.)
#[test]
fn scaffold_plans_substitute_and_gate_caps() {
    let versions = versions();

    let core = plan_core("Counter", "com.example.counter", &[], &versions).unwrap();
    assert_eq!(core.files[0].relative_path, "Cargo.toml");
    assert_eq!(core.files[8].relative_path, "shared/src/bin/codegen.rs");
    let app = core.files.iter().find(|file| file.relative_path == "shared/src/app.rs").unwrap();
    assert!(app.contents.contains("Hello from Counter"));
    assert!(!app.contents.contains("__APP_STRUCT__"));
    let codegen =
        core.files.iter().find(|file| file.relative_path == "shared/src/bin/codegen.rs").unwrap();
    assert!(codegen.contents.contains("com.example.counter"));
    assert!(!codegen.contents.contains("__ANDROID_PACKAGE__"));

    let ios_caps = parse_caps(Some("http")).unwrap();
    let ios = plan_ios("Counter", "com.vectis.counter", &ios_caps, &versions).unwrap();
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
    assert!(
        android_http
            .files
            .iter()
            .any(|file| file.relative_path.ends_with("network_security_config.xml"))
    );
    assert_eq!(
        android_http.files.len(),
        android_bare.files.len() + 1,
        "http adds exactly the network-security config"
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
// android shell root, leaving the prior tree untouched before any directory is
// created.
#[test]
fn scaffold_write_plan_refuses_existing_roots() {
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
