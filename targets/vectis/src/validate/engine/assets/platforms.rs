//! Resolve targeted shell platforms from `.emery/project.yaml`.

use std::path::Path;

use serde_json::Value;

const ASSET_SHELL_PLATFORMS: &[&str] = &["ios", "android"];

/// Load the `ios` / `android` entries declared in
/// `project.yaml.platforms`.
///
/// Fails closed (A15): a missing or unreadable declaration is an error
/// the caller surfaces as a blocking finding — never a guessed
/// both-shells set. A declared set with no shell members (core-only)
/// is the legal empty scope.
///
/// # Errors
///
/// One human-readable detail line when `project.yaml` is unreadable,
/// unparseable, or declares no `platforms` array.
pub fn load_shell_platforms(project_root: &Path) -> Result<Vec<String>, String> {
    let config_path = project_root.join(".emery").join("project.yaml");
    let source = std::fs::read_to_string(&config_path).map_err(|err| {
        format!(
            "platform declaration unreadable at {}: {err}; declare the project platform set \
             with `emery init --upgrade --platforms <csv>`",
            config_path.display()
        )
    })?;
    let doc: Value = serde_saphyr::from_str(&source).map_err(|err| {
        format!("project.yaml at {} is not parseable YAML: {err}", config_path.display())
    })?;
    let Some(platforms) = doc.get("platforms").and_then(Value::as_array) else {
        return Err(format!(
            "project.yaml at {} declares no `platforms` array; vectis requires a declared \
             platform set — run `emery init --upgrade --platforms <csv>`",
            config_path.display()
        ));
    };

    let mut shell: Vec<String> = Vec::new();
    for entry in platforms {
        let Some(name) = entry.as_str() else {
            continue;
        };
        if ASSET_SHELL_PLATFORMS.contains(&name) && !shell.iter().any(|p| p == name) {
            shell.push(name.to_string());
        }
    }
    Ok(shell)
}
