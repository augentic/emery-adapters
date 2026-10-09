//! Extracts claims from a tree of written documentation.
//!
//! The source is a workspace or an inline value. An inline value is mined
//! whole, in one call, with no survey.
//!
//! # Survey
//!
//! A workspace is surveyed before it is mined. Every document that reads as
//! text is read for its outline — its headings at their lines, what its body
//! holds — and one survey call has the model name each subject the tree
//! documents:
//!
//! - a feature, a resource, a flow, a policy, a recorded decision
//! - anchored at the heading, or the first line, that introduces it
//! - under the domain noun the documents spell, as its stem
//!
//! A document introducing no subject is listed as unreached. The answer is
//! held to the tree before anything rests on it:
//!
//! - every document is a subject's entry or listed as unreached
//! - no two subjects anchor at one line
//!
//! From the accepted anchors the code derives the rest:
//!
//! - the lines each subject spans: from its anchor to the line before the
//!   next subject's in the same document, else the document's end, the
//!   preamble before the first folded into it
//! - the id its `requirement` and `criterion` claims lead with: the stem
//!   alone for the one subject under it, else the stem and a slug from the
//!   heading it is anchored at
//!
//! # Seams
//!
//! The seams follow the subjects:
//!
//! - a tree within the SDK's inline budget is one call over every subject's
//!   document, held to every stem
//! - a larger tree is one call per stem, over the documents its subjects
//!   anchor in, held to that stem alone
//! - a document the survey placed under no subject is context for every
//!   call and never a claim's `path`
//!
//! # Groups
//!
//! A tree whose survey names no subject, or that holds no document reading
//! as text, is grouped by top-level directory instead, the root's own
//! documents one group beside them:
//!
//! - a directory of fewer than two documents joins the root's group
//! - the root's group joins the first directory's when it is itself fewer
//!   than two
//! - a directory of more than sixteen documents is cut once more, by its
//!   subdirectories under the same rule, its remaining documents one group
//!   beside them; a directory no subdirectory can cut stays one group
//! - if fewer than two groups remain, the whole workspace is one

mod survey;

use emery_sdk::{Context, Error, Evidence, Model, SourceAdapter, SourceKind};

/// The adapter implementation.
pub struct Adapter;

impl SourceAdapter for Adapter {
    const KIND: SourceKind = SourceKind::Documentation;

    async fn extract<P: Model>(ctx: &Context<'_, P>) -> Result<Evidence, Error> {
        let seams = survey::survey(ctx).await?;
        emery_sdk::extract(ctx, PROSE, &seams).await
    }
}

/// The prompts embedded in the adapter.
pub static PROSE: &[emery_sdk::Doc] =
    emery_sdk::prose!["../prose/extract.md", "../prose/survey.md"];

// Export the adapter as a WASM module.
#[cfg(target_arch = "wasm32")]
emery_sdk::source_adapter!(Adapter);
