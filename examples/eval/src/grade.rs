//! Mechanical grading of a committed `spec.md`: disagreement and gaps
//! inline, provenance one gesture away. Checks operate on the
//! published document format only — no engine parser is linked.

// The requirement-heading prefix of the published spec format.
const HEADING: &str = "### Requirement:";

/// Grade `spec` mechanically; each finding is one failed property.
/// An empty list is a pass.
#[must_use]
pub fn spec(text: &str, expect: &Expect) -> Vec<String> {
    let mut findings = Vec::new();
    let blocks = blocks(text);
    if blocks.is_empty() {
        findings.push("no requirement blocks: the spec is not reviewable".to_string());
        return findings;
    }

    if !blocks.iter().any(|(heading, _)| heading.contains(expect.subject_fragment)) {
        findings.push(format!(
            "no requirement heading mentions `{}`: the spec misses the bound estate",
            expect.subject_fragment
        ));
    }
    for (heading, body) in &blocks {
        provenance(heading, body, &mut findings);
        inline_tags(heading, body, &mut findings);
    }

    findings
}

/// Graded expectations one case declares over its committed spec.
#[derive(Debug, Clone, Copy)]
pub struct Expect {
    /// A fragment at least one requirement heading must contain —
    /// the cheap "the spec is about the bound estate" check.
    pub subject_fragment: &'static str,
}

// Every requirement block as `(heading line, body)`.
fn blocks(text: &str) -> Vec<(&str, &str)> {
    let mut blocks = Vec::new();
    let mut remaining = text;
    while let Some(start) = remaining.find(HEADING) {
        let block = &remaining[start..];
        let end = block[HEADING.len()..].find(HEADING).map_or(block.len(), |at| at + HEADING.len());
        let block = &block[..end];
        let (heading, body) = block.split_once('\n').unwrap_or((block, ""));
        blocks.push((heading.trim_end(), body));
        remaining = &remaining[start + end..];
    }
    blocks
}

// Provenance one gesture away: every block carries its `ID:`,
// `Sources:`, and `Status:` lines.
fn provenance(heading: &str, body: &str, findings: &mut Vec<String>) {
    for line in ["ID:", "Sources:", "Status:"] {
        if !body.lines().any(|candidate| candidate.trim_start().starts_with(line)) {
            findings.push(format!("`{heading}` is missing its `{line}` line (provenance)"));
        }
    }
}

// Disagreement and gaps are inline: a non-`agreed` status must
// surface as the matching heading tag, and vice versa.
fn inline_tags(heading: &str, body: &str, findings: &mut Vec<String>) {
    let status = body
        .lines()
        .find_map(|line| line.trim_start().strip_prefix("Status:"))
        .map_or("", str::trim);
    for tagged in ["unknown", "conflict", "divergence"] {
        let tag = format!("[{tagged}]");
        if (status == tagged) != heading.contains(&tag) {
            findings.push(format!(
                "`{heading}` (`Status: {status}`) and the `{tag}` heading tag disagree — \
                 disagreement and gaps must be visible in place"
            ));
        }
    }
}
