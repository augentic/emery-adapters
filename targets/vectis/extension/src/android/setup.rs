//! `vectis android setup` — vendored Gradle wrapper installation.
//!
//! Host-specific files (`local.properties`, `org.gradle.java.home`, NDK pin)
//! are written by the Android Makefile `setup-host` target; WASI only
//! forwards `PROJECT_DIR` / `CAPABILITY_DIR` and cannot read `$ANDROID_HOME`.

use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Args as ClapArgs;
use serde_json::{Value, json};

use crate::validate::find_project_root;
use crate::{VectisError, render_json as render_value};

const GRADLEW_BYTES: &[u8] = include_bytes!("../../assets/android/gradle-wrapper/gradlew");
const GRADLEW_BAT_BYTES: &[u8] = include_bytes!("../../assets/android/gradle-wrapper/gradlew.bat");
const WRAPPER_JAR_BYTES: &[u8] =
    include_bytes!("../../assets/android/gradle-wrapper/gradle/wrapper/gradle-wrapper.jar");
const WRAPPER_PROPERTIES: &str =
    include_str!("../../assets/android/gradle-wrapper/gradle/wrapper/gradle-wrapper.properties");

/// Arguments for `vectis android setup`.
#[derive(ClapArgs, Debug, Clone, PartialEq, Eq)]
pub struct AndroidSetupArgs {
    /// Project directory. Falls back to `PROJECT_DIR` env, then CWD walk-up.
    pub path: Option<PathBuf>,
}

/// Dispatch `vectis android setup`.
///
/// # Errors
///
/// Returns [`VectisError::InvalidProject`] when the project root or `Android/`
/// shell directory cannot be resolved.
pub fn run(args: &AndroidSetupArgs) -> Result<Value, VectisError> {
    let project_root = resolve_project_root(args.path.as_deref())?;
    let android_dir = android_shell_dir(&project_root)?;
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

/// Render a setup outcome as pretty-printed JSON and exit code.
#[must_use]
pub fn render_json(outcome: Result<Value, VectisError>) -> (String, u8) {
    match outcome {
        Ok(value) => {
            let code = setup_exit_code(&value);
            (render_value(&value), code)
        }
        Err(err) => {
            let exit_code = err.exit_code();
            let Value::Object(mut payload) = err.to_json() else {
                unreachable!("VectisError::to_json always returns an object")
            };
            payload.entry("exit-code".to_string()).or_insert(Value::from(exit_code));
            (render_value(&Value::Object(payload)), exit_code)
        }
    }
}

/// Returns 1 when any error-severity finding is present.
#[must_use]
pub fn setup_exit_code(value: &Value) -> u8 {
    let has_error = value.get("findings").and_then(Value::as_array).is_some_and(|arr| {
        arr.iter().any(|f| f.get("severity").and_then(Value::as_str) == Some("error"))
    });
    u8::from(has_error)
}

fn resolve_project_root(path: Option<&Path>) -> Result<PathBuf, VectisError> {
    if let Some(p) = path {
        return Ok(p.to_path_buf());
    }
    if let Some(project_dir) = std::env::var_os("PROJECT_DIR").filter(|v| !v.is_empty()) {
        return Ok(PathBuf::from(project_dir));
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    find_project_root(&cwd).ok_or_else(|| VectisError::InvalidProject {
        message: "cannot locate project root (no .specify/ directory found)".into(),
    })
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
    let gradlew = android_dir.join("gradlew");
    let wrapper_jar = android_dir.join("gradle/wrapper/gradle-wrapper.jar");
    if gradlew.is_file() && wrapper_jar.is_file() {
        return Ok(Vec::new());
    }
    if gradlew.is_file() || wrapper_jar.is_file() {
        return Err(
            "partial Gradle wrapper detected; remove `gradlew` and `gradle/wrapper/` then re-run \
             `vectis android setup`"
                .into(),
        );
    }

    let mut installed = Vec::new();
    std::fs::create_dir_all(android_dir.join("gradle/wrapper"))
        .map_err(|err| format!("failed to create gradle/wrapper: {err}"))?;

    write_bytes(&gradlew, GRADLEW_BYTES)?;
    installed.push(relative_path(android_dir, &gradlew));

    let gradlew_bat = android_dir.join("gradlew.bat");
    write_bytes(&gradlew_bat, GRADLEW_BAT_BYTES)?;
    installed.push(relative_path(android_dir, &gradlew_bat));

    write_bytes(&wrapper_jar, WRAPPER_JAR_BYTES)?;
    installed.push(relative_path(android_dir, &wrapper_jar));

    let properties_path = android_dir.join("gradle/wrapper/gradle-wrapper.properties");
    std::fs::write(&properties_path, WRAPPER_PROPERTIES)
        .map_err(|err| format!("failed to write gradle-wrapper.properties: {err}"))?;
    installed.push(relative_path(android_dir, &properties_path));

    set_executable(&gradlew);

    Ok(installed)
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
fn set_executable(_path: &Path) {}
