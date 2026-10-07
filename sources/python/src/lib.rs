//! Extracts behavioural claims from Python source.
//!
//! The source is a workspace or an inline value. A workspace's production
//! modules are its `.py` files outside tests, stubs, virtual environments,
//! caches, build output, and migrations. An inline value is mined whole, in
//! one call, with no survey.
//!
//! # Survey
//!
//! A workspace is surveyed before it is mined. The parser reads the tree,
//! and one survey call has the model name the surfaces a caller enters the
//! source through, each at the lines that register or declare it:
//!
//! - a handler handed to something a package provides, outside any handler
//!   of its own: a route, a command, a task, a consumer
//! - a function or class under a decorator a package provides
//! - what the entry modules export, where nothing is registered
//!
//! The bootstrap is the first module that runs when loaded: a console
//! script's target, a module under a `__main__` guard, or one constructing
//! an application at module level. It is the adapter's own surface,
//! `start`, and never the model's to name.
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
//! - the stem each surface's `requirement` and `criterion` ids lead with
//! - the id that tells a surface from the others under its stem
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
//! - where a module imports one the tree does not hold, the modules of the
//!   directory the import leads into follow
//! - a tree whose survey names no surface is cut mechanically, by package
//!   or by top-level directory, and read as a library is
//!
//! Each call is told:
//!
//! - its surfaces, each with its id and what it reaches
//! - the boundaries its modules spell as values of their own
//! - the packages they import, and the calls made through them
//! - the data files they name
//! - what the tree's own tests state
//! - what could not be followed
//!
//! # Type claims
//!
//! The `type` claims are copied from the declarations, never answered by
//! the model: every public class, type alias, and enumeration the seams'
//! modules declare at module level, as its header and member signatures. A
//! name led by an underscore, or left out of a declared `__all__`, is not
//! public.
//!
//! # Refusals
//!
//! A workspace with no production module is refused. Nothing else is: a
//! module the parser cannot read is walked as far as it got and never fails
//! the run.

#![expect(
    clippy::multiple_crate_versions,
    reason = "ruff's `itertools` and the SDK's host-side `prost-derive` pin different minors; \
              neither is this crate's to unify"
)]

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

        // replace the model's type claims with the parsed declarations
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
