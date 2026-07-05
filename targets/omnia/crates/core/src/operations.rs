//! The judgment operation template: `guidance`, `build`, and `merge`.
//!
//! Each judgment leg is bracketed by deterministic guest code. The core
//! assembles a prompt from the embedded briefs plus the typed inputs,
//! issues a single-shot `create` through the shared
//! [`specify_guest_kit::judgment`] helper with a schema-gated `format`,
//! and then runs the deterministic report-coherence gate after the final
//! answer lands. All state between calls lives in the workspace tree —
//! the session-less shape.
//!
//! `build` decomposes along the build brief's own phase order: one
//! *generation* leg (phases 1–5 — crate writer, test writer, guest
//! writer, and the cargo verify-repair loop, which only the spawned
//! agent can run in the lent workspace), one *review* leg (phase 6 —
//! the standards-review team and its remediation cycle), one *replay*
//! leg (phase 7 — capture replay, self-skipping when no `captures`
//! source is bound), then one report leg. Omnia has no in-core
//! validators (verification is cargo / clippy / wasm32 runs a wasm
//! guest cannot spawn), so the deterministic tail checks what pure Rust
//! over the mounted tree can: declared `outputs` paths exist, and a
//! `success` report carries no blocking findings.

use std::path::Path;

use serde::Deserialize;
use specify_guest_kit::answers::{REPORT_ANSWER_SCHEMA, ReportAnswer};
use specify_guest_kit::seam::{
    Changeset, Context, Error, Finding, Input, Report, Severity, Status, WorkingTree,
};
use specify_guest_kit::{Model, judgment};

use crate::registry;

/// Adapter-internal answer schema for one phase leg. Internal legs are
/// not part of the `augentic:specify` contract, so this schema lives
/// here rather than deriving from a canonical schema.
const PHASE_ANSWER_SCHEMA: &str = r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "additionalProperties": false,
  "properties": {
    "applicable": {
      "description": "Whether the phase had work to do. `false` means the phase wrote nothing (e.g. replay without a `captures` binding).",
      "type": "boolean"
    },
    "summary": {
      "description": "One-paragraph account of what was generated, reviewed, replayed, or why the phase was skipped.",
      "minLength": 1,
      "type": "string"
    },
    "written": {
      "default": [],
      "description": "Workspace-relative paths of files this phase created or modified.",
      "items": { "type": "string" },
      "type": "array"
    }
  },
  "required": ["applicable", "summary"]
}"#;

/// One phase leg's schema-gated answer.
#[derive(Debug, Deserialize)]
struct PhaseAnswer {
    applicable: bool,
    summary: String,
    #[serde(default)]
    written: Vec<String>,
}

/// The pointer at the adapter's own MCP reference shelf every judgment
/// leg's user prompt carries: the shelf serves the full `briefs/` /
/// `references/` / `rules/` trees, so prompts stay lean and the agent
/// fetches specialist material lazily instead of getting it inlined.
const SHELF_POINTER: &str = "Every brief, reference, and rule document this adapter ships is \
     served by the granted `omnia-references` MCP shelf (`list_docs` / `read_doc`, adapter-relative \
     paths like `references/guardrails.md`); fetch documents the briefs cite lazily from there.";

/// Guidance on the expected build artifacts for this target — the
/// embedded shape brief, returned deterministically (no judgment leg).
#[must_use]
pub fn guidance() -> &'static str {
    registry::body("briefs/shape.md")
}

