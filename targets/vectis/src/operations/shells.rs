//! Declared-platform shell legs: the closed iOS / Android leg table,
//! the fail-closed `project.yaml.platforms` scope resolution, and the
//! per-shell write-leg runner.

use std::path::Path;

use adapter::seam::{Context, Error};
use adapter::{Model, phase};

use super::{BINDING_NOTE, REFERENCES_POINTER, assemble};

pub(super) struct ShellLeg {
    pub(super) name: &'static str,
    write_prompt: &'static str,
    pub(super) review_prompt: &'static str,
}

const SHELL_LEGS: [ShellLeg; 2] = [
    ShellLeg {
        name: "ios",
        write_prompt: "prompts/build/ios/write.md",
        review_prompt: "prompts/build/ios/review.md",
    },
    ShellLeg {
        name: "android",
        write_prompt: "prompts/build/android/write.md",
        review_prompt: "prompts/build/android/review.md",
    },
];

/// The shell legs the project's declared platform set puts in scope.
/// `web` / `desktop` have no prompt and never match.
///
/// Fails closed (A15): a missing or unreadable
/// `project.yaml.platforms` declaration is an error the build surfaces
/// as a blocking finding — never a guessed both-shells set.
///
/// # Errors
///
/// The loader's detail line when the declaration cannot be resolved.
pub(super) fn declared_shell_legs(project_root: &Path) -> Result<Vec<&'static ShellLeg>, String> {
    let declared = crate::validate::engine::load_shell_platforms(project_root)?;
    Ok(SHELL_LEGS.iter().filter(|leg| declared.iter().any(|p| p == leg.name)).collect())
}

/// Run the write leg for every declared shell, in table order.
///
/// DX files stay consistent with `$TEMPLATE_DIR` after identity
/// substitution; the guest does not re-render them from embedded
/// templates (sibling checkout is outside the project mount).
pub(super) async fn run_write_legs<P: Model>(
    model: &P, ctx: &Context<'_>, slice: &str, legs: &[&'static ShellLeg], scaffold_block: &str,
) -> Result<Vec<(&'static str, phase::PhaseAnswer)>, Error> {
    let mut outcomes = Vec::new();
    for shell in legs {
        let system = assemble(&["prompts/build.md", shell.write_prompt]);
        let user = format!(
            "Run the {name} shell write phase of the vectis build for slice `{slice}`: \
             generate or update the shell per the write prompt. When the \
             template-materialize prelude below lists absent trees, materialize from \
             `$TEMPLATE_DIR` on the host FS first — do not hand-invent scaffold \
             boilerplate or version pins. This is a generation-only pass: do not run \
             a verify-repair loop, `make build`, or write any `.vectis/verify.ok` \
             stamp — the engine dispatches a separate verify operation. Keep DX \
             files (Makefiles, `project.yml`, assembly Gradle files, BoltFFI pack \
             recipes) consistent with `$TEMPLATE_DIR` after identity substitution; \
             refresh by re-copying those paths from the template, never by guessing \
             pins. When the slice introduces no work for this shell, write nothing \
             and answer with `applicable: false`; when a host prerequisite is \
             missing, stop per the prompt's deferred contract and report it in your \
             summary.\n\n{BINDING_NOTE}\n\n{scaffold_block}\n\n{REFERENCES_POINTER}",
            name = shell.name,
        );
        let answer = phase::phase(model, ctx, system, user, shell.name).await?;
        outcomes.push((shell.name, answer));
    }
    Ok(outcomes)
}
