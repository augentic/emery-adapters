//! The dated scorecard over one full eval run.
//!
//! Records the measured product.md numbers and both repos' shas. The
//! document is the release-gate *record* — the emery release workflow
//! verifies a green scorecard names the release-tip sha (CONSTITUTION
//! invariant 6); it never runs the live eval.

use std::fmt::Write as _;

/// product.md target: time to first reviewable specification.
pub const TIME_TARGET_SECS: f64 = 30.0 * 60.0;

/// product.md target: per-operation success rate.
pub const OP_TARGET: f64 = 0.95;

/// One case's recorded outcome.
#[derive(Debug, Clone)]
pub struct CaseResult {
    /// The case id.
    pub id: String,
    /// The typed outcome.
    pub outcome: Outcome,
    /// Wall-clock seconds, `init` through the committed generation
    /// pointer.
    pub secs: f64,
    /// Operations that succeeded (one extract per source, one
    /// synthesis).
    pub ops_succeeded: u32,
    /// Operations that failed typed.
    pub ops_failed: u32,
}

/// How a case ended: every branch is a typed record (T6).
#[derive(Debug, Clone)]
pub enum Outcome {
    /// A committed generation with no graded findings.
    Pass {
        /// The committed generation id.
        generation: String,
    },
    /// A typed nonzero exit from the published contract.
    TypedFailure {
        /// The `error` discriminant of the failure envelope.
        error: String,
        /// The typed exit code.
        exit_code: u8,
    },
    /// A committed generation with graded findings.
    Findings(Vec<String>),
}

/// The dated scorecard over one full eval run.
#[derive(Debug, Clone)]
pub struct Scorecard {
    /// `YYYY-MM-DD` of the run.
    pub date: String,
    /// The `augentic/emery` commit the exercised binary was built from.
    pub emery_sha: String,
    /// The `augentic/emery-adapters` commit the components were built
    /// from.
    pub adapters_sha: String,
    /// Every case's result.
    pub cases: Vec<CaseResult>,
}

impl Scorecard {
    /// Worst wall-clock over the cases, seconds.
    #[must_use]
    pub fn worst_secs(&self) -> f64 {
        self.cases.iter().map(|case| case.secs).fold(0.0, f64::max)
    }

    /// Per-operation success rate over every recorded operation.
    #[must_use]
    pub fn op_rate(&self) -> f64 {
        let succeeded: u32 = self.cases.iter().map(|case| case.ops_succeeded).sum();
        let failed: u32 = self.cases.iter().map(|case| case.ops_failed).sum();
        let total = succeeded + failed;
        if total == 0 { 0.0 } else { f64::from(succeeded) / f64::from(total) }
    }

    /// Green exactly when every case passed and both measured
    /// product.md numbers meet their targets.
    #[must_use]
    pub fn green(&self) -> bool {
        !self.cases.is_empty()
            && self.cases.iter().all(|case| matches!(case.outcome, Outcome::Pass { .. }))
            && self.worst_secs() <= TIME_TARGET_SECS
            && self.op_rate() >= OP_TARGET
    }

    /// The scorecard document. The `status:` / `emery-sha:` lines are
    /// the machine-readable record the release gate greps.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = format!("# Emery eval scorecard — {}\n\n", self.date);
        let status = if self.green() { "green" } else { "red" };
        let _ = writeln!(out, "- status: {status}");
        let _ = writeln!(out, "- emery-sha: {}", self.emery_sha);
        let _ = writeln!(out, "- adapters-sha: {}", self.adapters_sha);
        out.push_str("\n## product.md numbers\n\n");
        let _ = writeln!(
            out,
            "- time-to-first-reviewable-spec: {:.0}s (target ≤{:.0}s)",
            self.worst_secs(),
            TIME_TARGET_SECS
        );
        let _ = writeln!(
            out,
            "- per-operation-success: {:.1}% (target ≥{:.0}%)",
            self.op_rate() * 100.0,
            OP_TARGET * 100.0
        );
        out.push_str(
            "- reviewability-beyond-mechanical: unconfirmed (model grading not yet wired)\n",
        );
        out.push_str("\n## cases\n\n");
        for case in &self.cases {
            match &case.outcome {
                Outcome::Pass { generation } => {
                    let _ = writeln!(
                        out,
                        "- {}: pass — generation `{generation}`, {:.0}s, ops {}/{}",
                        case.id,
                        case.secs,
                        case.ops_succeeded,
                        case.ops_succeeded + case.ops_failed
                    );
                }
                Outcome::TypedFailure { error, exit_code } => {
                    let _ =
                        writeln!(out, "- {}: typed failure `{error}` (exit {exit_code})", case.id);
                }
                Outcome::Findings(findings) => {
                    let _ = writeln!(out, "- {}: graded findings", case.id);
                    for finding in findings {
                        let _ = writeln!(out, "  - {finding}");
                    }
                }
            }
        }
        out
    }
}
