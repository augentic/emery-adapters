//! Android scaffold zero-suppression and strict Gradle flag integration tests.

use specify_vectis::scaffold::{ScaffoldPlan, Versions, parse_caps, plan_android};

fn versions() -> Versions {
    Versions::embedded().expect("embedded versions parse")
}

fn plan(caps: Option<&str>) -> ScaffoldPlan {
    let caps = parse_caps(caps).expect("parse caps");
    plan_android("Counter", "com.vectis.counter", &caps, &versions()).expect("plan android")
}

fn kotlin_sources(plan: &ScaffoldPlan) -> impl Iterator<Item = &str> {
    plan.files
        .iter()
        .filter(|file| {
            std::path::Path::new(&file.relative_path)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("kt"))
        })
        .map(|file| file.contents.as_str())
}

fn assert_no_suppress(plan: &ScaffoldPlan) {
    for contents in kotlin_sources(plan) {
        assert!(
            !contents.contains("@Suppress"),
            "Kotlin source must not contain @Suppress:\n{contents}"
        );
    }
}

fn assert_gradle_strict_flags(plan: &ScaffoldPlan) {
    for path in ["Android/app/build.gradle.kts", "Android/shared/build.gradle.kts"] {
        let contents = plan
            .files
            .iter()
            .find(|file| file.relative_path == path)
            .unwrap_or_else(|| panic!("{path} missing from android plan"))
            .contents
            .as_str();
        assert!(
            contents.contains("allWarningsAsErrors = true"),
            "{path} must set allWarningsAsErrors = true:\n{contents}"
        );
        assert!(contents.contains("-Werror"), "{path} must add JavaCompile -Werror:\n{contents}");
    }
}

#[test]
fn android_scaffold_kt_has_no_suppress_render_only() {
    assert_no_suppress(&plan(None));
}

#[test]
fn android_scaffold_kt_has_no_suppress_http_kv_time_platform() {
    assert_no_suppress(&plan(Some("http,kv,time,platform")));
}

#[test]
fn android_scaffold_gradle_treats_warnings_as_errors_render_only() {
    assert_gradle_strict_flags(&plan(None));
}

#[test]
fn android_scaffold_gradle_treats_warnings_as_errors_http() {
    assert_gradle_strict_flags(&plan(Some("http")));
}
