//! Deterministic pre-write blocks rendered into build-leg prompts:
//! the in-guest prepare / component-inference / shell-verify summaries
//! and the template-materialize prelude for absent declared trees.

use std::path::Path;

use serde_json::Value;

use super::shells::declared_shell_legs;
use crate::{android_scaffold, infer, ios_scaffold, scaffold, shell, verify};

pub(super) fn render_prelude(summary: &Value) -> String {
    format!(
        "### prepare prelude (already run in-guest)\n\n\
         The adapter resolved the slice's materialize scope and ran the deterministic \
         `materialize assets` step before this leg; do not re-run it. Summary:\n\n{}",
        serde_json::to_string(summary).unwrap_or_else(|_| "{}".to_string()),
    )
}

pub(super) fn render_infer_report(change_root: &Path) -> String {
    let composition = change_root.join(".emery/specs/composition.yaml");
    let report = if composition.exists() {
        let args = infer::InferArgs {
            composition,
            candidate_cache: Some(change_root.join(".emery/.cache/component-candidates"))
                .filter(|p| p.is_dir()),
            parts: Some(change_root.join(".emery/design-system/parts.yaml"))
                .filter(|p| p.is_file()),
            min_occurrences: infer::DEFAULT_MIN_OCCURRENCES,
        };
        match infer::run(&args) {
            Ok(payload) => serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string()),
            Err(err) => format!("inference could not run: {err}"),
        }
    } else {
        "no merged baseline yet — empty report (nothing to name)".to_string()
    };
    format!("### component-identity cluster report (already run in-guest)\n\n{report}")
}

pub(super) fn render_verify_gate(change_root: &Path, code_root: &Path) -> String {
    let body = if change_root.join(".emery/project.yaml").exists() {
        match verify::run(verify::VerifyMode::Verify, change_root, code_root) {
            Ok(payload) => serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string()),
            Err(err) => format!("verify gate could not run: {err}"),
        }
    } else {
        "no declared platform set (`.emery/project.yaml` absent) — gate skipped".to_string()
    };
    format!("### shell verify gate (already run in-guest)\n\n{body}")
}

pub(super) fn scaffold_missing_trees(change_root: &Path, code_root: &Path) -> String {
    let mut targets: Vec<&'static str> = Vec::new();
    if !shell::shell_present(code_root, "core") {
        targets.push("core");
    }
    for leg in declared_shell_legs(change_root) {
        if !shell::shell_present(code_root, leg.name) {
            targets.push(leg.name);
        }
    }
    if targets.is_empty() {
        return "### template-materialize prelude\n\nAll declared trees were already present; \
                skip greenfield materialize. For DX pin drift, re-copy the drifted paths from \
                `$TEMPLATE_DIR` with identity substitution — never invent versions."
            .to_string();
    }
    let identity = resolve_scaffold_app_name(change_root, code_root).map_or_else(
        || {
            "- Resolve `app_name` (PascalCase from `design.md` `App` / `project.yaml` \
             `name:`) and `android_package` before materialize; refuse to invent them."
                .to_string()
        },
        |app_name| {
            let package = scaffold::default_android_package(&app_name);
            format!(
                "- Suggested identity: app_name=`{app_name}`, android_package=`{package}` \
                 (override the package from `design.md` when it declares one)."
            )
        },
    );
    let absent = targets.iter().map(|t| format!("`{t}`")).collect::<Vec<_>>().join(", ");
    format!(
        "### template-materialize prelude\n\nAbsent declared trees: {absent}. The guest did \
         **not** write them — target guests cannot see a sibling `$TEMPLATE_DIR`.\n\n\
         Before any write leg for those trees:\n\
         1. Resolve `$TEMPLATE_DIR` (`VECTIS_EXEMPLAR_DIR` or `../vectis-exemplar`); fail \
         closed if missing.\n\
         2. Run the allowlisted copy procedure in `references/template-materialize.md` \
         (root DX + `shared/` + `iOS/` + `Android/` + `supply-chain/` + `.maestro/`; \
         never `web/`, `.git/`, `.github/`, or `AGENTS.md`). One materialize covers the \
         workspace — do not invent per-shell scaffolds or pins.\n\
         3. Strip `VECTIS-OPTIONAL` per `$TEMPLATE_DIR/AGENTS.md` against the \
         `design.md` capability matrix (`http` / `kv` / `time` / `sse` / `demo`).\n\
         4. iOS: regenerate the Xcode project (`make -C iOS generate-project` / \
         `xcodegen`) — `.xcodeproj` is denylisted on purpose.\n\
         {identity}"
    )
}

fn resolve_scaffold_app_name(change_root: &Path, code_root: &Path) -> Option<String> {
    if let Ok(name) = ios_scaffold::resolve_ios_app_name(code_root) {
        return Some(name);
    }
    if let Ok(name) = android_scaffold::resolve_android_app_name(code_root) {
        return Some(name);
    }
    let source = std::fs::read_to_string(change_root.join(".emery/project.yaml")).ok()?;
    let doc: Value = serde_saphyr::from_str(&source).ok()?;
    let raw = doc.get("name")?.as_str()?;
    let pascal: String = raw
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            chars.next().map_or_else(String::new, |first| {
                let mut word = first.to_ascii_uppercase().to_string();
                word.push_str(chars.as_str());
                word
            })
        })
        .collect();
    scaffold::validate_app_name(&pascal).ok().map(|()| pascal)
}
