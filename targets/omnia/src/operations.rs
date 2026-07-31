//! `guidance` / `build` / `merge` over shared [`phase`] scaffolding.
//!
//! Build: preparation (exemplar checkout) → deterministic scaffold →
//! generation → replay (dispatched only when the build context binds
//! `captures`) → review (closes the build: findings synthesis, output
//! declaration), then the in-guest report assembly and its
//! deterministic gate (RFC-78 D6).

use std::path::Path;

use adapter::registry::Doc;
use adapter::seam::{
    BuildContext, Context, Error, Finding, Input, MergePhase, Report, TargetMetadata, WorkingTree,
};
use adapter::{AdapterIdentity, Model, Target, phase};

use crate::registry;
use crate::review::{REVIEW_ANSWER_SCHEMA, ReviewAnswer};

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
            emery_floor: Some("0.35.0".to_string()),
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
        model: &P, ctx: &Context<'_>, slice: &str, inputs: &[Input], context: &BuildContext,
        tree: &WorkingTree,
    ) -> Result<Report, Error> {
        let tree_root = ctx.tree_root(tree);
        let inputs_block = phase::render_inputs(inputs);

        // The agent prepares the read-only exemplar checkout the rest of
        // the build reads: scaffold templates, worked code, and the Omnia
        // compatibility contract all come from it.
        let system = assemble(&["prompts/build.md", "prompts/build/prepare.md"]);
        let user = format!(
            "Run the preparation leg of the omnia build for slice `{slice}` \
         (adapter `{}`): prepare the read-only exemplar checkout at \
         `target/omnia-exemplar/` in the lent workspace per the preparation prompt — \
         clone or refresh unpinned `main`, keep an existing checkout when only the \
         refresh fails (note the staleness in your summary), and surface the stop \
         hint per the build prompt's `## § Stop hint contract` in your summary when \
         no checkout can be obtained. {REFERENCES_POINTER}",
            ctx.adapter_id,
        );
        let preparation = phase::phase(model, ctx, system, user, "preparation").await?;

        // Deterministic base-repo prelude: strictly validate the prepared
        // checkout's template contract, then fill any missing standard
        // tooling file from it. A missing or malformed checkout or an I/O
        // failure aborts the build here — the agent must never recreate
        // deterministic files from prose.
        let scaffold_block = scaffold_prelude(&tree_root)?;

        // Writer prompts share one system channel: verify-repair re-enters
        // the owning writer, so one leg must hold crate / test / guest together.
        // `guidance.md` stays on the `guidance` operation only — its idioms
        // were folded into the artifacts at refine (RFC-78 D2). The guest
        // writer ships only in create mode: its own header loads it on first
        // build only, keyed on the workspace-root `src/lib.rs` the prelude's
        // tree walk already sees.
        let create_mode = !tree_root.join("src").join("lib.rs").is_file();
        let mut generation_prompts =
            vec!["prompts/build.md", "prompts/build/crate.md", "prompts/build/test.md"];
        if create_mode {
            generation_prompts.push("prompts/build/guest.md");
        }
        let system = assemble(&generation_prompts);
        let user = format!(
            "Run the generation leg of the omnia build for slice `{slice}` \
         (adapter `{}`).\n\n\
         The project workspace is lent to you. The read-only exemplar checkout at \
         `target/omnia-exemplar/` is already prepared and validated — read it per \
         `references/exemplar.md`. Detect create vs update mode per the \
         build prompt's `## Mode detection`, follow the crate-writer, test-writer, and \
         (create mode only) guest-writer prompts, then run the build prompt's \
         `## § Verify-repair loop` yourself — the cargo / clippy / test commands run \
         in the lent workspace; this adapter cannot spawn them. Guidance idioms were \
         already folded into the slice artifacts at refine; re-read `design.md` and the \
         specs, and fetch `references/guardrails.md` via MCP if needed. \
         {REFERENCES_POINTER}\n\n{scaffold_block}\n\n{inputs_block}",
            ctx.adapter_id,
        );
        let generation = phase::phase(model, ctx, system, user, "generation").await?;

        // Whether the slice binds `captures` is deterministic — the
        // engine forwards the bound source names on the build context —
        // so the leg is dispatched only when bound; no spawn exists to
        // answer `applicable: false` (RFC-78 D6). It runs before review
        // so the review's findings synthesis can fold replay failures.
        let replay = if context.sources.iter().any(|source| source == "captures") {
            let system = assemble(&["prompts/build.md", "prompts/build/replay.md"]);
            let user = format!(
                "Run the capture-replay leg of the omnia build for slice `{slice}` — \
             the slice binds the `captures` source. Follow the replay prompt and \
             classify results in your summary; the standards review folds unresolved \
             replay failures into its findings. {REFERENCES_POINTER}",
            );
            Some(phase::phase(model, ctx, system, user, "replay").await?)
        } else {
            None
        };

        // Review closes the build: remediation may re-enter the writers
        // and verify-repair, then the answer carries the absorbed report
        // residue (tasks.md checkboxes, findings synthesis, output
        // declaration) — no separate report leg is spawned (RFC-78 D6).
        let mut outcomes = vec![
            phase::render_outcome("preparation", &preparation),
            phase::render_outcome("generation", &generation),
        ];
        outcomes.push(replay.as_ref().map_or_else(
            || "- replay: skipped in-guest — the slice binds no `captures` source".to_string(),
            |answer| phase::render_outcome("replay", answer),
        ));
        let system = assemble(&["prompts/build.md", "prompts/build/review.md"]);
        let user = format!(
            "Run the standards-review leg of the omnia build for slice `{slice}`: \
         spawn the review team per the review prompt, synthesise `REVIEW.md`, and \
         drive the remediation cycle — re-running the verify-repair loop's cargo \
         commands in the lent workspace where the review prompt calls for it. \
         Then close out the build per the review prompt's `## Build close-out`: \
         mark the completed `tasks.md` checkboxes in the slice directory, declare \
         the slice's crate tree (and the guest scaffolding, when this build wrote \
         it) as `platform: core` outputs with paths relative to the project root, \
         and synthesise the findings left unresolved after remediation into your \
         answer — the adapter assembles the build report from it in-guest; there \
         is no separate report leg. A build that cannot succeed (an exhausted \
         verify-repair budget, unresolved blocking review findings) must carry at \
         least one blocking (`critical` / `important`) finding. \
         {REFERENCES_POINTER}\n\nPhase outcomes:\n{}",
            outcomes.join("\n"),
        );
        let review: ReviewAnswer =
            adapter::judgment(model, ctx, system, user, "review", REVIEW_ANSWER_SCHEMA).await?;

        // Deterministic report assembly and gate — the same checks the
        // engine re-runs (enforce_no_blocking / enforce_outputs_exist).
        // A declared-but-missing output is a blocking finding, not a
        // repair re-prompt: the review agent declared paths it just
        // verified, so a miss parks the slice for human review.
        let report = review.into_report();
        let missing = phase::missing_outputs(&report, &tree_root);
        Ok(phase::enforce(report, missing.into_iter().map(Finding::blocking).collect()))
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

// A scaffold failure fails the build before model generation: the
// templates are deterministic, so asking the agent to recreate them
// from prose would only produce weaker copies. A missing or malformed
// checkout is equally fatal — the preparation leg must be repaired, not
// papered over.
fn scaffold_prelude(tree_root: &Path) -> Result<String, Error> {
    use std::fmt::Write as _;

    let report = crate::scaffold::ensure_missing(tree_root)
        .map_err(|err| Error::Io(format!("base-repo scaffold prelude failed: {err}")))?;

    let mut block = if report.written.is_empty() {
        "### scaffold prelude (already run in-guest)\n\nEvery standard tooling file \
         was already present; nothing was written. Do not re-author them."
            .to_string()
    } else {
        format!(
            "### scaffold prelude (already run in-guest)\n\nThe adapter wrote the missing \
             standard tooling files from the exemplar checkout's template contract:\n{}\n\n\
             Do not re-author or overwrite them.",
            report.written.iter().map(|path| format!("- `{path}`")).collect::<Vec<_>>().join("\n"),
        )
    };
    if !report.unfilled_tokens.is_empty() {
        let tokens = report
            .unfilled_tokens
            .iter()
            .map(|token| format!("`{token}`"))
            .collect::<Vec<_>>()
            .join(" / ");
        let _ = write!(
            block,
            "\n\nUnfilled placeholders still present in `{}`: {tokens}. Fill them before \
             considering the guest scaffolding complete.",
            crate::scaffold::PUBLISH_WORKFLOW,
        );
    }
    if report.written.iter().any(|path| path == crate::scaffold::VET_CONFIG) {
        block.push_str(
            "\n\nRun `cargo vet regenerate {imports,exemptions,unpublished}` once \
             `Cargo.lock` exists.",
        );
    }
    if let Some(mismatch) = &report.pin_mismatch {
        let consumer_version = mismatch.consumer_version.as_deref().unwrap_or("(unparsed)");
        let consumer_rev = mismatch.consumer_rev.as_deref().unwrap_or("(unparsed)");
        let _ = write!(
            block,
            "\n\nSoft warning — Omnia pin mismatch (update mode): consumer \
             `omnia` version `{consumer_version}` / rev `{consumer_rev}` vs exemplar \
             `{}` / `{}`. Preserve the consumer pin; prefer consumer-evidenced idioms \
             over exemplar idioms wherever they conflict.",
            mismatch.exemplar_version, mismatch.exemplar_rev,
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
