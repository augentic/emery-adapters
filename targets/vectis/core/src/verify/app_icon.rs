//! Bootstrap `app-icon` gate, run in-guest as part of the build prelude
//! (RFC-46 §6).
//!
//! `project.yaml.platforms` is the authority for platform intent: every
//! declared UI platform (`ios` / `android`) must carry a satisfiable
//! launcher `app-icon` by build time. A shell that already ships a
//! resident launcher icon (§6.3 escape hatch) satisfies the gate;
//! otherwise `design-system/assets.yaml` must carry a materializable
//! `source:` master (path A) or an operator-pinned export tree via
//! `sources.<platform>` (path B), per §4.1.

use std::path::Path;

use serde_json::{Value, json};

use crate::shell::shell_resident_app_icon;
use crate::validate::engine::parse_yaml_file;

/// Stable finding id for the bootstrap `app-icon` gate (RFC §6.2).
const BOOTSTRAP_APP_ICON_MISSING: &str = "plan-bootstrap-app-icon-missing";

const ASSETS_REL: &str = "design-system/assets.yaml";

/// UI platform tokens that can trigger the §6 launcher-icon gate.
const UI_PLATFORMS: &[&str] = &["ios", "android"];

/// Emit `plan-bootstrap-app-icon-missing` findings for every declared UI
/// platform whose launcher `app-icon` is neither shell-resident (§6.3)
/// nor satisfiable from `design-system/assets.yaml` (§4.1 path A / B).
#[must_use]
pub fn bootstrap_app_icon_findings(
    project_root: &Path, declared_platforms: &[String],
) -> Vec<Value> {
    let mut findings = Vec::new();
    for platform in declared_platforms {
        if !UI_PLATFORMS.contains(&platform.as_str()) {
            continue;
        }
        if shell_resident_app_icon(project_root, platform) {
            continue;
        }
        if let Some(message) = unsatisfied_reason(project_root, platform) {
            findings.push(json!({
                "id": BOOTSTRAP_APP_ICON_MISSING,
                "severity": "error",
                "source": "deterministic",
                "message": message,
            }));
        }
    }
    findings
}

fn unsatisfied_reason(project_root: &Path, platform: &str) -> Option<String> {
    let assets_path = project_root.join(ASSETS_REL);
    if !assets_path.is_file() {
        return Some(missing_message(platform, "`design-system/assets.yaml` is absent"));
    }
    let Some(doc) = parse_yaml_file(&assets_path) else {
        return Some(missing_message(
            platform,
            "`design-system/assets.yaml` could not be read as valid YAML",
        ));
    };
    let assets_dir = assets_path.parent().unwrap_or_else(|| Path::new("."));

    let Some(pointer) = doc.get("app-icon").and_then(Value::as_str) else {
        return Some(missing_message(
            platform,
            "top-level `app-icon` is absent from `design-system/assets.yaml`",
        ));
    };
    let Some(assets) = doc.get("assets").and_then(Value::as_object) else {
        return Some(missing_message(platform, "`design-system/assets.yaml` has no `assets:` map"));
    };
    let Some(entry) = assets.get(pointer) else {
        return Some(missing_message(
            platform,
            &format!(
                "top-level `app-icon` references unknown asset id `{pointer}` under `assets:`"
            ),
        ));
    };
    if entry.get("role").and_then(Value::as_str) != Some("app-icon") {
        return Some(missing_message(
            platform,
            &format!(
                "asset `{pointer}` referenced by top-level `app-icon` must have `role: app-icon`"
            ),
        ));
    }

    if platform_satisfied(assets_dir, entry, platform) {
        None
    } else {
        Some(missing_message(
            platform,
            "neither path A (materializable `source:` master on disk) nor path B \
             (operator-pinned export tree via `sources.<platform>`) satisfies RFC §4.1",
        ))
    }
}

fn platform_satisfied(assets_dir: &Path, entry: &Value, platform: &str) -> bool {
    if let Some(pin) = entry.get("sources").and_then(|s| s.get(platform)).and_then(Value::as_str)
        && pin_resolves(assets_dir, pin)
    {
        return true;
    }
    if let Some(source) = entry.get("source").and_then(Value::as_str)
        && source_materializable(assets_dir, entry, source)
    {
        return true;
    }
    false
}

fn pin_resolves(assets_dir: &Path, path_rel: &str) -> bool {
    let resolved = assets_dir.join(path_rel);
    resolved.is_dir() || resolved.is_file()
}

fn source_materializable(assets_dir: &Path, entry: &Value, source: &str) -> bool {
    if !assets_dir.join(source).is_file() {
        return false;
    }
    let kind = entry.get("kind").and_then(Value::as_str);
    let ext = Path::new(source).extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
    matches!(
        (kind, ext.as_deref()),
        (Some("vector"), Some("svg")) | (Some("raster"), Some("png" | "jpg" | "jpeg" | "webp"))
    )
}

fn missing_message(platform: &str, detail: &str) -> String {
    format!(
        "UI platform bootstrap requires a satisfiable `app-icon` for `{platform}` (RFC §6.2): \
         {detail}; provide path A (`source:` with a materializable SVG or square raster master) \
         or path B (operator-pinned export tree under `exports/{platform}/app-icon/` with \
         `sources.{platform}` pointing at the export root), or satisfy the shell-resident \
         launcher icon escape hatch (§6.3)"
    )
}
