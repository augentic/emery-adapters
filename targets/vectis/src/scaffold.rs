//! Render-only Crux project scaffolding.
//!
//! [`materialize`] is the allowlisted local-`vectis-template` copy contract
//! (additive alongside the embedded template renderer until cutover).

pub mod materialize;

mod runtime;
mod templates;
mod versions;

use std::path::{Path, PathBuf};

pub use runtime::{plan_android, plan_core, plan_ios, validate_app_name, write_plan};
pub use templates::Capability;
pub use versions::Versions;

/// Alias for the crate-wide error type used by scaffold-side callers.
pub use crate::VectisError as ScaffoldError;

/// Scaffold targets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScaffoldCommand {
    /// Render the shared Rust Crux core crate.
    Core(CoreArgs),
    /// Render the `SwiftUI` iOS shell.
    Ios(IosArgs),
    /// Render the Jetpack Compose Android shell.
    Android(AndroidArgs),
}

impl ScaffoldCommand {
    /// Return the stable target spelling for this scaffold target.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Core(_) => "core",
            Self::Ios(_) => "ios",
            Self::Android(_) => "android",
        }
    }

    /// Return the app name supplied to this scaffold target.
    #[must_use]
    pub fn app_name(&self) -> &str {
        match self {
            Self::Core(args) => &args.common.app_name,
            Self::Ios(args) => &args.common.app_name,
            Self::Android(args) => &args.common.app_name,
        }
    }

    /// Return common arguments for this command.
    #[must_use]
    pub const fn common(&self) -> &CommonArgs {
        match self {
            Self::Core(args) => &args.common,
            Self::Ios(args) => &args.common,
            Self::Android(args) => &args.common,
        }
    }
}

/// Inputs for the `core` scaffold target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreArgs {
    /// Common app, capability, and version arguments.
    pub common: CommonArgs,

    /// Android package name used when rendering Android-facing core bindings.
    pub android_package: Option<String>,
}

/// Inputs for the `ios` scaffold target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IosArgs {
    /// Common app, capability, and version arguments.
    pub common: CommonArgs,
}

/// Inputs for the `android` scaffold target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AndroidArgs {
    /// Common app, capability, and version arguments.
    pub common: CommonArgs,

    /// Android application package name.
    pub android_package: Option<String>,
}

/// Inputs shared by all scaffold targets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonArgs {
    /// App struct/name to scaffold, for example `Counter` or `TodoApp`.
    pub app_name: String,

    /// Comma-separated capabilities, for example `http,kv,time`.
    pub caps: Option<String>,

    /// Complete TOML file overriding the embedded version defaults.
    pub version_file: Option<PathBuf>,
}

impl CommonArgs {
    /// Common arguments carrying only an app name.
    #[must_use]
    pub const fn for_app(app_name: String) -> Self {
        Self {
            app_name,
            caps: None,
            version_file: None,
        }
    }
}

/// A rendered file ready to write under the project directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedFile {
    /// Relative target path under the project directory.
    pub relative_path: String,
    /// Rendered file bytes as UTF-8 text.
    pub contents: String,
}

/// A complete scaffold plan for one target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScaffoldPlan {
    /// Scaffold target name.
    pub target: &'static str,
    /// App name supplied by the caller.
    pub app_name: String,
    /// Android package used for placeholders.
    pub android_package: String,
    /// Selected capability tags in stable input order.
    pub capabilities: Vec<String>,
    /// Files to write in template declaration order.
    pub files: Vec<PlannedFile>,
}

impl ScaffoldPlan {
    fn file_paths(&self) -> Vec<String> {
        self.files.iter().map(|file| file.relative_path.clone()).collect()
    }

    fn to_json(&self, project_dir: &Path) -> serde_json::Value {
        serde_json::json!({
            "target": self.target,
            "app-name": self.app_name,
            "project-dir": project_dir.display().to_string(),
            "android-package": self.android_package,
            "capabilities": self.capabilities,
            "files": self.file_paths(),
        })
    }
}

/// Execute a scaffold command against an explicit project directory.
///
/// # Errors
/// Returns [`ScaffoldError`] for invalid inputs, version-file issues,
pub fn run_at(
    project_dir: &Path, command: &ScaffoldCommand,
) -> Result<serde_json::Value, ScaffoldError> {
    let versions = Versions::resolve(command.common().version_file.as_deref())?;
    let plan = plan_command(command, &versions)?;
    write_plan(project_dir, &plan)?;
    let mut payload = plan.to_json(project_dir);
    if matches!(command, ScaffoldCommand::Android(_)) {
        let setup = crate::android::run_for_shell_dir(&project_dir.join("Android"));
        if let serde_json::Value::Object(ref mut map) = payload {
            map.insert("android-setup".to_string(), setup);
        }
    }
    Ok(payload)
}

/// Compute the exit code for a scaffold payload: `1` when the chained
/// Android setup surfaced an error finding, `0` otherwise.
#[must_use]
pub fn exit_code(value: &serde_json::Value) -> u8 {
    value.get("android-setup").map_or(0, crate::android::setup_exit_code)
}

/// Plan a scaffold command without touching the filesystem.
///
/// # Errors
/// Returns [`ScaffoldError`] when arguments are invalid.
pub fn plan_command(
    command: &ScaffoldCommand, versions: &Versions,
) -> Result<ScaffoldPlan, ScaffoldError> {
    match command {
        ScaffoldCommand::Core(args) => {
            let caps = parse_caps(args.common.caps.as_deref())?;
            let android_package = args
                .android_package
                .clone()
                .unwrap_or_else(|| default_android_package(&args.common.app_name));
            plan_core(&args.common.app_name, &android_package, &caps, versions)
        }
        ScaffoldCommand::Ios(args) => {
            let caps = parse_caps(args.common.caps.as_deref())?;
            let android_package = default_android_package(&args.common.app_name);
            plan_ios(&args.common.app_name, &android_package, &caps, versions)
        }
        ScaffoldCommand::Android(args) => {
            let caps = parse_caps(args.common.caps.as_deref())?;
            let android_package = args
                .android_package
                .clone()
                .unwrap_or_else(|| default_android_package(&args.common.app_name));
            plan_android(&args.common.app_name, &android_package, &caps, versions)
        }
    }
}

/// Compute the default Android package: `com.vectis.<lower app name>`.
#[must_use]
pub fn default_android_package(app_name: &str) -> String {
    format!("com.vectis.{}", app_name.to_lowercase())
}

/// Parse a comma-separated capability list into the canonical set.
///
/// # Errors
/// Returns [`ScaffoldError`] when an unknown capability tag is present.
pub fn parse_caps(raw: Option<&str>) -> Result<Vec<Capability>, ScaffoldError> {
    let mut out: Vec<Capability> = Vec::new();
    let Some(raw) = raw else { return Ok(out) };
    for tag in raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let cap = Capability::from_tag(tag).ok_or_else(|| ScaffoldError::InvalidProject {
            message: format!(
                "unknown capability: {tag:?} (expected one of: http, kv, time, platform, sse)"
            ),
        })?;
        if !out.contains(&cap) {
            out.push(cap);
        }
    }
    Ok(out)
}
