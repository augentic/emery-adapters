//! Judgment-answer schemas and deserialization.
//!
//! Every judgment leg is gated by `format: schema(...)`, so the host
//! validates the reply against the derived answer schema before the guest
//! sees it. This module carries the three vendored schema pins
//! (`crates/guest-kit/schemas/answers/{leads,evidence,report}.schema.json`) as embedded
//! strings and the matching parse functions: a survey answer's `leads[]`
//! envelope, an extract answer's Evidence body, and a build / merge
//! answer's full diagnostic shape projected onto the compact seam-facing
//! [`Report`]. The source-axis answers also get deterministic validation
//! tails ([`validate_leads`] / [`validate_evidence`]) re-checking the id
//! grammar the schemas pin, so a misconfigured host gate cannot leak a
//! malformed answer into the workflow.

use serde::Deserialize;

use crate::seam::{
    BuildOutput, ClaimKind, Error, Evidence, Finding, Lead, Report, Severity, Status, UiSurface,
};

/// The derived judgment-answer schema gating `survey` replies — the
/// vendored `crates/guest-kit/schemas/answers/leads.schema.json` pin.
pub const LEADS_ANSWER_SCHEMA: &str = include_str!("../schemas/answers/leads.schema.json");

/// The derived judgment-answer schema gating `extract` replies — the
/// vendored `crates/guest-kit/schemas/answers/evidence.schema.json` pin.
pub const EVIDENCE_ANSWER_SCHEMA: &str = include_str!("../schemas/answers/evidence.schema.json");

/// The derived judgment-answer schema gating `build` / `merge` replies —
/// the vendored `crates/guest-kit/schemas/answers/report.schema.json` pin.
pub const REPORT_ANSWER_SCHEMA: &str = include_str!("../schemas/answers/report.schema.json");

/// The schema-gated `survey` answer envelope: leads ride under a `leads`
/// key so the answer stays one JSON object.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct LeadsAnswer {
    /// Every lead the survey surfaced, in source order.
    pub leads: Vec<Lead>,
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

/// The kebab-case pattern the answer schemas pin on lead ids
/// (`leads.schema.json` `$defs.kebabName`).
const KEBAB_PATTERN: &str = "^[a-z0-9]+(-[a-z0-9]+)*$";

/// The dotted-kebab pattern the answer schemas pin on claim ids
/// (`evidence.schema.json` `$defs.claim.properties.id`).
const DOTTED_KEBAB_PATTERN: &str = "^[a-z0-9]+(-[a-z0-9]+)*(\\.[a-z0-9]+(-[a-z0-9]+)*)*$";

/// Whether `value` matches the schemas' kebab-case name pattern.
fn is_kebab(value: &str) -> bool {
    !value.is_empty()
        && value.split('-').all(|seg| {
            !seg.is_empty() && seg.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}

/// Whether `value` matches the schemas' dotted-kebab claim-id pattern.
fn is_dotted_kebab(value: &str) -> bool {
    !value.is_empty() && value.split('.').all(is_kebab)
}

/// Fold findings into one findings-style [`Error::Internal`], or pass.
fn enforce(operation: &str, findings: &[String]) -> Result<(), Error> {
    if findings.is_empty() {
        return Ok(());
    }
    Err(Error::Internal(format!(
        "{operation} answer failed deterministic validation:\n{}",
        findings.join("\n")
    )))
}

/// Deterministic tail after a schema-gated `survey` answer lands: every
/// lead id must match the schema's kebab-case pattern and every synopsis
/// must carry content.
///
/// Belt and braces — the host gate already validated the answer against
/// the leads schema, but a misconfigured gate must not let a malformed
/// lead reach the workflow.
///
/// # Errors
///
/// Returns [`Error::Internal`] carrying one findings-style line per
/// violation.
pub fn validate_leads(leads: &[Lead]) -> Result<(), Error> {
    let mut findings = Vec::new();
    for lead in leads {
        if !is_kebab(&lead.lead) {
            findings.push(format!("- lead `{}`: id does not match `{KEBAB_PATTERN}`", lead.lead));
        }
        if lead.synopsis.trim().is_empty() {
            findings.push(format!("- lead `{}`: synopsis is empty", lead.lead));
        }
    }
    enforce("survey", &findings)
}

/// Deterministic tail after a schema-gated `extract` answer lands:
/// claim ids must match the schema's dotted-kebab pattern.
///
/// An id is required when the claim kind is `requirement`, `criterion`,
/// or `example` — mirroring the evidence schema's conditional
/// requirement. Belt and braces, like [`validate_leads`].
///
/// # Errors
///
/// Returns [`Error::Internal`] carrying one findings-style line per
/// violation.
pub fn validate_evidence(evidence: &Evidence) -> Result<(), Error> {
    let mut findings = Vec::new();
    for (index, claim) in evidence.claims.iter().enumerate() {
        match &claim.id {
            Some(id) if !is_dotted_kebab(id) => {
                findings.push(format!(
                    "- claim {index}: id `{id}` does not match `{DOTTED_KEBAB_PATTERN}`"
                ));
            }
            None if matches!(
                claim.kind,
                ClaimKind::Requirement | ClaimKind::Criterion | ClaimKind::Example
            ) =>
            {
                findings.push(format!("- claim {index}: `{:?}` claims require an id", claim.kind));
            }
            _ => {}
        }
    }
    enforce("extract", &findings)
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
