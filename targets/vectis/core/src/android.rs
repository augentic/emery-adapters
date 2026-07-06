//! Android shell bootstrap — vendored Gradle wrapper installation,
//! absorbed from the legacy extension's `android setup` subcommand
//! (RFC-61 Step 5 Milestone A1).
//!
//! Host-specific files (`local.properties`, `org.gradle.java.home`, NDK pin)
//! are written by the Android Makefile `setup-host` target; the guest only
//! sees the mounted project tree and cannot read `$ANDROID_HOME`.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::VectisError;

const GRADLEW_BYTES: &[u8] = include_bytes!("../assets/android/gradle-wrapper/gradlew");
const GRADLEW_BAT_BYTES: &[u8] = include_bytes!("../assets/android/gradle-wrapper/gradlew.bat");
const WRAPPER_JAR_BYTES: &[u8] =
    include_bytes!("../assets/android/gradle-wrapper/gradle/wrapper/gradle-wrapper.jar");
const WRAPPER_PROPERTIES: &str =
    include_str!("../assets/android/gradle-wrapper/gradle/wrapper/gradle-wrapper.properties");

/// Install the vendored Gradle wrapper for the project's `Android/` shell.
///
/// # Errors
///
/// Returns [`VectisError::InvalidProject`] when the `Android/` shell
/// directory does not exist under `project_root`.
pub fn setup(project_root: &Path) -> Result<Value, VectisError> {
    let android_dir = android_shell_dir(project_root)?;
    Ok(run_for_shell_dir(&android_dir))
}

/// Install the vendored Gradle wrapper under `android_dir` when absent.
///
/// Idempotent: never overwrites an existing wrapper tree.
#[must_use]
pub fn run_for_shell_dir(android_dir: &Path) -> Value {
    let mut actions: Vec<Value> = Vec::new();
    let mut findings: Vec<Value> = Vec::new();

    match install_wrapper(android_dir) {
        Ok(installed) => {
            if installed.is_empty() {
                actions.push(json!({
                    "kind": "gradle-wrapper",
                    "status": "skipped",
                    "reason": "wrapper already present",
                }));
            } else {
                for path in &installed {
                    actions.push(json!({
                        "kind": "gradle-wrapper",
                        "status": "installed",
                        "path": path,
                    }));
                }
            }
        }
        Err(message) => {
            findings.push(json!({
                "id": "android-setup-wrapper-failed",
                "severity": "error",
                "source": "deterministic",
                "message": message,
            }));
        }
    }

    json!({
        "command": "android setup",
        "android-dir": android_dir.display().to_string(),
        "actions": actions,
        "findings": findings,
    })
}

/// Returns 1 when any error-severity finding is present.
#[must_use]
pub fn setup_exit_code(value: &Value) -> u8 {
    let has_error = value.get("findings").and_then(Value::as_array).is_some_and(|arr| {
        arr.iter().any(|f| f.get("severity").and_then(Value::as_str) == Some("error"))
    });
    u8::from(has_error)
}

fn android_shell_dir(project_root: &Path) -> Result<PathBuf, VectisError> {
    let android_dir = project_root.join("Android");
    if !android_dir.is_dir() {
        return Err(VectisError::InvalidProject {
            message: format!("Android shell directory not found at {}", android_dir.display()),
        });
    }
    Ok(android_dir)
}

fn install_wrapper(android_dir: &Path) -> Result<Vec<String>, String> {
    if wrapper_complete(android_dir) {
        return Ok(Vec::new());
    }
    if wrapper_partial(android_dir) {
        return Err("partial Gradle wrapper detected; remove `gradlew`, `gradlew.bat`, and \
             `gradle/wrapper/` then re-run the Android shell setup"
            .into());
    }

    let mut installed = Vec::new();
    std::fs::create_dir_all(android_dir.join("gradle/wrapper"))
        .map_err(|err| format!("failed to create gradle/wrapper: {err}"))?;

    let gradlew = android_dir.join("gradlew");
    write_bytes(&gradlew, GRADLEW_BYTES)?;
    installed.push(relative_path(android_dir, &gradlew));

    let gradlew_bat = android_dir.join("gradlew.bat");
    write_bytes(&gradlew_bat, GRADLEW_BAT_BYTES)?;
    installed.push(relative_path(android_dir, &gradlew_bat));

    let wrapper_jar = android_dir.join("gradle/wrapper/gradle-wrapper.jar");
    write_bytes(&wrapper_jar, WRAPPER_JAR_BYTES)?;
    installed.push(relative_path(android_dir, &wrapper_jar));

    let properties_path = android_dir.join("gradle/wrapper/gradle-wrapper.properties");
    std::fs::write(&properties_path, WRAPPER_PROPERTIES)
        .map_err(|err| format!("failed to write gradle-wrapper.properties: {err}"))?;
    installed.push(relative_path(android_dir, &properties_path));

    set_executable(&gradlew);

    Ok(installed)
}

fn wrapper_paths(android_dir: &Path) -> [PathBuf; 4] {
    [
        android_dir.join("gradlew"),
        android_dir.join("gradlew.bat"),
        android_dir.join("gradle/wrapper/gradle-wrapper.jar"),
        android_dir.join("gradle/wrapper/gradle-wrapper.properties"),
    ]
}

fn wrapper_complete(android_dir: &Path) -> bool {
    wrapper_paths(android_dir).iter().all(|path| path.is_file())
}

fn wrapper_partial(android_dir: &Path) -> bool {
    let present = wrapper_paths(android_dir).iter().filter(|path| path.is_file()).count();
    present > 0 && !wrapper_complete(android_dir)
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = std::fs::File::create(path)
        .map_err(|err| format!("failed to write {}: {err}", path.display()))?;
    file.write_all(bytes).map_err(|err| format!("failed to write {}: {err}", path.display()))
}

fn relative_path(base: &Path, path: &Path) -> String {
    path.strip_prefix(base).map_or_else(|_| path.display().to_string(), |p| p.display().to_string())
}

#[cfg(unix)]
fn set_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    if let Ok(meta) = std::fs::metadata(path) {
        let mut perms = meta.permissions();
        perms.set_mode(0o755);
        let _unused = std::fs::set_permissions(path, perms);
    }
}

#[cfg(not(unix))]
const fn set_executable(_path: &Path) {}
