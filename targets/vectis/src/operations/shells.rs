//! Declared-platform shell legs: the closed iOS / Android leg table,
//! the `project.yaml.platforms` read that scopes them, and the
//! per-shell write-leg runner.

use std::path::Path;

use adapter::seam::{Context, Error};
use adapter::{Model, phase};
use serde_json::Value;

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

// Absent / unreadable `project.yaml.platforms` → both shells; `web` /
// `desktop` have no prompt and never match.
pub(super) fn declared_shell_legs(project_root: &Path) -> Vec<&'static ShellLeg> {
    let declared = declared_platforms(project_root);
    SHELL_LEGS
        .iter()
        .filter(|leg| declared.as_ref().is_none_or(|set| set.iter().any(|p| p == leg.name)))
        .collect()
}

fn declared_platforms(project_root: &Path) -> Option<Vec<String>> {
    let source = std::fs::read_to_string(project_root.join(".emery/project.yaml")).ok()?;
    let doc: Value = serde_saphyr::from_str(&source).ok()?;
    let platforms = doc.get("platforms")?.as_array()?;
    Some(platforms.iter().filter_map(Value::as_str).map(str::to_string).collect())
}

/// Run the write leg for every declared shell, in table order.
///
/// DX files stay consistent with `$TEMPLATE_DIR` after identity
/// substitution; the guest does not re-render them from embedded
/// templates (sibling checkout is outside the project mount).
pub(super) async fn run_write_legs<P: Model>(
    model: &P, ctx: &Context<'_>, slice: &str, change_root: &Path, scaffold_block: &str,
) -> Result<Vec<(&'static str, phase::PhaseAnswer)>, Error> {
    let mut outcomes = Vec::new();
    for shell in declared_shell_legs(change_root) {
        let system = assemble(&["prompts/build.md", shell.write_prompt]);
        let user = format!(
            "Run the {name} shell phase of the vectis build for slice `{slice}`: \
             generate or update the shell per the write prompt. When the \
             template-materialize prelude below lists absent trees, materialize from \
             `$TEMPLATE_DIR` on the host FS first — do not hand-invent scaffold \
             boilerplate or version pins. Then run the write prompt's \
             orchestrator-owned verify loop yourself in the lent workspace — this \
             adapter cannot spawn host commands. Keep DX files (Makefiles, \
             `project.yml`, assembly Gradle files, BoltFFI pack recipes) consistent \
             with `$TEMPLATE_DIR` after identity substitution; refresh by re-copying \
             those paths from the template, never by guessing pins. When the slice \
             introduces no work for this shell, write nothing \
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
