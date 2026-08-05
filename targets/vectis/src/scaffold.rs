//! Crux project scaffolding helpers.
//!
//! [`materialize`] is the allowlisted local-`vectis-exemplar` copy contract
//! used by build agents for greenfield trees. Pins and structure live only
//! in `$TEMPLATE_DIR` — this crate no longer embeds a template corpus or
//! version registry.

pub mod allowlist;
pub mod materialize;

/// Alias for the crate-wide error type used by scaffold-side callers.
pub use crate::VectisError as ScaffoldError;

/// Compute the default Android package: `com.vectis.<lower app name>`.
#[must_use]
pub fn default_android_package(app_name: &str) -> String {
    format!("com.vectis.{}", app_name.to_lowercase())
}

/// Validate `app_name` as `PascalCase` ASCII.
///
/// # Errors
///
/// Returns [`ScaffoldError`] when the app name cannot be used as a generated
/// Rust/Swift/Kotlin identifier segment.
pub fn validate_app_name(app_name: &str) -> Result<(), ScaffoldError> {
    let mut chars = app_name.chars();
    let first = chars.next().ok_or_else(|| ScaffoldError::InvalidProject {
        message: "app name must not be empty".into(),
    })?;
    if !first.is_ascii_uppercase() {
        return Err(ScaffoldError::InvalidProject {
            message: format!(
                "app name {app_name:?} must start with an ASCII uppercase letter (PascalCase, e.g. \"Counter\")"
            ),
        });
    }
    for c in chars {
        if !c.is_ascii_alphanumeric() {
            return Err(ScaffoldError::InvalidProject {
                message: format!(
                    "app name {app_name:?} must contain only ASCII alphanumeric characters (PascalCase)"
                ),
            });
        }
    }
    Ok(())
}
