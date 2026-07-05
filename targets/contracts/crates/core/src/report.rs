//! Report-answer deserialization and the seam projection.
//!
//! A judgment leg's final `create` call is gated by the derived answer
//! schema (`schemas/answers/report.schema.json`), so the host validates
//! the reply before the guest sees it. This module deserializes that
//! answer into [`ReportAnswer`] — the full diagnostic shape — and projects
//! it onto [`Report`], the compact WIT-shaped record that crosses the
//! guest-to-guest seam: `rule-id` and `severity` map through, and each
//! diagnostic's `title` / `impact` / `remediation` prose folds into
//! `detail`.

use serde::Deserialize;

use crate::validate::ContractFinding;

/// The derived judgment-answer schema gating `build` / `merge` replies —
/// the vendored `schemas/answers/report.schema.json` pin.
pub const REPORT_ANSWER_SCHEMA: &str =
    include_str!("../../../../../schemas/answers/report.schema.json");

/// Closed review severity enum, ordered for sort stability.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum Severity {
    /// Must fix; blocks success.
    Critical,
    /// Should fix; blocks success.
    Important,
    /// Advisory; never blocks.
    Suggestion,
    /// Take-it-or-leave-it; never blocks.
    Optional,
}

impl Severity {
    /// Whether a finding at this severity blocks a `success` report.
    #[must_use]
    pub const fn blocking(self) -> bool {
        matches!(self, Self::Critical | Self::Important)
    }
}

/// Operation outcome.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    /// The operation completed; findings, if any, are non-blocking.
    Success,
    /// The operation did not complete cleanly.
    Failure,
}

/// Closed target platform taxonomy.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Platform {
    /// Shared core.
    Core,
    /// iOS shell.
    Ios,
    /// Android shell.
    Android,
    /// Web shell.
    Web,
    /// Desktop shell.
    Desktop,
}

/// One per-platform build output declared by the answer.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct BuildOutput {
    /// Platform this output was produced for.
    pub platform: Platform,
    /// Relative path (from the project root) to the produced artifact.
    pub path: String,
}

/// Per-slice UI-surface signal.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct UiSurface {
    /// Count of screen-bearing requirements the slice introduces or modifies.
    pub screens: u32,
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
            findings: self.findings.into_iter().map(Finding::from_diagnostic).collect(),
            outputs: self.outputs,
            ui_surface: self.ui_surface,
        }
    }
}

/// Compact seam projection of one diagnostic — the WIT `finding` record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    /// Rule identifier, absent for findings that cite no codex policy.
    pub rule_id: Option<String>,
    /// Review severity.
    pub severity: Severity,
    /// Folded `title` / `impact` / `remediation` prose.
    pub detail: String,
}

impl Finding {
    /// Fold one full diagnostic into the compact seam shape.
    #[must_use]
    pub fn from_diagnostic(diagnostic: Diagnostic) -> Self {
        Self {
            rule_id: diagnostic.rule_id,
            severity: diagnostic.severity,
            detail: format!(
                "{} — {}; remediation: {}",
                diagnostic.title, diagnostic.impact, diagnostic.remediation
            ),
        }
    }

    /// Map one deterministic validator finding into the seam shape.
    /// Contract rules gate the build, so validator findings are blocking
    /// (`important`).
    #[must_use]
    pub fn from_validator(finding: &ContractFinding) -> Self {
        Self {
            rule_id: Some(finding.rule_id.to_string()),
            severity: Severity::Important,
            detail: format!("{}: {}", finding.path.display(), finding.detail),
        }
    }
}

/// Judgment returned by `build` and `merge` — the WIT `report` record.
/// The resulting state lives in the working tree, not here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    /// Operation outcome.
    pub status: Status,
    /// Compact findings.
    pub findings: Vec<Finding>,
    /// Per-platform build outputs.
    pub outputs: Vec<BuildOutput>,
    /// Optional UI-surface signal.
    pub ui_surface: Option<UiSurface>,
}
