//! Android toolchain + compile-artifact probes for `verify --mode verify`.
//!
//! Compilation is driven by `make verify` in the Android shell; this module
//! checks that host-side setup artifacts and the debug APK are present on disk.

use std::path::Path;

use serde_json::{Value, json};

const APK_REL: &str = "app/build/outputs/apk/debug/app-debug.apk";

/// Emit Android toolchain findings when `android` is declared and the shell
/// tree is present.
#[must_use]
pub fn android_toolchain_findings(
    project_root: &Path, android_declared: bool, android_present: bool,
) -> Vec<Value> {
    if !android_declared || !android_present {
        return Vec::new();
    }

    let android_dir = project_root.join("Android");
    let mut findings = Vec::new();

    let gradlew = android_dir.join("gradlew");
    if !gradlew.is_file() {
        findings.push(error_finding(
            "android-gradlew-missing",
            "Android shell is missing `gradlew`; run `make setup` or `vectis android setup`",
        ));
    } else if !is_executable(&gradlew) {
        findings.push(error_finding(
            "android-gradlew-not-executable",
            "`gradlew` exists but is not executable; run `chmod +x Android/gradlew`",
        ));
    }

    let wrapper_jar = android_dir.join("gradle/wrapper/gradle-wrapper.jar");
    if !wrapper_jar.is_file() {
        findings.push(error_finding(
            "android-wrapper-jar-missing",
            "Android shell is missing `gradle/wrapper/gradle-wrapper.jar`",
        ));
    }

    let wrapper_properties = android_dir.join("gradle/wrapper/gradle-wrapper.properties");
    if !wrapper_properties.is_file() {
        findings.push(error_finding(
            "android-wrapper-properties-missing",
            "Android shell is missing `gradle/wrapper/gradle-wrapper.properties`",
        ));
    }

    let local_properties = android_dir.join("local.properties");
    if !local_properties.is_file() {
        findings.push(error_finding(
            "android-local-properties-missing",
            "Android shell is missing `local.properties`; run `make setup-host` with `ANDROID_HOME` set",
        ));
    } else if !file_contains(&local_properties, "sdk.dir") {
        findings.push(error_finding(
            "android-local-properties-missing",
            "`local.properties` exists but has no `sdk.dir` entry; run `make setup-host`",
        ));
    }

    let gradle_properties = android_dir.join("gradle.properties");
    if gradle_properties.is_file() && !file_contains(&gradle_properties, "org.gradle.java.home") {
        findings.push(info_finding(
            "android-java-home-unpinned",
            "`gradle.properties` has no `org.gradle.java.home`; pin Java 21 via `make setup-host`",
        ));
    }

    let shared_build = android_dir.join("shared/build.gradle.kts");
    if shared_build.is_file() && file_contains(&shared_build, "__ANDROID_NDK_VERSION__") {
        findings.push(info_finding(
            "android-ndk-unsubstituted",
            "`shared/build.gradle.kts` still contains `__ANDROID_NDK_VERSION__`; run `make setup-host`",
        ));
    }

    let apk = android_dir.join(APK_REL);
    if !apk.is_file() {
        findings.push(error_finding(
            "android-apk-missing",
            format!(
                "debug APK not found at `Android/{APK_REL}`; run `make verify` in the Android shell"
            ),
        ));
    }

    findings
}

fn error_finding(id: &str, message: impl Into<String>) -> Value {
    json!({
        "id": id,
        "severity": "error",
        "source": "deterministic",
        "message": message.into(),
    })
}

fn info_finding(id: &str, message: impl Into<String>) -> Value {
    json!({
        "id": id,
        "severity": "info",
        "source": "deterministic",
        "message": message.into(),
    })
}

fn file_contains(path: &Path, needle: &str) -> bool {
    std::fs::read_to_string(path).ok().is_some_and(|content| content.contains(needle))
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    std::fs::metadata(path).ok().is_some_and(|meta| meta.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}
