//! Deterministic in-guest results projected onto the RFC-90
//! `PhaseFinding` shape: the validator / verify-gate blocking
//! violations and the suggestion-grade UI-surface coherence reviews.

use std::path::Path;

use adapter::seam::{
    DiagnosticSource, FindingArtifact, FindingEvidence, FindingKind, PhaseFinding, Severity,
    UiSurface,
};
use serde_json::Value;

use crate::verify;

/// One blocking deterministic violation — the shape the in-guest
/// validators contribute to a phase report. The engine renumbers ids
/// and recomputes fingerprints.
pub(super) fn violation(
    detail: String, artifact: FindingArtifact, remediation: &str,
) -> PhaseFinding {
    PhaseFinding {
        id: String::new(),
        rule_id: None,
        related_rule_ids: Vec::new(),
        title: detail.clone(),
        severity: Severity::Important,
        source: DiagnosticSource::Deterministic,
        kind: FindingKind::Violation,
        artifact,
        location: None,
        evidence: FindingEvidence::Snippet { value: detail },
        impact: "a deterministic in-guest vectis check failed; the candidate cannot be accepted \
                 while it stands"
            .to_string(),
        remediation: remediation.to_string(),
        confidence: None,
        fingerprint: String::new(),
    }
}

/// One suggestion-grade deterministic review finding (never blocks).
fn review(rule_id: &str, detail: String) -> PhaseFinding {
    PhaseFinding {
        id: String::new(),
        rule_id: Some(rule_id.to_string()),
        related_rule_ids: Vec::new(),
        title: detail.clone(),
        severity: Severity::Suggestion,
        source: DiagnosticSource::Deterministic,
        kind: FindingKind::Review,
        artifact: FindingArtifact::Composition,
        location: None,
        evidence: FindingEvidence::Snippet { value: detail },
        impact: "the UI-surface judgement and the produced composition disagree; operator \
                 attention is suggested"
            .to_string(),
        remediation: "align `ui-surface.screens` with the produced composition on the next build"
            .to_string(),
        confidence: None,
        fingerprint: String::new(),
    }
}

/// Composition-validator blocking findings for one candidate
/// `composition.yaml`. `ran` is true when the validator actually
/// executed (the file exists); an absent composition is clean
/// (core-only / pre-first-merge) and contributes no assurance.
pub(super) fn composition_findings(composition: &Path) -> (bool, Vec<PhaseFinding>) {
    if !composition.exists() {
        return (false, Vec::new());
    }
    let findings = super::gate::validation_findings(composition)
        .into_iter()
        .map(|detail| {
            violation(
                detail.trim_start_matches("- ").to_string(),
                FindingArtifact::Composition,
                "repair the candidate composition (or the operator-curated manifests it \
                 references) so the deterministic composition validator passes",
            )
        })
        .collect();
    (true, findings)
}

/// Deterministic shell-verify gate findings over the lent workspace.
///
/// `ran` is false when the project declares no platform set
/// (`.emery/project.yaml` absent) — the gate is skipped and
/// contributes no assurance. Error-severity findings block; an
/// unrunnable gate is itself a blocking violation.
pub(super) fn shell_verify_findings(
    change_root: &Path, code_root: &Path, slice_composition: Option<&Path>,
) -> (bool, Vec<PhaseFinding>) {
    if !change_root.join(".emery/project.yaml").exists() {
        return (false, Vec::new());
    }
    let findings =
        match verify::run(verify::VerifyMode::Verify, change_root, code_root, slice_composition) {
            Ok(payload) => payload
                .get("findings")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter(|f| f.get("severity").and_then(Value::as_str) == Some("error"))
                .map(|f| {
                    violation(
                        format!(
                            "[{}] {}",
                            f.get("id").and_then(Value::as_str).unwrap_or("shell-verify"),
                            f.get("message").and_then(Value::as_str).unwrap_or_default(),
                        ),
                        FindingArtifact::Code,
                        "repair the workspace so the deterministic vectis shell verify gate passes",
                    )
                })
                .collect(),
            Err(err) => vec![violation(
                format!("[shell-verify] {err}"),
                FindingArtifact::Code,
                "fix the project configuration so the deterministic verify gate can run",
            )],
        };
    (true, findings)
}

/// A4 self-consistency only (`suggestion`); never fails the report.
pub(super) fn ui_surface_coherence(
    ui_surface: Option<UiSurface>, composition: &Path,
) -> Vec<PhaseFinding> {
    let Some(ui_surface) = ui_surface else {
        return Vec::new();
    };
    let has_surface = composition_declares_surface(composition);
    let mut warnings = Vec::new();
    if ui_surface.screens == 0 && has_surface {
        warnings.push(review(
            "composition-unexpected-for-non-ui-slice",
            "the report claims `ui-surface.screens: 0` but produced a non-empty \
             composition.yaml; the UI-surface judgement contradicts the composition output"
                .to_string(),
        ));
    }
    if ui_surface.screens > 0 && !has_surface {
        warnings.push(review(
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
