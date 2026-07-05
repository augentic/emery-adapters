//! `vectis scaffold` subcommand surface.
//!
//! The render-only scaffolding engine moved to `specify-vectis-core`
//! (RFC-61 Step 5 Milestone A1); this module keeps the WASI command
//! surface — the clap derive types, `PROJECT_DIR` resolution, and the
//! JSON envelope — and delegates planning and writes to the core.

use std::path::PathBuf;

use clap::{Args as ClapArgs, Subcommand};
pub use specify_vectis_core::scaffold::{
    Capability, PlannedFile, ScaffoldError, ScaffoldPlan, Versions, default_android_package,
    parse_caps, plan_android, plan_command, plan_core, plan_ios, validate_app_name, write_plan,
};

use crate::render_json as render_value;

/// Scaffold targets exposed under `vectis scaffold`.
#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum ScaffoldCommand {
    /// Render the shared Rust Crux core crate.
    Core(CoreArgs),
    /// Render the `SwiftUI` iOS shell.
    Ios(IosArgs),
    /// Render the Jetpack Compose Android shell.
    Android(AndroidArgs),
}

impl ScaffoldCommand {
    /// Project the parsed CLI command onto the core's request shape.
    fn to_core(&self) -> specify_vectis_core::scaffold::ScaffoldCommand {
        match self {
            Self::Core(args) => specify_vectis_core::scaffold::ScaffoldCommand::Core(
                specify_vectis_core::scaffold::CoreArgs {
                    common: args.common.to_core(),
                    android_package: args.android_package.clone(),
                },
            ),
            Self::Ios(args) => specify_vectis_core::scaffold::ScaffoldCommand::Ios(
                specify_vectis_core::scaffold::IosArgs {
                    common: args.common.to_core(),
                },
            ),
            Self::Android(args) => specify_vectis_core::scaffold::ScaffoldCommand::Android(
                specify_vectis_core::scaffold::AndroidArgs {
                    common: args.common.to_core(),
                    android_package: args.android_package.clone(),
                },
            ),
        }
    }
}

/// Arguments for `vectis scaffold core`.
#[derive(ClapArgs, Debug, Clone, PartialEq, Eq)]
pub struct CoreArgs {
    /// Common app, capability, and version arguments.
    #[command(flatten)]
    pub common: CommonArgs,

    /// Android package name used when rendering Android-facing core bindings.
    #[arg(long)]
    pub android_package: Option<String>,
}

/// Arguments for `vectis scaffold ios`.
#[derive(ClapArgs, Debug, Clone, PartialEq, Eq)]
pub struct IosArgs {
    /// Common app, capability, and version arguments.
    #[command(flatten)]
    pub common: CommonArgs,
}

/// Arguments for `vectis scaffold android`.
#[derive(ClapArgs, Debug, Clone, PartialEq, Eq)]
pub struct AndroidArgs {
    /// Common app, capability, and version arguments.
    #[command(flatten)]
    pub common: CommonArgs,

    /// Android application package name.
    #[arg(long)]
    pub android_package: Option<String>,
}

/// Arguments shared by all scaffold targets.
#[derive(ClapArgs, Debug, Clone, PartialEq, Eq)]
pub struct CommonArgs {
    /// App struct/name to scaffold, for example `Counter` or `TodoApp`.
    pub app_name: String,

    /// Comma-separated capabilities, for example `http,kv,time`.
    #[arg(long)]
    pub caps: Option<String>,

    /// Complete TOML file overriding the embedded version defaults.
    #[arg(long)]
    pub version_file: Option<PathBuf>,
}

impl CommonArgs {
    fn to_core(&self) -> specify_vectis_core::scaffold::CommonArgs {
        specify_vectis_core::scaffold::CommonArgs {
            app_name: self.app_name.clone(),
            caps: self.caps.clone(),
            version_file: self.version_file.clone(),
        }
    }
}

/// Execute a scaffold command against the `PROJECT_DIR` project root.
///
/// # Errors
///
/// Returns [`ScaffoldError`] for invalid inputs, version-file issues,
/// or write failures.
pub fn run(command: &ScaffoldCommand) -> Result<serde_json::Value, ScaffoldError> {
    let project_dir = project_dir_from_env()?;
    specify_vectis_core::scaffold::run_at(&project_dir, &command.to_core())
}

/// Render a scaffold outcome as pretty-printed JSON and exit code.
#[must_use]
pub fn render_json(outcome: Result<serde_json::Value, ScaffoldError>) -> (String, u8) {
    match outcome {
        Ok(value) => {
            let code = specify_vectis_core::scaffold::exit_code(&value);
            (render_value(&value), code)
        }
        Err(err) => {
            let exit_code = err.exit_code();
            let serde_json::Value::Object(mut payload) = err.to_json() else {
                unreachable!("ScaffoldError::to_json always returns an object")
            };
            payload.entry("exit-code".to_string()).or_insert(serde_json::Value::from(exit_code));
            (render_value(&serde_json::Value::Object(payload)), exit_code)
        }
    }
}

fn project_dir_from_env() -> Result<PathBuf, ScaffoldError> {
    std::env::var_os("PROJECT_DIR").map(PathBuf::from).ok_or_else(|| {
        ScaffoldError::InvalidProject {
            message: "PROJECT_DIR is not set; run with a project scope".into(),
        }
    })
}