/// Build a slice's crate, tests, and guest scaffolding per the build
/// brief's phase order.
///
/// Session-less decomposition along the brief's own structure: a
/// generation leg (phases 1–5, with the cargo verify-repair loop run by
/// the spawned agent inside the leg), a review leg (phase 6), a replay
/// leg (phase 7, self-skipping when no `captures` source is bound), and
/// one report leg gated by the derived answer schema. The deterministic
/// report-coherence gate then runs in core, with one bounded repair leg
/// when a `success` report declares outputs the mounted tree does not
/// contain.
///
/// # Errors
///
/// Returns [`Error::InvalidRequest`] when the model rejects a request as
/// malformed, and [`Error::Internal`] for other model failures or an
/// answer that does not deserialize.
pub async fn build<P: Model>(
    model: &P, ctx: &Context<'_>, slice: &str, inputs: &[Input], tree: &WorkingTree,
) -> Result<Report, Error> {
    let tree_root = ctx.tree_root(tree);
    let inputs_block = render_inputs(inputs);
    let build_brief = registry::body("briefs/build.md");

    // Phases 1–5 — generation: the shape refresher plus the crate / test
    // / guest sub-briefs ride in the system channel because the brief's
    // verify-repair loop crosses all three (a cargo failure re-enters the
    // owning sub-brief), so one agent leg must hold them together. Mode
    // detection (create vs update) is the brief's own § Mode detection,
    // judged inside the leg against the lent workspace.
    let system = assemble_system(&[
        "briefs/build.md",
        "briefs/shape.md",
        "briefs/build/crate.md",
        "briefs/build/test.md",
        "briefs/build/guest.md",
    ]);
    let user = format!(
        "Run the generation phases (1-5) of the omnia build for slice `{slice}` \
         (adapter `{}`).\n\n\
         The project workspace is lent to you. Detect create vs update mode per the \
         build brief's `## Mode detection`, follow the crate-writer, test-writer, and \
         (create mode only) guest-writer sub-briefs, then run the brief's \
         `## § Verify-repair loop` yourself — the cargo / clippy / test commands run \
         in the lent workspace; this adapter cannot spawn them. {SHELF_POINTER}\n\n\
         {inputs_block}",
        ctx.adapter_id,
    );
    let generation = phase_call(model, ctx, system, user, "generation").await?;

    // Phase 6 — standards review: its remediation cycle may re-enter the
    // crate / test sub-briefs and the verify-repair loop with tighter
    // caps; the specialist protocol and rule codex live on the shelf.
    let system = assemble_system(&["briefs/build.md", "briefs/build/review.md"]);
    let user = format!(
        "Run the standards-review phase (6) of the omnia build for slice `{slice}`: \
         spawn the review team per the review sub-brief, synthesise `REVIEW.md`, and \
         drive the remediation cycle — re-running the verify-repair loop's cargo \
         commands in the lent workspace where the sub-brief calls for it. \
         {SHELF_POINTER}",
    );
    let review = phase_call(model, ctx, system, user, "review").await?;

    // Phase 7 — capture replay: conditional on a `captures` source
    // binding only the workspace knows about, so the leg itself judges
    // applicability and self-skips, like a contracts sub-flow with no
    // owned surface.
    let system = assemble_system(&["briefs/build.md", "briefs/build/replay.md"]);
    let user = format!(
        "Run the capture-replay phase (7) of the omnia build for slice `{slice}`. \
         When the slice has no `captures` source binding in `plan.yaml`, write \
         nothing and answer with `applicable: false` — omission when unbound is not \
         an error. {SHELF_POINTER}",
    );
    let replay = phase_call(model, ctx, system, user, "replay").await?;

    // Final leg — the report answer, gated by the derived answer schema.
    let user = format!(
        "Write the build report for slice `{slice}` per the build brief's \
         `## Build report`. A `success` report carries only non-blocking findings; \
         an exhausted verify-repair budget or unresolved blocking review findings \
         mean `status: failure`. Declare the slice's crate tree (and the guest \
         scaffolding, when this build wrote it) as `platform: core` outputs with \
         paths relative to the project root.\n\n\
         Phase outcomes:\n{}",
        [("generation", &generation), ("review", &review), ("replay", &replay)]
            .map(render_phase_outcome)
            .join("\n"),
    );
    let report = report_call(model, ctx, build_brief.to_string(), user).await?;

    gate_report(model, ctx, build_brief, report, &tree_root, "build").await
}

/// Gate a built slice's landing on the merge brief's pre-merge
/// verification.
///
/// One judgment leg runs the brief's § Omnia pre-merge gate (cargo fmt /
/// clippy / workspace check / test / wasm32 build — agent-run in the
/// lent workspace) over the folded delta and answers with the report;
/// the deterministic report-coherence gate then runs in core, with one
/// bounded repair leg — mirroring the contracts merge shape.
///
/// # Errors
///
/// Returns [`Error::InvalidRequest`] when the model rejects a request as
/// malformed, and [`Error::Internal`] for other model failures or an
/// answer that does not deserialize.
pub async fn merge<P: Model>(
    model: &P, ctx: &Context<'_>, slice: &str, delta: &Changeset, tree: &WorkingTree,
) -> Result<Report, Error> {
    let tree_root = ctx.tree_root(tree);
    let merge_brief = registry::body("briefs/merge.md");
    let delta_block = render_delta(delta);

    let user = format!(
        "Merge slice `{slice}`'s built delta (adapter `{}`). The project workspace is \
         lent to you; the delta below applies against base `{}` (a 3-way merge: the \
         baseline is ours, the delta is theirs). Fold the changes in place, then run \
         the merge brief's `## § Omnia pre-merge gate` yourself — the cargo / clippy \
         / test / wasm32-wasip2 commands run in the lent workspace; this adapter \
         cannot spawn them. Any gate failure means `status: failure`. Answer with the \
         report body. {SHELF_POINTER}\n\n{delta_block}",
        ctx.adapter_id, delta.base,
    );
    let report = report_call(model, ctx, merge_brief.to_string(), user).await?;

    gate_report(model, ctx, merge_brief, report, &tree_root, "merge").await
}

