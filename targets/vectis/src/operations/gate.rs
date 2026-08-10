//! Deterministic gates: the composition validator projection, the
//! bootstrap app-icon gate, and the merge-phase report gate (the one
//! surface outside the RFC-90 build loop that still folds a bounded
//! repair leg).

use std::path::Path;

use adapter::seam::{Context, Error, Finding, Report};
use adapter::{Model, phase};
use serde_json::Value;

use crate::{validate, verify};

/// Postflight merge gate: re-run the deterministic composition
/// validator over the merged baseline after the judgment leg, feed a
/// rejection back once, then enforce the residual findings.
pub(super) async fn merge_gate<P: Model>(
    model: &P, ctx: &Context<'_>, prompt: &str, mut report: Report, composition: &Path,
) -> Result<Report, Error> {
    let mut residual = validation_findings(composition);
    if !residual.is_empty() {
        let user = format!(
            "The deterministic report gate rejected the merge-postflight report:\n\n{}\n\n\
             Repair the workspace (or correct the report), then answer with the \
             corrected report body.",
            residual.join("\n"),
        );
        report = phase::report(model, ctx, prompt.to_string(), user).await?;
        residual = validation_findings(composition);
    }
    let findings = residual.into_iter().map(Finding::blocking).collect();
    Ok(phase::enforce(report, findings))
}

pub(super) fn bootstrap_findings(change_root: &Path, code_root: &Path) -> Vec<String> {
    if !change_root.join(".emery/project.yaml").exists() {
        return Vec::new();
    }
    match verify::run(verify::VerifyMode::BootstrapAppIcon, change_root, code_root, None) {
        Ok(payload) => payload
            .get("findings")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|f| f.get("severity").and_then(Value::as_str) == Some("error"))
            .map(|f| {
                format!(
                    "[bootstrap-app-icon] {}: {}",
                    f.get("id").and_then(Value::as_str).unwrap_or("finding"),
                    f.get("message").and_then(Value::as_str).unwrap_or(""),
                )
            })
            .collect(),
        Err(err) => vec![format!("[bootstrap-app-icon] {err}")],
    }
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
