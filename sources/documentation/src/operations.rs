use emery_prose::registry::Doc;
use emery_sdk::{Context, Error, Evidence, Material, Model, SourceAdapter, SourceKind};

use crate::registry;

/// Written specifications / documentation trees → one Evidence document.
#[derive(Debug)]
pub struct Adapter;

impl SourceAdapter for Adapter {
    const KIND: SourceKind = SourceKind::Documentation;
    const SOURCE: &'static str = "documentation";

    fn docs() -> &'static [Doc] {
        registry::docs()
    }

    async fn extract<P: Model>(model: &P, ctx: &Context<'_>) -> Result<Evidence, Error> {
        Self::evidence(model, ctx, Material::Bound).await
    }
}
