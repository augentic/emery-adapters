//! The component: the `source-adapter` world answered over the host model.

use emery_sdk::export::{self, AdapterId, AdapterMetadata, Error, Evidence, Guest, Input};
use emery_sdk::model::WasiModel;
use emery_sdk::{Context, SourceInput};

struct Adapter;
export::export!(Adapter with_types_in export);

impl Guest for Adapter {
    fn metadata(_id: AdapterId) -> AdapterMetadata {
        export::metadata(crate::KIND)
    }

    async fn extract(id: AdapterId, input: Input) -> Result<Evidence, Error> {
        let input = SourceInput::from(input);
        let ctx = Context {
            adapter_id: &id,
            input: &input,
        };
        let seams = crate::survey(&input.content)?;
        Ok(emery_sdk::mine(&WasiModel, &ctx, crate::docs(), &seams).await?.into())
    }
}
