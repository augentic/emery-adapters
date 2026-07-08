//! Shared scaffolding for target-adapter operation templates.
//!
//! Every target core decomposes `build` / `merge` into schema-gated
//! judgment legs and brackets them with deterministic guest code. The
//! pieces with no per-adapter variance live here: the internal phase-leg
//! answer shape and its schema, the leg helpers over [`judgment`], the
//! prompt renderers, and the deterministic report-coherence checks. Leg
//! sequencing and adapter-specific validator gates stay in each core.

use std::path::Path;

use serde::Deserialize;

use crate::answers::{REPORT_ANSWER_SCHEMA, ReportAnswer};
use crate::judgment;
use crate::model::JudgmentModel;
use crate::seam::{Changeset, Context, Error, Finding, Input, Report, Status};

/// Answer schema for one internal phase leg.
///
/// Internal legs are not part of the `specify:adapter` contract, so
/// this schema is adapter-internal rather than derived from a canonical
/// schema.
pub const PHASE_ANSWER_SCHEMA: &str = r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "additionalProperties": false,
  "properties": {
    "applicable": {
      "description": "Whether this leg had work to do. `false` means the leg wrote nothing (e.g. a phase or format sub-flow with no owned surface in this slice).",
      "type": "boolean"
    },
    "summary": {
      "description": "One-paragraph account of what was generated, reviewed, repaired, or why the leg was skipped.",
      "minLength": 1,
      "type": "string"
    },
    "written": {
      "default": [],
      "description": "Workspace-relative paths of files this leg created or modified.",
      "items": { "type": "string" },
      "type": "array"
    }
  },
  "required": ["applicable", "summary"]
}"#;

/// One internal phase leg's schema-gated answer.
#[derive(Debug, Deserialize)]
pub struct PhaseAnswer {
    /// Whether the leg had work to do.
    pub applicable: bool,
    /// One-paragraph account of the leg's outcome.
    pub summary: String,
    /// Workspace-relative paths the leg created or modified.
    #[serde(default)]
    pub written: Vec<String>,
}

/// Issue one internal phase leg through the shared judgment helper.
///
/// # Errors
///
/// As [`judgment`].
pub async fn phase<P: JudgmentModel>(
    model: &P, ctx: &Context<'_>, system: String, user: String, name: &str,
) -> Result<PhaseAnswer, Error> {
    judgment(model, ctx, system, user, name, PHASE_ANSWER_SCHEMA).await
}

/// Issue one report leg gated by the derived answer schema and project
/// the answer onto the seam-facing report.
///
/// # Errors
///
/// As [`judgment`].
pub async fn report<P: JudgmentModel>(
    model: &P, ctx: &Context<'_>, system: String, user: String,
) -> Result<Report, Error> {
    judgment::<P, ReportAnswer>(model, ctx, system, user, "report", REPORT_ANSWER_SCHEMA)
        .await
        .map(ReportAnswer::into_report)
}

/// Assemble a system prompt from embedded prompt bodies, shared preamble first.
#[must_use]
pub fn assemble_system(bodies: &[&str]) -> String {
    bodies.join("\n\n---\n\n")
}

/// Render the typed inputs as labeled prompt sections.
#[must_use]
pub fn render_inputs(inputs: &[Input]) -> String {
    if inputs.is_empty() {
        return "(no slice artifacts were provided)".to_string();
    }
    inputs
        .iter()
        .map(|input| format!("### input: {}\n\n{}", input.label(), input.body()))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Render a changeset's edits for the merge prompt.
#[must_use]
pub fn render_delta(delta: &Changeset) -> String {
    if delta.edits.is_empty() {
        return "### delta\n\n(empty changeset — the slice wrote no files)".to_string();
    }
    let edits = delta
        .edits
        .iter()
        .map(|edit| {
            edit.content.as_ref().map_or_else(
                || format!("- {} (deleted)", edit.path),
                |content| format!("- {} (content: {content})", edit.path),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("### delta (base {})\n\n{edits}", delta.base)
}

/// Render one phase leg's outcome for the report prompt.
#[must_use]
pub fn render_outcome(name: &str, answer: &PhaseAnswer) -> String {
    format!(
        "- {name}: applicable={}, wrote {:?} — {}",
        answer.applicable, answer.written, answer.summary
    )
}

/// The declared outputs a `success` report claims that the mounted tree
/// does not contain, one findings-style line each.
///
/// A `failure` report is already parked for human review per the prompts'
/// stop contract, so its output claims are not re-litigated.
#[must_use]
pub fn missing_outputs(report: &Report, tree_root: &Path) -> Vec<String> {
    if report.status == Status::Failure {
        return Vec::new();
    }
    report
        .outputs
        .iter()
        .filter(|output| !tree_root.join(&output.path).exists())
        .map(|output| {
            format!("- declared output `{}` does not exist in the working tree", output.path)
        })
        .collect()
}

/// Deterministic guard after the final answer lands.
///
/// Residual findings force `failure` and are appended to the report; a
/// `success` answer carrying blocking findings is downgraded the same
/// way.
#[must_use]
pub fn enforce(mut report: Report, residual: Vec<Finding>) -> Report {
    if !residual.is_empty() {
        report.status = Status::Failure;
        report.findings.extend(residual);
    }
    if report.status == Status::Success
        && report.findings.iter().any(|finding| finding.severity.blocking())
    {
        report.status = Status::Failure;
    }
    report
}
