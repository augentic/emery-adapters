//! Android shell bootstrap helpers (`vectis android setup`).

mod setup;

use clap::Subcommand;
use serde_json::Value;
pub use setup::{AndroidSetupArgs, run as run_setup, run_for_shell_dir, setup_exit_code};

/// Nested targets under `vectis android`.
#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum AndroidCommand {
    /// Install the vendored Gradle wrapper when absent.
    Setup(AndroidSetupArgs),
}

/// Dispatch a parsed [`AndroidCommand`].
///
/// # Errors
///
/// Returns [`crate::VectisError::InvalidProject`] when the project root or
/// `Android/` shell directory cannot be resolved.
pub fn run(command: &AndroidCommand) -> Result<Value, crate::VectisError> {
    match command {
        AndroidCommand::Setup(args) => run_setup(args),
    }
}

/// Render an android outcome as pretty-printed JSON and exit code.
#[must_use]
pub fn render_json(outcome: Result<Value, crate::VectisError>) -> (String, u8) {
    setup::render_json(outcome)
}
