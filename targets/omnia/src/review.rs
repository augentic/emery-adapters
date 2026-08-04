//! Standards-review answer: the phase answer plus the inline report
//! fields.
//!
//! The review leg closes the build — its answer carries the findings
//! synthesis and the output declaration, so no separate report leg is
//! spawned. The adapter assembles the seam [`Report`] from it in-guest;
//! the deterministic report gate (missing outputs, blocking findings)
//! still applies to the assembled report.

use adapter::answers::Diagnostic;
use adapter::seam::{BuildOutput, Report, Status};
use serde::Deserialize;

/// Answer schema for the review leg (not part of the WIT contract).
pub const REVIEW_ANSWER_SCHEMA: &str = r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "additionalProperties": false,
  "properties": {
    "applicable": {
      "description": "Whether this leg had work to do. A review always does; `false` means nothing was reviewable and the summary explains why.",
      "type": "boolean"
    },
    "summary": {
      "description": "One-paragraph account of the review, the remediation cycle, and the build close-out.",
      "minLength": 1,
      "type": "string"
    },
    "written": {
      "default": [],
      "description": "Workspace-relative paths this leg created or modified (REVIEW.md, tasks.md, remediation edits).",
      "items": { "type": "string" },
      "type": "array"
    },
    "findings": {
      "default": [],
      "description": "Findings left unresolved after the remediation cycle, synthesised from REVIEW.md, the verify-repair output, and capture replay. Any `critical` / `important` finding forces the assembled build report to `failure`; a build that cannot succeed must carry at least one.",
      "items": {
        "additionalProperties": false,
        "properties": {
          "rule-id": {
            "description": "Codex citation (e.g. `OMNIA-002`); omit for findings that cite no codex policy.",
            "type": "string"
          },
          "title": {
            "description": "Short finding title.",
            "type": "string"
          },
          "severity": {
            "description": "Review severity.",
            "enum": ["critical", "important", "suggestion", "optional"],
            "type": "string"
          },
          "impact": {
            "description": "Operator-facing risk.",
            "type": "string"
          },
          "remediation": {
            "description": "Concrete action to clear the finding.",
            "type": "string"
          }
        },
        "required": ["title", "severity", "impact", "remediation"],
        "type": "object"
      },
      "type": "array"
    },
    "outputs": {
      "default": [],
      "description": "Build outputs the workspace now carries: the slice's crate tree and, in create mode, the guest scaffolding — `platform: core`, paths relative to the workspace root. The deterministic report gate fails the build when a declared path is missing.",
      "items": {
        "additionalProperties": false,
        "properties": {
          "platform": {
            "description": "Target platform of the output.",
            "enum": ["core", "ios", "android", "web", "desktop"],
            "type": "string"
          },
          "path": {
            "description": "Project-root-relative path of the produced artifact.",
            "type": "string"
          }
        },
        "required": ["platform", "path"],
        "type": "object"
      },
      "type": "array"
    }
  },
  "required": ["applicable", "summary"]
}"#;

/// The review leg's schema-gated answer — only the report residue the
/// adapter consumes. The schema's narrative fields (`applicable`,
/// `summary`, `written`) are elicited for the transcript and ignored
/// here.
#[derive(Debug, Deserialize)]
pub struct ReviewAnswer {
    /// Findings left unresolved after the remediation cycle.
    #[serde(default)]
    pub findings: Vec<Diagnostic>,
    /// Build outputs the workspace carries.
    #[serde(default)]
    pub outputs: Vec<BuildOutput>,
}

impl ReviewAnswer {
    /// Assemble the seam-facing report. Status starts at `success`; the
    /// deterministic gate downgrades it when a blocking finding rides
    /// the answer or a declared output is missing from the tree.
    pub fn into_report(self) -> Report {
        Report {
            status: Status::Success,
            findings: self.findings.into_iter().map(Diagnostic::into_finding).collect(),
            outputs: self.outputs,
            ui_surface: None,
        }
    }
}
