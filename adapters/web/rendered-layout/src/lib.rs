//! Experiment / non-production: reads a web page's rendered-layout facts as UI layout evidence.
//!
//! The source is a workspace holding the facts folder `web-layout-capture capture`
//! wrote. It is read whole by code, with no model turn: every element the
//! facts place is a `region`, `container`, or `leaf` claim at the facts file
//! holding it, and the facts travel whole as one layout with a wireframe at
//! each review width. An inline value, or a tree that does not read as facts,
//! is refused.

mod read;

use emery_sdk::{Context, Doc, Error, Evidence, Metadata, Model, SourceAdapter, SourceKind};

/// The adapter implementation.
pub struct Adapter;

impl SourceAdapter for Adapter {
    fn metadata() -> Metadata {
        Metadata {
            kind: SourceKind::Behaviour,
        }
    }

    #[expect(
        clippy::unused_async_trait_impl,
        reason = "the facts are read by code, awaiting nothing"
    )]
    async fn extract<P: Model>(ctx: &Context<'_, P>) -> Result<Evidence, Error> {
        read::evidence(ctx.input)
    }
}

/// The prompts embedded in the adapter: none, since no turn is put.
pub static PROSE: &[Doc] = &[];

#[cfg(target_arch = "wasm32")]
emery_sdk::export_source!(Adapter);