/// Assemble a system prompt from embedded brief bodies, separated the
/// way the contracts template separates orchestrator and sub-brief.
fn assemble_system(briefs: &[&str]) -> String {
    briefs.iter().map(|brief| registry::body(brief)).collect::<Vec<_>>().join("\n\n---\n\n")
}

/// Issue one internal phase leg through the shared judgment helper.
async fn phase_call<P: Model>(
    model: &P, ctx: &Context<'_>, system: String, user: String, name: &str,
) -> Result<PhaseAnswer, Error> {
    judgment(model, ctx, system, user, name, PHASE_ANSWER_SCHEMA).await
}

/// Issue one report leg gated by the derived answer schema and project
/// the answer onto the seam-facing report.
async fn report_call<P: Model>(
    model: &P, ctx: &Context<'_>, system: String, user: String,
) -> Result<Report, Error> {
    judgment::<P, ReportAnswer>(model, ctx, system, user, "report", REPORT_ANSWER_SCHEMA)
        .await
        .map(ReportAnswer::into_report)
}

/// The deterministic report-coherence gate after the report answer
/// lands, with one bounded repair leg — the omnia counterpart of the
/// contracts validator gate, limited to what pure Rust over the mounted
/// tree can check. When a `success` report declares outputs the tree
/// does not contain, one repair leg gets the discrepancies; residual
/// discrepancies then force `failure` regardless of the answer.
async fn gate_report<P: Model>(
    model: &P, ctx: &Context<'_>, brief: &str, mut report: Report, tree_root: &Path,
    operation: &str,
) -> Result<Report, Error> {
    let mut missing = missing_outputs(&report, tree_root);
    if !missing.is_empty() {
        let user = format!(
            "The deterministic report gate rejected the {operation} report: it claims \
             `status: success` but declares outputs the working tree does not \
             contain.\n\n{}\n\n\
             Repair the working tree or correct the report, then answer with the \
             corrected report body.",
            missing.join("\n"),
        );
        report = report_call(model, ctx, brief.to_string(), user).await?;
        missing = missing_outputs(&report, tree_root);
    }
    Ok(enforce_coherence(report, &missing))
}

/// The declared outputs a `success` report claims that the mounted tree
/// does not contain, one findings-style line each. A `failure` report is
/// already parked for human review per the briefs' stop contract, so its
/// output claims are not re-litigated.
fn missing_outputs(report: &Report, tree_root: &Path) -> Vec<String> {
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

/// Deterministic guard after the final answer lands: residual output
/// discrepancies force `failure` and are appended to the report; a
/// `success` answer carrying blocking findings is downgraded the same
/// way.
fn enforce_coherence(mut report: Report, missing: &[String]) -> Report {
    if !missing.is_empty() {
        report.status = Status::Failure;
        report.findings.extend(missing.iter().map(|detail| Finding {
            rule_id: None,
            severity: Severity::Important,
            detail: detail.clone(),
        }));
    }
    if report.status == Status::Success
        && report.findings.iter().any(|finding| finding.severity.blocking())
    {
        report.status = Status::Failure;
    }
    report
}

/// Render one phase leg's outcome for the report prompt.
fn render_phase_outcome((name, answer): (&str, &PhaseAnswer)) -> String {
    format!(
        "- {name}: applicable={}, wrote {:?} — {}",
        answer.applicable, answer.written, answer.summary
    )
}

/// Render the typed inputs as labeled prompt sections.
fn render_inputs(inputs: &[Input]) -> String {
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
fn render_delta(delta: &Changeset) -> String {
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
