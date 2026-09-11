//! The dated scorecard over one full eval run: the measured
//! product.md numbers and both repos' shas. The release workflow
//! verifies a green scorecard; it never runs the live eval.

use std::fmt;

/// product.md target: time to first reviewable specification.
pub const TIME_TARGET_SECS: f64 = 30.0 * 60.0;

// product.md target: per-operation success rate.
const OP_TARGET: f64 = 0.95;

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
    /// True when the run covered the whole case catalog. A filtered
    /// run is an iteration aid and can never produce a green record.
    pub complete: bool,
}

/// One case's recorded outcome.
#[derive(Debug, Clone)]
pub struct CaseResult {
    /// The case id.
    pub id: String,
    /// The typed outcome.
    pub outcome: Outcome,
    /// Wall-clock seconds, one `specify` invocation through the
    /// committed revision.
    pub secs: f64,
    /// Operations that succeeded (one extract per source, one
    /// synthesis).
    pub ops_succeeded: u32,
    /// Operations that failed typed.
    pub ops_failed: u32,
    /// Head sha of the cloned fixture, recorded for reproducibility.
    pub fixture_sha: Option<String>,
}

/// How a case ended: every branch is a typed record.
#[derive(Debug, Clone)]
pub enum Outcome {
    /// A committed revision with no graded findings.
    Pass {
        /// The committed revision id.
        revision: String,
    },
    /// A typed nonzero exit from the published contract.
    TypedFailure {
        /// The `error` discriminant of the failure envelope.
        error: String,
        /// The typed exit code.
        exit_code: u8,
    },
    /// A committed revision with graded findings.
    Findings(Vec<String>),
}

impl Scorecard {
    fn worst_secs(&self) -> f64 {
        self.cases.iter().map(|case| case.secs).fold(0.0, f64::max)
    }

    fn op_rate(&self) -> f64 {
        let succeeded: u32 = self.cases.iter().map(|case| case.ops_succeeded).sum();
        let failed: u32 = self.cases.iter().map(|case| case.ops_failed).sum();
        let total = succeeded + failed;
        if total == 0 { 0.0 } else { f64::from(succeeded) / f64::from(total) }
    }

    /// Green exactly when the whole catalog ran, every case passed,
    /// and both measured product.md numbers meet their targets.
    #[must_use]
    pub fn green(&self) -> bool {
        self.complete
            && !self.cases.is_empty()
            && self.cases.iter().all(|case| matches!(case.outcome, Outcome::Pass { .. }))
            && self.worst_secs() <= TIME_TARGET_SECS
            && self.op_rate() >= OP_TARGET
    }
}

/// The scorecard document. The `status:` / `emery-sha:` lines are the
/// machine-readable record of the run.
impl fmt::Display for Scorecard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "# Emery eval scorecard — {}\n", self.date)?;
        writeln!(f, "- status: {}", if self.green() { "green" } else { "red" })?;
        writeln!(f, "- emery-sha: {}", self.emery_sha)?;
        writeln!(f, "- adapters-sha: {}", self.adapters_sha)?;
        let catalog = if self.complete { "complete" } else { "filtered (never green)" };
        writeln!(f, "- catalog: {catalog}")?;
        writeln!(f, "\n## product.md numbers\n")?;
        writeln!(
            f,
            "- time-to-first-reviewable-spec: {:.0}s (target ≤{:.0}s)",
            self.worst_secs(),
            TIME_TARGET_SECS
        )?;
        writeln!(
            f,
            "- per-operation-success: {:.1}% (target ≥{:.0}%)",
            self.op_rate() * 100.0,
            OP_TARGET * 100.0
        )?;
        writeln!(
            f,
            "- reviewability-beyond-mechanical: unconfirmed (model grading not yet wired)"
        )?;
        writeln!(f, "\n## cases\n")?;
        self.cases.iter().try_for_each(|case| write!(f, "{case}"))
    }
}

/// One case's bullet under the scorecard's `## cases`.
impl fmt::Display for CaseResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.outcome {
            Outcome::Pass { revision } => {
                let fixture = self
                    .fixture_sha
                    .as_deref()
                    .map(|sha| format!(", fixture {sha}"))
                    .unwrap_or_default();
                writeln!(
                    f,
                    "- {}: pass — revision `{revision}`, {:.0}s, ops {}/{}{fixture}",
                    self.id,
                    self.secs,
                    self.ops_succeeded,
                    self.ops_succeeded + self.ops_failed
                )
            }
            Outcome::TypedFailure { error, exit_code } => {
                writeln!(f, "- {}: typed failure `{error}` (exit {exit_code})", self.id)
            }
            Outcome::Findings(findings) => {
                writeln!(f, "- {}: graded findings", self.id)?;
                findings.iter().try_for_each(|finding| writeln!(f, "  - {finding}"))
            }
        }
    }
}
