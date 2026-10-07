//! Extracts behavioural claims from TypeScript and JavaScript source.
//!
//! The source is a workspace or an inline value. A workspace's production
//! modules are its `.ts`, `.tsx`, `.js`, and sibling files outside tests,
//! declarations, dependencies, and build output. An inline value is mined
//! whole, in one call, with no survey.
//!
//! # Survey
//!
//! A workspace is surveyed before it is mined. The parser reads the tree,
//! and one survey call has the model name the surfaces a caller enters the
//! source through, each at the lines that register or declare it:
//!
//! - a function handed to something a package provides: a route, a command,
//!   a job
//! - a method under a decorator a package provides
//! - what the entry modules export, where nothing is registered
//!
//! The bootstrap is the first entry `package.json` names, or a conventional
//! entry the tree holds, that runs something when loaded. It is the
//! adapter's own surface, `start`, and never the model's to name.
//!
//! The answer is held to the tree before anything rests on it:
//!
//! - every anchor names a module the tree holds
//! - no surface leads with `start`
//! - every module the facts locate a surface in is reached by a named
//!   surface or listed as unreached
//!
//! From the accepted anchors the code derives the rest:
//!
//! - the stem each surface's `requirement` and `criterion` ids lead with:
//!   the resource a route spells or a literal's first word, where the code
//!   spells one, else the survey's
//! - the id that tells a surface from the others under its stem: a
//!   handler's name, a route's verb and path, a method
//! - an exported class's public methods
//! - the modules the surface reaches
//!
//! # Seams
//!
//! The seams follow the surfaces:
//!
//! - a tree within the SDK's inline budget is one call over every module,
//!   held to every surface's stem
//! - a larger tree is one call per stem, over the modules its surfaces
//!   reach, held to that stem alone
//! - where a module imports one the tree does not hold, or loads one by a
//!   computed name, the modules of the directory the import leads into
//!   follow
//! - a tree whose survey names no surface is cut mechanically, by package
//!   or by top-level directory, and read as a library is
//!
//! Each call is told:
//!
//! - its surfaces, each with its id and what it reaches
//! - the boundaries its modules spell as values of their own: literals and
//!   expressions over them, patterns, `process.env` reads, and definitions
//!   handed literals, whether at module level, in a class, or as a
//!   constant-named local
//! - the packages they import, and the calls made through them, grouped by
//!   callee
//! - the `.json` files they import, laid as the seam's data
//! - what the tree's own tests state
//! - what could not be followed
//!
//! # Type claims
//!
//! The `type` claims are copied from the declarations, never answered by
//! the model: every `interface`, `type`, and `enum` the seams' modules
//! export, verbatim, and every exported class as its header and member
//! signatures. A declaration a module keeps to itself is none.
//!
//! # Refusals
//!
//! A workspace with no production module is refused. Nothing else is: a
//! module the parser cannot read is walked as far as it got and never fails
//! the run.

#[cfg(target_arch = "wasm32")]
mod survey;

#[cfg(target_arch = "wasm32")]
mod guest {
    use emery_sdk::survey::Survey;
    use emery_sdk::{AdapterMetadata, ClaimKind, Context, Error, Evidence, Model, SourceKind};

    use crate::{PROSE, survey};

    emery_sdk::source_adapter!(metadata, extract);

    fn metadata() -> AdapterMetadata {
        emery_sdk::metadata(SourceKind::Behaviour)
    }

    async fn extract<P: Model>(ctx: &Context<'_, P>) -> Result<Evidence, Error> {
        let Survey { seams, types } = survey::survey(ctx).await?;
        let mut evidence = emery_sdk::extract(ctx, PROSE, &seams).await?;

        // the declarations are the code's to state: what the model answered
        // as a `type` gives way to what the parser read
        let answered = evidence.claims.len();
        evidence.claims.retain(|claim| claim.kind != ClaimKind::Type);
        let dropped = answered - evidence.claims.len();
        if dropped > 0 {
            emery_sdk::tracing::debug!(
                dropped,
                "type claims the model answered give way to the parsed declarations"
            );
        }
        evidence.claims.extend(types);
        Ok(evidence)
    }
}

/// The prompts and reference documents embedded in the adapter.
pub static PROSE: &[emery_sdk::Doc] = emery_sdk::prose![
    "../prose/extract.md",
    "../prose/survey.md",
    "../prose/references/examples/README.md",
    "../prose/references/examples/branching-caching.md",
    "../prose/references/examples/outbound-http.md",
    "../prose/references/examples/parallel-execution.md",
];
