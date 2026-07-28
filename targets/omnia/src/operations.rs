//! `guidance` / `build` / `merge` over shared [`phase`] scaffolding.
//!
//! Build: generation → review → replay (self-skips without `captures`) →
//! report, then a report-coherence gate.

use std::path::Path;

use adapter::registry::Doc;
use adapter::seam::{
    Context, Error, Finding, Input, MergePhase, Report, TargetMetadata, WorkingTree,
};
use adapter::{AdapterIdentity, Model, Target, phase};

use crate::registry;

const REFERENCES_POINTER: &str = "Every prompt, reference, and rule document this adapter ships is \
     served by the granted `omnia-references` MCP references (`list_docs` / `read_doc`, adapter-relative \
     paths like `references/guardrails.md`); fetch documents the prompts cite lazily from there.";

/// Rust crates, tests, and guest scaffolding for Omnia deployments.
#[derive(Clone, Copy, Debug)]
pub struct Adapter;

impl Target for Adapter {
    const IDENTITY: AdapterIdentity = AdapterIdentity {
        name: "omnia",
        version: env!("CARGO_PKG_VERSION"),
    };

    fn metadata() -> TargetMetadata {
        TargetMetadata {
            emery_floor: Some("0.28.0".to_string()),
            inputs: Vec::new(),
            platforms: None,
        }
    }

    fn docs() -> &'static [Doc] {
        registry::docs()
    }

    async fn guidance<P: Model>(_model: &P, _ctx: &Context<'_>) -> Result<String, Error> {
        Ok(registry::body("prompts/guidance.md").to_string())
    }

    async fn build<P: Model>(
        model: &P, ctx: &Context<'_>, slice: &str, inputs: &[Input], tree: &WorkingTree,
    ) -> Result<Report, Error> {
        let tree_root = ctx.tree_root(tree);
        let inputs_block = phase::render_inputs(inputs);
        let build_prompt = registry::body("prompts/build.md");

        // Deterministic base-repo prelude: fill any missing standard
        // tooling file from the embedded templates before the model runs.
        // An I/O failure aborts the build here — the agent must never
        // recreate deterministic files from prose.
        let scaffold_block = scaffold_prelude(&tree_root)?;

        // Writer prompts share one system channel: verify-repair re-enters
        // the owning writer, so one leg must hold crate / test / guest together.
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
         The project workspace is lent to you. First prepare the read-only exemplar \
         checkout at `target/omnia-exemplar/` per the build prompt's `## § Exemplar \
         checkout` (clone/refresh `main`; stop with a stop hint if no checkout can be \
         obtained). Then detect create vs update mode per the \
         build prompt's `## Mode detection`, follow the crate-writer, test-writer, and \
         (create mode only) guest-writer prompts, then run the build prompt's \
         `## § Verify-repair loop` yourself — the cargo / clippy / test commands run \
         in the lent workspace; this adapter cannot spawn them. \
         {REFERENCES_POINTER}\n\n{scaffold_block}\n\n{inputs_block}",
            ctx.adapter_id,
        );
        let generation = phase::phase(model, ctx, system, user, "generation").await?;

        // Review remediation may re-enter the writers and verify-repair.
        let system = assemble(&["prompts/build.md", "prompts/build/review.md"]);
        let user = format!(
            "Run the standards-review leg of the omnia build for slice `{slice}`: \
         spawn the review team per the review prompt, synthesise `REVIEW.md`, and \
         drive the remediation cycle — re-running the verify-repair loop's cargo \
         commands in the lent workspace where the review prompt calls for it. \
         {REFERENCES_POINTER}",
        );
        let review = phase::phase(model, ctx, system, user, "review").await?;

        // Applicability is workspace-local; the leg self-skips when unbound.
        let system = assemble(&["prompts/build.md", "prompts/build/replay.md"]);
        let user = format!(
            "Run the capture-replay leg of the omnia build for slice `{slice}`. \
         When the slice has no `captures` source binding in `plan.yaml`, write \
         nothing and answer with `applicable: false` — omission when unbound is not \
         an error. {REFERENCES_POINTER}",
        );
        let replay = phase::phase(model, ctx, system, user, "replay").await?;

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

    async fn merge<P: Model>(
        model: &P, ctx: &Context<'_>, slice: &str, phase: MergePhase, tree: &WorkingTree,
    ) -> Result<Report, Error> {
        // No merged-baseline validator — postflight is deterministic success.
        if phase == MergePhase::Postflight {
            return Ok(Report::success());
        }

        let tree_root = ctx.tree_root(tree);
        let merge_prompt = registry::body("prompts/merge.md");

        let user = format!(
            "Run the preflight merge gate for slice `{slice}` (adapter `{}`). The project \
         workspace is lent to you; the build already wrote the slice's code in place, \
         and the engine folds the slice's spec deltas only after this gate passes. Run \
         the merge prompt's `## § Omnia pre-merge gate` yourself — the cargo / clippy \
         / test / wasm32-wasip2 commands run in the lent workspace; this adapter \
         cannot spawn them. Any gate failure means `status: failure`. Answer with the \
         report body. {REFERENCES_POINTER}",
            ctx.adapter_id,
        );
        let report = phase::report(model, ctx, merge_prompt.to_string(), user).await?;

        gate_report(model, ctx, merge_prompt, report, &tree_root, "merge-preflight").await
    }
}

fn assemble(prompts: &[&str]) -> String {
    let bodies: Vec<&str> = prompts.iter().map(|prompt| registry::body(prompt)).collect();
    phase::assemble_system(&bodies)
}

// A scaffold I/O failure fails the build before model generation: the
// templates are deterministic, so asking the agent to recreate them
// from prose would only produce weaker copies.
fn scaffold_prelude(tree_root: &Path) -> Result<String, Error> {
    use std::fmt::Write as _;

    let report = crate::scaffold::ensure_missing(tree_root)
        .map_err(|err| Error::Io(format!("base-repo scaffold prelude failed: {err}")))?;

    if report.written.is_empty() {
        return Ok("### scaffold prelude (already run in-guest)\n\nEvery standard tooling file \
             was already present; nothing was written. Do not re-author them."
            .to_string());
    }

    let mut block = format!(
        "### scaffold prelude (already run in-guest)\n\nThe adapter wrote the missing \
         standard tooling files from its embedded templates:\n{}\n\nDo not re-author or \
         overwrite them.",
        report.written.iter().map(|path| format!("- `{path}`")).collect::<Vec<_>>().join("\n"),
    );
    if report.written.contains(&crate::scaffold::PUBLISH_WORKFLOW) {
        let tokens = crate::scaffold::publish_placeholders()
            .iter()
            .map(|token| format!("`{token}`"))
            .collect::<Vec<_>>()
            .join(" / ");
        let _ = write!(
            block,
            " Fill the {tokens} placeholders in `{}`.",
            crate::scaffold::PUBLISH_WORKFLOW,
        );
    }
    if report.written.contains(&crate::scaffold::VET_CONFIG) {
        block.push_str(
            " Run `cargo vet regenerate {imports,exemptions,unpublished}` once \
             `Cargo.lock` exists.",
        );
    }
    Ok(block)
}

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
