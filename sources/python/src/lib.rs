//! Extracts behavioural claims from Python source.
//!
//! The source is a workspace or an inline value. A workspace's production
//! modules are its `.py` files outside tests, stubs, virtual environments,
//! caches, build output, and migrations. An inline value is mined whole, in
//! one call, with no survey.
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
//! The bootstrap is the adapter's own surface, `start`, and is never the
//! model's to name. The answer is held to the tree before anything rests on
//! it. From the accepted anchors the code derives the rest: the stem each
//! surface's `requirement` and `criterion` ids lead with, the id that tells
//! it from the other surfaces under that stem, an exported class's public
//! methods, and the modules it reaches.
//!
//! The seams follow the surfaces. A tree within the SDK's inline budget is
//! one call over every module. A larger tree is one call per stem, over the
//! modules the surfaces under it reach; where one of those imports what the
//! tree does not hold, the rest of the tree is laid after them. Each call is
//! told its surfaces, the boundaries its modules spell as values of their
//! own, the packages they import, the calls made through those packages,
//! the data files they name, and what the tree's own tests state.
//!
//! The `type` claims are copied from the declarations, never answered by
//! the model: every public class, type alias, and enumeration the seams'
//! modules declare at module level, as its header and member signatures. A
//! name led by an underscore, or left out of a declared `__all__`, is not
//! public.
//!
//! A workspace with no production module is refused. One whose survey names
//! no surface is mined under a mechanical cut, by package or by top-level
//! directory, and read as a library is. A module the parser cannot read is
//! walked as far as it got and never fails the run.

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

    // An inline value is one seam at once; a workspace's surfaces are named
    // in one turn from what the parser read, and its seams cut from them.
    async fn surveyed<P: Model>(ctx: &Context<'_, P>) -> Result<Survey, Error> {
        match survey::prepare(ctx.input)? {
            survey::Preparation::Value(survey) => Ok(survey),
            survey::Preparation::Workspace(tree) => {
                emery_sdk::survey::seams(ctx, PROSE, &tree).await
            }
        }
    }

    async fn extract<P: Model>(ctx: &Context<'_, P>) -> Result<Evidence, Error> {
        let Survey { seams, types } = surveyed(ctx).await?;
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
