//! The judgment operation template: `guidance`, `build`, and `merge`,
//! over the shared [`phase`] scaffolding.
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

use specify_guest_kit::seam::{Changeset, Context, Error, Finding, Input, Report, WorkingTree};
use specify_guest_kit::{Model, phase};

use crate::registry;

/// The pointer at the adapter's own MCP reference shelf every judgment
/// leg's user prompt carries, so prompts stay lean and the agent fetches
/// specialist material lazily instead of getting it inlined.
const SHELF_POINTER: &str = "Every brief, reference, and rule document this adapter ships is \
     served by the granted `omnia-references` MCP shelf (`list_docs` / `read_doc`, adapter-relative \
     paths like `references/guardrails.md`); fetch documents the briefs cite lazily from there.";

/// Guidance on the expected build artifacts for this target — the
/// embedded guidance brief, returned deterministically (no judgment leg).
#[must_use]
pub fn guidance() -> &'static str {
    registry::body("briefs/guidance.md")
}

/// Build a slice's crate, tests, and guest scaffolding per the build
/// brief's phase order.
///
/// A generation leg (phases 1–5), a review leg (phase 6), a replay leg
/// (phase 7, self-skipping when no `captures` source is bound), one
/// report leg, then the deterministic report-coherence gate with one
/// bounded repair leg.
///
/// # Errors
///
/// As [`specify_guest_kit::judgment`].
pub async fn build<P: Model>(
    model: &P, ctx: &Context<'_>, slice: &str, inputs: &[Input], tree: &WorkingTree,
) -> Result<Report, Error> {
    let tree_root = ctx.tree_root(tree);
    let inputs_block = phase::render_inputs(inputs);
    let build_brief = registry::body("briefs/build.md");

    // Phases 1–5 — generation: the guidance refresher plus the crate /
    // test / guest sub-briefs ride in the system channel because the
    // brief's verify-repair loop crosses all three (a cargo failure
    // re-enters the owning sub-brief), so one agent leg must hold them
    // together. Mode detection (create vs update) is the brief's own
    // § Mode detection, judged inside the leg against the lent workspace.
    let system = assemble(&[
        "briefs/build.md",
        "briefs/guidance.md",
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
    let generation = phase::phase(model, ctx, system, user, "generation").await?;

    // Phase 6 — standards review: its remediation cycle may re-enter the
    // crate / test sub-briefs and the verify-repair loop with tighter
    // caps; the specialist protocol and rule codex live on the shelf.
    let system = assemble(&["briefs/build.md", "briefs/build/review.md"]);
    let user = format!(
        "Run the standards-review phase (6) of the omnia build for slice `{slice}`: \
         spawn the review team per the review sub-brief, synthesise `REVIEW.md`, and \
         drive the remediation cycle — re-running the verify-repair loop's cargo \
         commands in the lent workspace where the sub-brief calls for it. \
         {SHELF_POINTER}",
    );
    let review = phase::phase(model, ctx, system, user, "review").await?;

    // Phase 7 — capture replay: conditional on a `captures` source
    // binding only the workspace knows about, so the leg itself judges
    // applicability and self-skips, like a contracts sub-flow with no
    // owned surface.
    let system = assemble(&["briefs/build.md", "briefs/build/replay.md"]);
    let user = format!(
        "Run the capture-replay phase (7) of the omnia build for slice `{slice}`. \
         When the slice has no `captures` source binding in `plan.yaml`, write \
         nothing and answer with `applicable: false` — omission when unbound is not \
         an error. {SHELF_POINTER}",
    );
    let replay = phase::phase(model, ctx, system, user, "replay").await?;

    // Final leg — the report answer, gated by the derived answer schema.
    let user = format!(
        "Write the build report for slice `{slice}` per the build brief's \
         `## Build report`. First mark the completed `tasks.md` checkboxes in the \
         slice directory per the brief. A `success` report carries only non-blocking \
         findings; an exhausted verify-repair budget or unresolved blocking review \
         findings mean `status: failure`. Declare the slice's crate tree (and the \
         guest scaffolding, when this build wrote it) as `platform: core` outputs \
         with paths relative to the project root.\n\n\
         Phase outcomes:\n{}",
        [("generation", &generation), ("review", &review), ("replay", &replay)]
            .map(|(name, answer)| phase::render_outcome(name, answer))
            .join("\n"),
    );
    let report = phase::report(model, ctx, build_brief.to_string(), user).await?;

    gate_report(model, ctx, build_brief, report, &tree_root, "build").await
}

/// Gate a built slice's landing on the merge brief's pre-merge
/// verification.
///
/// One judgment leg folds the delta and runs the brief's § Omnia
/// pre-merge gate (cargo fmt / clippy / workspace check / test / wasm32
/// build — agent-run in the lent workspace), then the deterministic
/// report-coherence gate runs in core.
///
/// # Errors
///
/// As [`specify_guest_kit::judgment`].
pub async fn merge<P: Model>(
    model: &P, ctx: &Context<'_>, slice: &str, delta: &Changeset, tree: &WorkingTree,
) -> Result<Report, Error> {
    let tree_root = ctx.tree_root(tree);
    let merge_brief = registry::body("briefs/merge.md");
    let delta_block = phase::render_delta(delta);

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
    let report = phase::report(model, ctx, merge_brief.to_string(), user).await?;

    gate_report(model, ctx, merge_brief, report, &tree_root, "merge").await
}

/// Assemble a system prompt from embedded brief bodies.
fn assemble(briefs: &[&str]) -> String {
    let bodies: Vec<&str> = briefs.iter().map(|brief| registry::body(brief)).collect();
    phase::assemble_system(&bodies)
}

/// The deterministic report-coherence gate after the report answer
/// lands, with one bounded repair leg — limited to what pure Rust over
/// the mounted tree can check: when a `success` report declares outputs
/// the tree does not contain, one repair leg gets the discrepancies, and
/// residual discrepancies force `failure` regardless of the answer.
async fn gate_report<P: Model>(
    model: &P, ctx: &Context<'_>, brief: &str, mut report: Report, tree_root: &Path,
    operation: &str,
) -> Result<Report, Error> {
    let mut missing = phase::missing_outputs(&report, tree_root);
    if !missing.is_empty() {
        let user = format!(
            "The deterministic report gate rejected the {operation} report: it claims \
             `status: success` but declares outputs the working tree does not \
             contain.\n\n{}\n\n\
             Repair the working tree or correct the report, then answer with the \
             corrected report body.",
            missing.join("\n"),
        );
        report = phase::report(model, ctx, brief.to_string(), user).await?;
        missing = phase::missing_outputs(&report, tree_root);
    }
    Ok(phase::enforce(report, missing.into_iter().map(Finding::blocking).collect()))
}
