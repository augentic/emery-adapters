//! Mechanical grading of a committed specification: disagreement and
//! gaps inline, provenance one gesture away.
//!
//! Checks operate on the published wire shapes only — the typed master
//! `emery show --format json` carries as `document` and the Markdown
//! projection it carries as `body` — no engine parser is linked.

use serde::Deserialize;

// The requirement-heading prefix of the published projection.
const HEADING: &str = "### Requirement:";

// The statuses the projection also surfaces as a heading tag.
const TAGGED: [&str; 3] = ["unknown", "conflict", "divergence"];

/// Grade the typed `spec` and its `body` projection mechanically; each
/// finding is one failed property. An empty list is a pass.
#[must_use]
pub fn spec(spec: &Spec, body: &str, expect: &Expect) -> Vec<String> {
    let mut findings = Vec::new();
    if spec.requirements.is_empty() {
        findings.push("no requirements: the spec is not reviewable".to_string());
        return findings;
    }

    if !spec.requirements.iter().any(|req| req.subject.contains(expect.subject_fragment)) {
        findings.push(format!(
            "no requirement subject mentions `{}`: the spec misses the bound estate",
            expect.subject_fragment
        ));
    }
    identity(&spec.requirements, &mut findings);
    for req in &spec.requirements {
        provenance(req, &mut findings);
        inline_tags(req, body, &mut findings);
    }

    findings
}

/// Graded expectations one case declares over its committed spec.
#[derive(Debug, Clone, Copy)]
pub struct Expect {
    /// A fragment at least one requirement subject must contain —
    /// the cheap "the spec is about the bound estate" check.
    pub subject_fragment: &'static str,
}

/// The graded slice of the typed specification master: the fields the
/// mechanical properties read. Unknown fields are ignored, so the
/// grader survives additive engine changes.
#[derive(Debug, Clone, Deserialize)]
pub struct Spec {
    /// The requirements in id order.
    pub requirements: Vec<Requirement>,
}

/// One requirement of the typed master.
#[derive(Debug, Clone, Deserialize)]
pub struct Requirement {
    /// The stored `REQ-NNN` identity.
    pub id: String,
    /// The claim id the requirement is about.
    pub subject: String,
    /// `agreed`, `unknown`, `conflict`, or `divergence`.
    pub status: String,
    /// The `(source, claim)` pairs the requirement cites.
    pub sources: Vec<Cited>,
}

/// One cited `(source, claim)` pair.
#[derive(Debug, Clone, Deserialize)]
pub struct Cited {
    /// The source key.
    pub source: String,
    /// The claim id within that source.
    pub claim: String,
}

// Identity is stored and stable: every requirement carries a
// `REQ-NNN` id and no two share one.
fn identity(requirements: &[Requirement], findings: &mut Vec<String>) {
    let mut seen = std::collections::BTreeSet::new();
    for req in requirements {
        let digits = req.id.strip_prefix("REQ-").map_or("", str::trim);
        if digits.len() < 3 || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            findings.push(format!("`{}` (`{}`) is not a `REQ-NNN` id", req.subject, req.id));
        }
        if !seen.insert(req.id.as_str()) {
            findings.push(format!("`{}` is carried by more than one requirement", req.id));
        }
    }
}

// Provenance one gesture away: every requirement cites at least one
// `(source, claim)` pair, and every pair names both halves.
fn provenance(req: &Requirement, findings: &mut Vec<String>) {
    if req.sources.is_empty() {
        findings.push(format!("`{}` cites no source (provenance)", req.id));
    }
    for cited in &req.sources {
        if cited.source.trim().is_empty() || cited.claim.trim().is_empty() {
            findings.push(format!(
                "`{}` cites an incomplete pair `{}:{}` (provenance)",
                req.id, cited.source, cited.claim
            ));
        }
    }
}

// Disagreement and gaps are inline: a tagged status must surface as
// the matching tag on the requirement's projected heading, and vice
// versa.
fn inline_tags(req: &Requirement, body: &str, findings: &mut Vec<String>) {
    let prefix = format!("{HEADING} {}", req.subject);
    let Some(heading) = body.lines().find(|line| {
        line.strip_prefix(prefix.as_str())
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(" ["))
    }) else {
        findings.push(format!("`{}` (`{}`) has no heading in the projection", req.id, req.subject));
        return;
    };
    for tagged in TAGGED {
        let tag = format!("[{tagged}]");
        if (req.status == tagged) != heading.contains(&tag) {
            findings.push(format!(
                "`{heading}` (`status: {}`) and the `{tag}` heading tag disagree — \
                 disagreement and gaps must be visible in place",
                req.status
            ));
        }
    }
}
