//! Deterministic gates around the judgment legs: the composition
//! validator repair loop, the shared report gate (validator + shell
//! verify + declared outputs), the bootstrap app-icon gate, and the
//! suggestion-only UI-surface coherence check.

use std::path::Path;

use adapter::seam::{Context, Error, Finding, Report, Severity};
use adapter::{Model, phase};
use serde_json::Value;

use super::{REFERENCES_POINTER, assemble};
use crate::{validate, verify};

const MAX_VALIDATE_REPAIR_ITERATIONS: usize = 2;

pub(super) async fn composition_gate<P: Model>(
    model: &P, ctx: &Context<'_>, slice: &str, slice_dir_rel: &str, composition: &Path,
) -> Result<Vec<String>, Error> {
    let mut findings = validation_findings(composition);
    for _ in 0..MAX_VALIDATE_REPAIR_ITERATIONS {
        if findings.is_empty() {
            break;
        }
        let system = assemble(&["prompts/build.md", "prompts/build/composition.md"]);
        let user = format!(
            "The deterministic composition validator found blocking issues in slice \
             `{slice}`'s regenerated `{slice_dir_rel}/composition.yaml`. Repair the \
             composition (or the operator-curated manifests it references) in place per \
             the composition prompt's validator gate.\n\n{}\n\n\
             Answer `applicable: true` with a summary of the repairs. {REFERENCES_POINTER}",
            findings.join("\n"),
        );
        phase::phase(model, ctx, system, user, "composition-repair").await?;
        findings = validation_findings(composition);
    }
    Ok(findings)
}

#[expect(clippy::too_many_arguments, reason = "One internal gate call site per operation.")]
pub(super) async fn gate_report<P: Model>(
    model: &P, ctx: &Context<'_>, prompt: &str, mut report: Report, tree_root: &Path,
    composition: &Path, operation: &str, shell_verify: bool, active_slice: Option<&str>,
) -> Result<Report, Error> {
    let gather = |report: &Report| {
        let mut residual = validation_findings(composition);
        if shell_verify {
            residual.extend(shell_verify_findings(tree_root, active_slice));
        }
        residual.extend(phase::missing_outputs(report, tree_root));
        residual
    };
    let mut residual = gather(&report);
    if !residual.is_empty() {
        let user = format!(
            "The deterministic report gate rejected the {operation} report:\n\n{}\n\n\
             Repair the working tree (or correct the report), then answer with the \
             corrected report body.",
            residual.join("\n"),
        );
        report = phase::report(model, ctx, prompt.to_string(), user).await?;
        residual = gather(&report);
    }
    let findings = residual.into_iter().map(Finding::blocking).collect();
    Ok(phase::enforce(report, findings))
}

pub(super) fn bootstrap_findings(tree_root: &Path) -> Vec<String> {
    if !tree_root.join(".emery/project.yaml").exists() {
        return Vec::new();
    }
    match verify::run(verify::VerifyMode::BootstrapAppIcon, tree_root, None) {
        Ok(payload) => payload
            .get("findings")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|f| f.get("severity").and_then(Value::as_str) == Some("error"))
            .map(|f| {
                format!(
                    "- [bootstrap-app-icon] {}: {}",
                    f.get("id").and_then(Value::as_str).unwrap_or("finding"),
                    f.get("message").and_then(Value::as_str).unwrap_or(""),
                )
            })
            .collect(),
        Err(err) => vec![format!("- [bootstrap-app-icon] {err}")],
    }
}

fn shell_verify_findings(tree_root: &Path, active_slice: Option<&str>) -> Vec<String> {
    if !tree_root.join(".emery/project.yaml").exists() {
        return Vec::new();
    }
    match verify::run(verify::VerifyMode::Verify, tree_root, active_slice) {
        Ok(payload) => payload
            .get("findings")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|f| f.get("severity").and_then(Value::as_str) == Some("error"))
            .map(|f| {
                format!(
                    "- [shell-verify] {}: {}",
                    f.get("id").and_then(Value::as_str).unwrap_or("finding"),
                    f.get("message").and_then(Value::as_str).unwrap_or(""),
                )
            })
            .collect(),
        Err(err) => vec![format!("- [shell-verify] {err}")],
    }
}

// A4 self-consistency only (`suggestion`); never fails the report.
pub(super) fn ui_surface_coherence(report: &Report, composition: &Path) -> Vec<Finding> {
    let Some(ui_surface) = report.ui_surface else {
        return Vec::new();
    };
    let has_surface = composition_declares_surface(composition);
    let mut warnings = Vec::new();
    if ui_surface.screens == 0 && has_surface {
        warnings.push(ui_surface_warning(
            "composition-unexpected-for-non-ui-slice",
            "the report claims `ui-surface.screens: 0` but produced a non-empty \
             composition.yaml; the UI-surface judgement contradicts the composition output"
                .to_string(),
        ));
    }
    if ui_surface.screens > 0 && !has_surface {
        warnings.push(ui_surface_warning(
            "composition-empty-for-ui-slice",
            format!(
                "the report claims `ui-surface.screens: {}` but produced an absent or empty \
                 composition.yaml; the UI-surface judgement contradicts the composition output",
                ui_surface.screens
            ),
        ));
    }
    warnings
}

fn ui_surface_warning(rule_id: &str, detail: String) -> Finding {
    Finding {
        rule_id: Some(rule_id.to_string()),
        severity: Severity::Suggestion,
        detail,
    }
}

fn composition_declares_surface(path: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };
    let Ok(doc) = serde_saphyr::from_str::<Value>(&text) else {
        return false;
    };

    if doc.get("screens").and_then(Value::as_object).is_some_and(|s| !s.is_empty()) {
        return true;
    }

    doc.get("delta").and_then(Value::as_object).is_some_and(|delta| {
        ["added", "modified", "removed"]
            .iter()
            .any(|key| delta.get(*key).and_then(Value::as_object).is_some_and(|m| !m.is_empty()))
    })
}

// Absent composition is clean (core-only / pre-first-merge).
pub(super) fn validation_findings(composition: &Path) -> Vec<String> {
    if !composition.exists() {
        return Vec::new();
    }
    match validate::run(validate::ValidateMode::Composition, Some(composition)) {
        Ok(envelope) => {
            let mut findings = Vec::new();
            collect_envelope_errors(&envelope, "composition", &mut findings);
            findings
        }
        Err(err) => vec![format!("- [composition] {}: {err}", composition.display())],
    }
}

fn collect_envelope_errors(envelope: &Value, mode: &str, findings: &mut Vec<String>) {
    let mode = envelope.get("mode").and_then(Value::as_str).unwrap_or(mode);
    if let Some(errors) = envelope.get("errors").and_then(Value::as_array) {
        for error in errors {
            let path = error.get("path").and_then(Value::as_str).unwrap_or("");
            let message = error.get("message").and_then(Value::as_str).unwrap_or("");
            findings.push(format!("- [{mode}] `{path}`: {message}"));
        }
    }
    if let Some(results) = envelope.get("results").and_then(Value::as_array) {
        for entry in results {
            if let Some(report) = entry.get("report") {
                collect_envelope_errors(report, mode, findings);
            }
        }
    }
}
