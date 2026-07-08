//! The judgment operation template: `guidance`, `build`, and `merge`,
//! over the shared [`phase`] scaffolding.
//!
//! `build` runs a generation leg (crate / test / guest writers plus
//! the agent-run cargo verify-repair loop), a review leg, a replay leg
//! (self-skipping when no `captures` source is bound), then a report
//! leg gated by the report-coherence check: declared `outputs` paths
//! exist and a `success` report carries no blocking findings.

use std::path::Path;

use adapter::seam::{
    Changeset, Context, Error, Finding, Input, Report, TargetManifest, WorkingTree,
};
use adapter::{Model, phase};

use crate::registry;

/// Pointer to the adapter's MCP references carried by every leg's user
/// prompt, so the agent fetches specialist material lazily.
const REFERENCES_POINTER: &str = "Every prompt, reference, and rule document this adapter ships is \
     served by the granted `omnia-references` MCP references (`list_docs` / `read_doc`, adapter-relative \
     paths like `references/guardrails.md`); fetch documents the prompts cite lazily from there.";

/// Resolve-time `describe` metadata: no compatibility floor, no
/// declared build inputs (omnia reads the working tree's `Cargo.toml`
/// directly, not a slice-tree input), no platform capability.
#[must_use]
pub const fn describe() -> TargetManifest {
    TargetManifest {
        specify_floor: None,
        inputs: Vec::new(),
        platforms: None,
    }
}

/// The embedded guidance prompt (no judgment leg).
#[must_use]
pub fn guidance() -> &'static str {
    registry::body("prompts/guidance.md")
}

/// Build a slice's crate, tests, and guest scaffolding.
///
/// A generation leg, a review leg, a replay leg (self-skipping when no
/// `captures` source is bound), a report leg, then the
/// report-coherence gate with one bounded repair leg.
///
/// # Errors
///
/// As [`adapter::judgment`].
pub async fn build<P: Model>(
    model: &P, ctx: &Context<'_>, slice: &str, inputs: &[Input], tree: &WorkingTree,
) -> Result<Report, Error> {
    let tree_root = ctx.tree_root(tree);
    let inputs_block = phase::render_inputs(inputs);
    let build_prompt = registry::body("prompts/build.md");

    // The writer prompts ride in one system channel because the
    // verify-repair loop crosses all three: a cargo failure re-enters
    // the owning writer prompt, so one agent leg must hold them together.
    let system = assemble(&[
        "prompts/build.md",
        "prompts/guidance.md",
        "prompts/build/crate.md",
        "prompts/build/test.md",
        "prompts/build/guest.md",
    ]);
    let user = format!(
        "Run the generation leg of the omnia build for slice `{slice}` \
         (adapter `{}`).\n\n\
         The project workspace is lent to you. Detect create vs update mode per the \
         build prompt's `## Mode detection`, follow the crate-writer, test-writer, and \
         (create mode only) guest-writer prompts, then run the build prompt's \
         `## § Verify-repair loop` yourself — the cargo / clippy / test commands run \
         in the lent workspace; this adapter cannot spawn them. {REFERENCES_POINTER}\n\n\
         {inputs_block}",
        ctx.adapter_id,
    );
    let generation = phase::phase(model, ctx, system, user, "generation").await?;

    // The review remediation cycle may re-enter the writer prompts and
    // the verify-repair loop with tighter caps.
    let system = assemble(&["prompts/build.md", "prompts/build/review.md"]);
    let user = format!(
        "Run the standards-review leg of the omnia build for slice `{slice}`: \
         spawn the review team per the review prompt, synthesise `REVIEW.md`, and \
         drive the remediation cycle — re-running the verify-repair loop's cargo \
         commands in the lent workspace where the review prompt calls for it. \
         {REFERENCES_POINTER}",
    );
    let review = phase::phase(model, ctx, system, user, "review").await?;

    // Only the workspace knows whether a `captures` source is bound,
    // so the leg judges applicability itself and self-skips.
    let system = assemble(&["prompts/build.md", "prompts/build/replay.md"]);
    let user = format!(
        "Run the capture-replay leg of the omnia build for slice `{slice}`. \
         When the slice has no `captures` source binding in `plan.yaml`, write \
         nothing and answer with `applicable: false` — omission when unbound is not \
         an error. {REFERENCES_POINTER}",
    );
    let replay = phase::phase(model, ctx, system, user, "replay").await?;

    // Report answer, gated by the answer schema.
    let user = format!(
        "Write the build report for slice `{slice}` per the build prompt's \
         `## Build report`. First mark the completed `tasks.md` checkboxes in the \
         slice directory per the build prompt. A `success` report carries only non-blocking \
         findings; an exhausted verify-repair budget or unresolved blocking review \
         findings mean `status: failure`. Declare the slice's crate tree (and the \
         guest scaffolding, when this build wrote it) as `platform: core` outputs \
         with paths relative to the project root.\n\n\
         Phase outcomes:\n{}",
        [("generation", &generation), ("review", &review), ("replay", &replay)]
            .map(|(name, answer)| phase::render_outcome(name, answer))
            .join("\n"),
    );
    let report = phase::report(model, ctx, build_prompt.to_string(), user).await?;

    gate_report(model, ctx, build_prompt, report, &tree_root, "build").await
}

/// Gate a built slice's landing on the merge prompt's pre-merge
/// verification.
///
/// One judgment leg folds the delta and runs the merge prompt's
/// § Omnia pre-merge gate (agent-run cargo / clippy / test / wasm32 in
/// the lent workspace), then the report-coherence gate runs in core.
///
/// # Errors
///
/// As [`adapter::judgment`].
pub async fn merge<P: Model>(
    model: &P, ctx: &Context<'_>, slice: &str, delta: &Changeset, tree: &WorkingTree,
) -> Result<Report, Error> {
    let tree_root = ctx.tree_root(tree);
    let merge_prompt = registry::body("prompts/merge.md");
    let delta_block = phase::render_delta(delta);

    let user = format!(
        "Merge slice `{slice}`'s built delta (adapter `{}`). The project workspace is \
         lent to you; the delta below applies against base `{}` (a 3-way merge: the \
         baseline is ours, the delta is theirs). Fold the changes in place, then run \
         the merge prompt's `## § Omnia pre-merge gate` yourself — the cargo / clippy \
         / test / wasm32-wasip2 commands run in the lent workspace; this adapter \
         cannot spawn them. Any gate failure means `status: failure`. Answer with the \
         report body. {REFERENCES_POINTER}\n\n{delta_block}",
        ctx.adapter_id, delta.base,
    );
    let report = phase::report(model, ctx, merge_prompt.to_string(), user).await?;

    gate_report(model, ctx, merge_prompt, report, &tree_root, "merge").await
}

fn assemble(prompts: &[&str]) -> String {
    let bodies: Vec<&str> = prompts.iter().map(|prompt| registry::body(prompt)).collect();
    phase::assemble_system(&bodies)
}

/// Report-coherence gate with one bounded repair leg: when a `success`
/// report declares outputs the tree does not contain, one repair leg
/// gets the discrepancies; residual discrepancies force `failure`.
async fn gate_report<P: Model>(
    model: &P, ctx: &Context<'_>, prompt: &str, mut report: Report, tree_root: &Path,
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
        report = phase::report(model, ctx, prompt.to_string(), user).await?;
        missing = phase::missing_outputs(&report, tree_root);
    }
    Ok(phase::enforce(report, missing.into_iter().map(Finding::blocking).collect()))
}
