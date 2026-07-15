//! Judgment-answer schemas and deserialization.
//!
//! Every judgment leg is gated by `format: schema(...)`, so the host
//! validates the reply against the generated answer schema before the
//! guest sees it. This module carries the three vendored schema pins
//! (`schemas/answers/{leads,evidence,report}.schema.json`, generated
//! upstream from the Rust wire types) and the matching parse functions.
//! The source-axis answers also get deterministic validation tails
//! ([`validate_leads`] / [`validate_evidence`]) re-checking the id
//! grammars alongside what the schemas cannot express (trim-aware
//! synopses), plus the composed [`leads_tail`] / [`evidence_tail`]
//! source operations run inside [`crate::repaired`]'s bounded
//! repair loop.

use serde::Deserialize;

use crate::seam::{
    BuildOutput, ClaimKind, Error, Evidence, Finding, Lead, Report, Severity, Status, UiSurface,
};

/// Answer schema gating `survey` replies.
pub const LEADS_ANSWER_SCHEMA: &str = include_str!("../schemas/answers/leads.schema.json");

/// Answer schema gating `extract` replies.
pub const EVIDENCE_ANSWER_SCHEMA: &str = include_str!("../schemas/answers/evidence.schema.json");

/// Answer schema gating `build` / `merge` replies.
pub const REPORT_ANSWER_SCHEMA: &str = include_str!("../schemas/answers/report.schema.json");

/// The `survey` answer envelope: leads ride under a `leads` key so the
/// answer stays one JSON object.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct LeadsAnswer {
    /// Every lead the survey surfaced, in source order.
    pub leads: Vec<Lead>,
}

/// Deserialize a `survey` answer body into its leads.
///
/// # Errors
///
/// Returns the underlying JSON error when the answer does not parse into
/// the `{ "leads": [...] }` envelope.
pub fn parse_leads(answer: &str) -> Result<Vec<Lead>, serde_json::Error> {
    serde_json::from_str::<LeadsAnswer>(answer).map(|envelope| envelope.leads)
}

/// Deserialize an `extract` answer body into its Evidence.
///
/// # Errors
///
/// Returns the underlying JSON error when the answer does not parse into
/// the Evidence shape.
pub fn parse_evidence(answer: &str) -> Result<Evidence, serde_json::Error> {
    serde_json::from_str(answer)
}

/// Lead-id kebab grammar, enforced deterministically (the generated
/// answer schema leaves lead ids as plain strings).
const KEBAB_PATTERN: &str = "^[a-z0-9]+(-[a-z0-9]+)*$";

/// Claim-id dotted-kebab grammar, enforced deterministically (the
/// generated answer schema leaves claim ids as plain strings).
const DOTTED_KEBAB_PATTERN: &str = "^[a-z0-9]+(-[a-z0-9]+)*(\\.[a-z0-9]+(-[a-z0-9]+)*)*$";

fn is_kebab(value: &str) -> bool {
    !value.is_empty()
        && value.split('-').all(|seg| {
            !seg.is_empty() && seg.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}

fn is_dotted_kebab(value: &str) -> bool {
    !value.is_empty() && value.split('.').all(is_kebab)
}

fn enforce(operation: &str, findings: &[String]) -> Result<(), Error> {
    if findings.is_empty() {
        return Ok(());
    }
    Err(Error::Internal(format!(
        "{operation} answer failed deterministic validation:\n{}",
        findings.join("\n")
    )))
}

/// Re-check a `survey` answer after the host gate.
///
/// Every lead id and topic slug must match the kebab-case grammar and
/// every synopsis must carry content — the same set the engine's
/// `artifacts::discovery::validate_leads` enforces before merging into
/// `discovery.md`.
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
        for topic in &lead.topics {
            if !is_kebab(topic) {
                findings.push(format!(
                    "- lead `{}`: topic `{topic}` does not match `{KEBAB_PATTERN}`",
                    lead.lead
                ));
            }
        }
    }
    enforce("survey", &findings)
}

/// The composed `survey` answer tail: typed parse plus deterministic
/// validation, the shape [`crate::repaired`]'s repair loop retries.
///
/// # Errors
///
/// Returns [`Error::Internal`] when the answer does not parse into the
/// `{ "leads": [...] }` envelope or fails [`validate_leads`].
pub fn leads_tail(answer: &str) -> Result<Vec<Lead>, Error> {
    let leads = parse_leads(answer)
        .map_err(|err| Error::Internal(format!("leads answer did not deserialize: {err}")))?;
    validate_leads(&leads)?;
    Ok(leads)
}

/// Re-check an `extract` answer after the host gate: claim ids must match
/// the dotted-kebab grammar, and `requirement` / `criterion` /
/// `example` claims must carry one.
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

/// The composed `extract` answer tail: typed parse plus deterministic
/// validation, the shape [`crate::repaired`]'s repair loop retries.
///
/// # Errors
///
/// Returns [`Error::Internal`] when the answer does not parse into the
/// Evidence shape or fails [`validate_evidence`].
pub fn evidence_tail(answer: &str) -> Result<Evidence, Error> {
    let evidence = parse_evidence(answer)
        .map_err(|err| Error::Internal(format!("evidence answer did not deserialize: {err}")))?;
    validate_evidence(&evidence)?;
    Ok(evidence)
}

/// The slice of one full diagnostic the seam projection reads. The rest
/// of the `diagnostic.schema.json` shape is host-validated and not
/// modeled here.
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
    /// Fold into the compact seam-facing [`Finding`], collapsing `title` /
    /// `impact` / `remediation` into `detail`.
    #[must_use]
    pub fn into_finding(self) -> Finding {
        Finding {
            rule_id: self.rule_id,
            severity: self.severity,
            detail: format!("{} — {}; remediation: {}", self.title, self.impact, self.remediation),
        }
    }
}

/// The `build` / `merge` answer body.
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
    /// Deserialize an answer body.
    ///
    /// # Errors
    ///
    /// Returns the underlying JSON error when the answer does not parse
    /// into the report shape.
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
