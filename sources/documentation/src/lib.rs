//! Extracts claims from a tree of written documentation.
//!
//! The source is a workspace or an inline value. An inline value is mined
//! whole, in one call. A workspace is mined one call per group of
//! documents.
//!
//! # Groups
//!
//! A workspace is grouped by top-level directory, the root's own documents
//! one group beside them:
//!
//! - a directory of fewer than two documents joins the root's group
//! - the root's group joins the first directory's when it is itself fewer
//!   than two
//! - a directory of more than sixteen documents is cut once more, by its
//!   subdirectories under the same rule, its remaining documents one group
//!   beside them; a directory no subdirectory can cut stays one group
//! - if fewer than two groups remain, the whole workspace is one
//!
//! # Model survey
//!
//! Under the `model-survey` feature, an experiment arm and not the default,
//! a workspace is surveyed before it is mined. Every document's outline is
//! laid before the model, which names each subject the tree documents:
//!
//! - a feature, a resource, or a flow
//! - anchored at the heading or first line that introduces it
//! - under the domain noun the documents spell, as its stem
//!
//! A document introducing no subject is listed as unreached. From the
//! answer the code derives each subject's span and the id its claims lead
//! with, and cuts the seams: one call over every subject's document within
//! the SDK's inline budget, one per stem past it, each told its subjects
//! and their spans. A survey naming no subject falls back to the groups
//! above.

#[cfg(target_arch = "wasm32")]
mod survey;

#[cfg(target_arch = "wasm32")]
mod guest {
    use emery_sdk::{AdapterMetadata, Context, Error, Evidence, Model, Seam, SourceKind};

    use crate::{PROSE, survey};

    emery_sdk::source_adapter!(metadata, extract);

    fn metadata() -> AdapterMetadata {
        emery_sdk::metadata(SourceKind::Documentation)
    }

    // An inline value is one seam at once; a workspace's subjects are named
    // in one turn from the outlines code read, and its seams cut from them.
    #[cfg(feature = "model-survey")]
    async fn seams<P: Model>(ctx: &Context<'_, P>) -> Result<Vec<Seam>, Error> {
        match survey::prepare(ctx.input)? {
            survey::Preparation::Value => Ok(vec![Seam::whole()]),
            survey::Preparation::Workspace(prepared) if prepared.documents.is_empty() => {
                survey::survey(prepared.input)
            }
            survey::Preparation::Workspace(prepared) => {
                let inventory = survey::model::subjects(ctx, PROSE, &prepared).await?;
                survey::seams(&prepared, &inventory)
            }
        }
    }

    async fn extract<P: Model>(ctx: &Context<'_, P>) -> Result<Evidence, Error> {
        #[cfg(not(feature = "model-survey"))]
        let seams: Vec<Seam> = survey::survey(ctx.input)?;
        #[cfg(feature = "model-survey")]
        let seams = seams(ctx).await?;
        emery_sdk::extract(ctx, PROSE, &seams).await
    }
}

/// The prompts embedded in the adapter.
pub static PROSE: &[emery_sdk::Doc] =
    emery_sdk::prose!["../prose/extract.md", "../prose/survey.md"];
