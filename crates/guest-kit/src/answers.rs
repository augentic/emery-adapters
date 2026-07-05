//! Judgment-answer schemas and deserialization.
//!
//! Every judgment leg is gated by `format: schema(...)`, so the host
//! validates the reply against the derived answer schema before the guest
//! sees it. This module carries the three vendored schema pins
//! (`schemas/answers/{leads,evidence,report}.schema.json`) as embedded
//! strings and the matching parse functions: a survey answer's `leads[]`
//! envelope, an extract answer's Evidence body, and a build / merge
//! answer's full diagnostic shape projected onto the compact seam-facing
//! [`Report`].

use serde::Deserialize;

use crate::seam::{BuildOutput, Evidence, Finding, Lead, Report, Severity, Status, UiSurface};

/// The derived judgment-answer schema gating `survey` replies — the
/// vendored `schemas/answers/leads.schema.json` pin.
pub const LEADS_ANSWER_SCHEMA: &str = include_str!("../../../schemas/answers/leads.schema.json");

/// The derived judgment-answer schema gating `extract` replies — the
/// vendored `schemas/answers/evidence.schema.json` pin.
pub const EVIDENCE_ANSWER_SCHEMA: &str =
    include_str!("../../../schemas/answers/evidence.schema.json");

/// The derived judgment-answer schema gating `build` / `merge` replies —
/// the vendored `schemas/answers/report.schema.json` pin.
pub const REPORT_ANSWER_SCHEMA: &str = include_str!("../../../schemas/answers/report.schema.json");

/// The schema-gated `survey` answer envelope: leads ride under a `leads`
/// key so the answer stays one JSON object.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct LeadsAnswer {
    /// Every lead the survey surfaced, in source order.
    leads: Vec<Lead>,
}

/// Deserialize a schema-gated `survey` answer body into its leads.
///
/// # Errors
///
/// Returns the underlying JSON error when the answer does not parse into
/// the `{ "leads": [...] }` envelope — the host gate makes this
/// unreachable in production, but a misbehaving provider must fail loudly.
pub fn parse_leads(answer: &str) -> Result<Vec<Lead>, serde_json::Error> {
    serde_json::from_str::<LeadsAnswer>(answer).map(|envelope| envelope.leads)
}

/// Deserialize a schema-gated `extract` answer body into its Evidence.
///
/// # Errors
///
/// Returns the underlying JSON error when the answer does not parse into
/// the Evidence shape.
pub fn parse_evidence(answer: &str) -> Result<Evidence, serde_json::Error> {
    serde_json::from_str(answer)
}

/// The slice of one full diagnostic the seam projection reads.
///
/// The answer carries the complete
/// `schemas/diagnostics/diagnostic.schema.json` shape; unprojected fields
/// (`id`, `source`, `evidence`, `fingerprint`, …) are host-validated and
/// deliberately not modeled here.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct Diagnostic {
    /// Durable codex citation, absent for findings that cite no rule.
    #[serde(default)]
    pub rule_id: Option<String>,
    /// Short finding title.
    pub title: String,
    /// Review severity.
    pub severity: Severity,
    /// Operator-facing risk.
    pub impact: String,
    /// Concrete action to clear the finding.
    pub remediation: String,
}

impl Diagnostic {
    /// Fold this full diagnostic into the compact seam-facing [`Finding`]:
    /// `rule-id` and `severity` map through, and the `title` / `impact` /
    /// `remediation` prose folds into `detail`.
    #[must_use]
    pub fn into_finding(self) -> Finding {
        Finding {
            rule_id: self.rule_id,
            severity: self.severity,
            detail: format!("{} — {}; remediation: {}", self.title, self.impact, self.remediation),
        }
    }
}

/// The schema-gated `build` / `merge` answer body.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct ReportAnswer {
    /// Operation outcome as judged by the model.
    pub status: Status,
    /// Full structured diagnostics; default `[]`.
    #[serde(default)]
    pub findings: Vec<Diagnostic>,
    /// Per-platform build outputs; default `[]`.
    #[serde(default)]
    pub outputs: Vec<BuildOutput>,
    /// Optional UI-surface signal.
    #[serde(default)]
    pub ui_surface: Option<UiSurface>,
}

impl ReportAnswer {
    /// Deserialize a schema-gated answer body.
    ///
    /// # Errors
    ///
    /// Returns the underlying JSON error when the answer does not parse
    /// into the report shape — the host gate makes this unreachable in
    /// production, but a misbehaving provider must fail loudly.
    pub fn parse(answer: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(answer)
    }

    /// Project onto the compact seam-facing [`Report`].
    #[must_use]
    pub fn into_report(self) -> Report {
        Report {
            status: self.status,
            findings: self.findings.into_iter().map(Diagnostic::into_finding).collect(),
            outputs: self.outputs,
            ui_surface: self.ui_surface,
        }
    }
}
